//! Direct3D 11 + Direct2D + DirectWrite terminal renderer. Draws a `ScreenSnapshot` into a composition swap chain
//! that the host attaches to a XAML `SwapChainPanel`. Everything is measured in physical pixels (the D2D target is 96 DPI).

use crate::color::{ColorScheme, ResolvedCellColors, Rgb};
use crate::gpu_bindings::{
    D2D_COLOR_F, D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_ANTIALIAS_MODE_ALIASED,
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_PIXEL_FORMAT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory,
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_METRICS,
    DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, DXGI_ALPHA_MODE_PREMULTIPLIED,
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE, DXGI_USAGE_RENDER_TARGET_OUTPUT, HMODULE, ID2D1Bitmap1,
    ID2D1DeviceContext, ID2D1Factory1, ID2D1Image, ID2D1SolidColorBrush, ID3D11Device, ID3D11DeviceContext,
    ID3D11Texture2D, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat, IDXGIAdapter, IDXGIDevice,
    IDXGIFactory2, IDXGIOutput, IDXGISurface, IDXGISwapChain1,
};
use core::mem::ManuallyDrop;
use windows_core::{Interface, PCWSTR, Result};
use winuake_term::snapshot::{CursorStyle, ScreenSnapshot, SnapshotCell, UnderlineStyle};

const FALLBACK_FONT_FAMILY: &str = "Consolas";

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec {
    pub family: String,
    pub size_points: f32,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self { family: "Cascadia Mono".to_owned(), size_points: 11.0 }
    }
}

/// Size of one terminal cell in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    pub width: f32,
    pub height: f32,
    /// Distance from the cell top to the text baseline.
    pub baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub columns: u16,
    pub rows: u16,
}

/// BGRA8 pixels, as read back from the GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl PixelImage {
    pub fn pixel(&self, x: u32, y: u32) -> Rgb {
        let offset = ((y * self.width + x) * 4) as usize;
        Rgb::new(self.bgra[offset + 2], self.bgra[offset + 1], self.bgra[offset])
    }
}

pub struct TerminalRenderer {
    d3d_device: ID3D11Device,
    d3d_context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    d2d_context: ID2D1DeviceContext,
    target: Option<ID2D1Bitmap1>,
    brush: ID2D1SolidColorBrush,
    /// Regular, bold, italic, bold italic.
    text_formats: [IDWriteTextFormat; 4],
    metrics: CellMetrics,
    width_px: u32,
    height_px: u32,
}

impl TerminalRenderer {
    /// `dpi` scales the font (96 = 100%). The swap chain starts at `width_px` x `height_px`.
    pub fn new(font: &FontSpec, dpi: f32, width_px: u32, height_px: u32) -> Result<Self> {
        let (width_px, height_px) = (width_px.max(1), height_px.max(1));
        unsafe {
            let (d3d_device, d3d_context) = create_d3d_device()?;
            let dxgi_device: IDXGIDevice = d3d_device.cast()?;
            let dxgi_factory: IDXGIFactory2 = dxgi_device.GetAdapter()?.GetParent()?;
            let description = DXGI_SWAP_CHAIN_DESC1 {
                Width: width_px,
                Height: height_px,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE(DXGI_USAGE_RENDER_TARGET_OUTPUT),
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };
            let swap_chain =
                dxgi_factory.CreateSwapChainForComposition(&d3d_device, &description, None::<&IDXGIOutput>)?;

            let d2d_factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d_device = d2d_factory.CreateDevice(&dxgi_device)?;
            let d2d_context = d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let brush = d2d_context.CreateSolidColorBrush(&color_f(Rgb::new(255, 255, 255)), None)?;

            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let family = resolve_font_family(&dwrite, &font.family)?;
            let size_px = font.size_points * dpi / 72.0;
            let text_formats = [
                create_text_format(&dwrite, &family, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, size_px)?,
                create_text_format(&dwrite, &family, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_STYLE_NORMAL, size_px)?,
                create_text_format(&dwrite, &family, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_ITALIC, size_px)?,
                create_text_format(&dwrite, &family, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_STYLE_ITALIC, size_px)?,
            ];
            let metrics = measure_cell(&dwrite, &text_formats[0])?;

            let mut renderer = Self {
                d3d_device,
                d3d_context,
                swap_chain,
                d2d_context,
                target: None,
                brush,
                text_formats,
                metrics,
                width_px,
                height_px,
            };
            renderer.attach_target()?;
            Ok(renderer)
        }
    }

    /// Attach this to a `SwapChainPanel` (via `ISwapChainPanelNative::SetSwapChain`).
    pub fn swap_chain(&self) -> &IDXGISwapChain1 {
        &self.swap_chain
    }

    pub fn cell_metrics(&self) -> CellMetrics {
        self.metrics
    }

    pub fn size_px(&self) -> (u32, u32) {
        (self.width_px, self.height_px)
    }

    /// How many whole cells fit in the given pixel area.
    pub fn grid_size_for(&self, width_px: u32, height_px: u32) -> GridSize {
        let columns = (width_px as f32 / self.metrics.width).floor().clamp(1.0, f32::from(u16::MAX)) as u16;
        let rows = (height_px as f32 / self.metrics.height).floor().clamp(1.0, f32::from(u16::MAX)) as u16;
        GridSize { columns, rows }
    }

    pub fn resize(&mut self, width_px: u32, height_px: u32) -> Result<()> {
        let (width_px, height_px) = (width_px.max(1), height_px.max(1));
        if (width_px, height_px) == (self.width_px, self.height_px) {
            return Ok(());
        }
        unsafe {
            self.d2d_context.SetTarget(None::<&ID2D1Image>);
            self.target = None;
            self.swap_chain.ResizeBuffers(0, width_px, height_px, DXGI_FORMAT_B8G8R8A8_UNORM, 0).ok()?;
        }
        self.width_px = width_px;
        self.height_px = height_px;
        unsafe { self.attach_target() }
    }

    /// Draws `snapshot` and presents it.
    pub fn render(&mut self, snapshot: &ScreenSnapshot, scheme: &ColorScheme, cursor_visible: bool) -> Result<()> {
        unsafe {
            self.draw(snapshot, scheme, cursor_visible)?;
            self.swap_chain.Present(1, Default::default()).ok()
        }
    }

    /// Draws `snapshot`, reads the pixels back before presenting, then presents. For tests and screenshots.
    pub fn render_and_capture(
        &mut self,
        snapshot: &ScreenSnapshot,
        scheme: &ColorScheme,
        cursor_visible: bool,
    ) -> Result<PixelImage> {
        unsafe {
            self.draw(snapshot, scheme, cursor_visible)?;
            let image = self.read_back_buffer()?;
            self.swap_chain.Present(1, Default::default()).ok()?;
            Ok(image)
        }
    }

    unsafe fn attach_target(&mut self) -> Result<()> {
        unsafe {
            let surface: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                colorContext: ManuallyDrop::new(None),
            };
            let bitmap = self.d2d_context.CreateBitmapFromDxgiSurface(&surface, Some(&properties))?;
            self.d2d_context.SetTarget(&bitmap);
            self.target = Some(bitmap);
        }
        Ok(())
    }

    unsafe fn draw(&self, snapshot: &ScreenSnapshot, scheme: &ColorScheme, cursor_visible: bool) -> Result<()> {
        let dc = &self.d2d_context;
        let CellMetrics { width, height, baseline } = self.metrics;
        let resolved: Vec<ResolvedCellColors> = snapshot.cells.iter().map(|cell| scheme.resolve(cell)).collect();
        let column_x = |column: usize| (column as f32 * width).round();
        let row_y = |row: usize| (row as f32 * height).round();

        unsafe {
            dc.BeginDraw();
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            dc.Clear(Some(&color_f(scheme.background)));

            // Backgrounds, coalesced into horizontal runs. Aliased so neighbouring rectangles do not leave seams.
            dc.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            for row in 0..snapshot.rows {
                let mut column = 0;
                while column < snapshot.columns {
                    let background = resolved[row * snapshot.columns + column].background;
                    let mut end = column + 1;
                    while end < snapshot.columns && resolved[row * snapshot.columns + end].background == background {
                        end += 1;
                    }
                    if background != scheme.background {
                        self.fill(rect(column_x(column), row_y(row), column_x(end), row_y(row + 1)), background);
                    }
                    column = end;
                }
            }

            // Text, underline and strikethrough.
            dc.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            for row in 0..snapshot.rows {
                self.draw_row_text(snapshot, &resolved, row, row_y(row), column_x(0))?;
                for column in 0..snapshot.columns {
                    let cell = snapshot.cell(row, column);
                    let cell_width = if cell.attributes.wide { 2.0 * width } else { width };
                    let (left, top) = (column_x(column), row_y(row));
                    let foreground = resolved[row * snapshot.columns + column].foreground;
                    if cell.attributes.underline != UnderlineStyle::None && !cell.attributes.wide_spacer {
                        let underline_y = (top + baseline + 1.0).min(top + height - 1.0).round();
                        self.fill(rect(left, underline_y, left + cell_width, underline_y + 1.0), foreground);
                        if cell.attributes.underline == UnderlineStyle::Double {
                            self.fill(rect(left, underline_y - 2.0, left + cell_width, underline_y - 1.0), foreground);
                        }
                    }
                    if cell.attributes.strikethrough && !cell.attributes.wide_spacer {
                        let strike_y = (top + height * 0.5).round();
                        self.fill(rect(left, strike_y, left + cell_width, strike_y + 1.0), foreground);
                    }
                }
            }

            if cursor_visible && let Some(cursor) = snapshot.cursor {
                self.draw_cursor(snapshot, scheme, cursor.row, cursor.column, cursor.style, &resolved);
            }

            dc.EndDraw(None, None).ok()
        }
    }

    unsafe fn draw_row_text(
        &self,
        snapshot: &ScreenSnapshot,
        resolved: &[ResolvedCellColors],
        row: usize,
        top: f32,
        _left: f32,
    ) -> Result<()> {
        let width = self.metrics.width;
        let cells = snapshot.row(row);
        let mut column = 0;
        while column < cells.len() {
            let cell = &cells[column];
            if cell.attributes.wide_spacer || (cell.character == ' ' && !cell.attributes.wide) {
                column += 1;
                continue;
            }
            let foreground = resolved[row * snapshot.columns + column].foreground;
            let style_index = text_format_index(cell);

            if cell.attributes.wide || !cell.character.is_ascii_graphic() {
                // Non-ASCII glyphs may not advance exactly one cell: place each at its own cell.
                let span = if cell.attributes.wide { 2.0 } else { 1.0 };
                let left = (column as f32 * width).round();
                unsafe {
                    self.draw_text(
                        &cell.character.to_string(),
                        style_index,
                        left,
                        top,
                        left + span * width + width,
                        foreground,
                    )
                };
                column += 1;
                continue;
            }

            // Run of ASCII cells sharing foreground and font style; a monospace font keeps them aligned.
            let start = column;
            let mut text = String::new();
            while column < cells.len() {
                let next = &cells[column];
                let same_style = text_format_index(next) == style_index
                    && resolved[row * snapshot.columns + column].foreground == foreground
                    && !next.attributes.wide
                    && !next.attributes.wide_spacer
                    && (next.character.is_ascii_graphic() || next.character == ' ');
                if !same_style {
                    break;
                }
                text.push(next.character);
                column += 1;
            }
            let trimmed = text.trim_end();
            let left = (start as f32 * width).round();
            unsafe {
                self.draw_text(trimmed, style_index, left, top, left + (trimmed.len() as f32 + 1.0) * width, foreground)
            };
        }
        Ok(())
    }

    unsafe fn draw_text(&self, text: &str, style_index: usize, left: f32, top: f32, right: f32, color: Rgb) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout_rect = rect(left, top, right, top + self.metrics.height);
        unsafe {
            self.brush.SetColor(&color_f(color));
            self.d2d_context.DrawText(
                &wide,
                &self.text_formats[style_index],
                &layout_rect,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                0,
            );
        }
    }

    unsafe fn draw_cursor(
        &self,
        snapshot: &ScreenSnapshot,
        scheme: &ColorScheme,
        row: usize,
        column: usize,
        style: CursorStyle,
        resolved: &[ResolvedCellColors],
    ) {
        let CellMetrics { width, height, .. } = self.metrics;
        let cell = snapshot.cell(row, column);
        let cell_width = if cell.attributes.wide { 2.0 * width } else { width };
        let (left, top) = ((column as f32 * width).round(), (row as f32 * height).round());
        let (right, bottom) = (left + cell_width, top + height);
        let thickness = (height * 0.1).round().max(1.0);
        unsafe {
            match style {
                CursorStyle::Block => {
                    self.fill(rect(left, top, right, bottom), scheme.cursor);
                    if cell.character != ' ' {
                        let background = resolved[row * snapshot.columns + column].background;
                        self.draw_text(
                            &cell.character.to_string(),
                            text_format_index(cell),
                            left,
                            top,
                            right + width,
                            background,
                        );
                    }
                }
                CursorStyle::Underline => self.fill(rect(left, bottom - thickness, right, bottom), scheme.cursor),
                CursorStyle::Beam => {
                    self.fill(rect(left, top, left + (width * 0.12).round().max(1.0), bottom), scheme.cursor)
                }
                CursorStyle::HollowBlock => {
                    self.fill(rect(left, top, right, top + 1.0), scheme.cursor);
                    self.fill(rect(left, bottom - 1.0, right, bottom), scheme.cursor);
                    self.fill(rect(left, top, left + 1.0, bottom), scheme.cursor);
                    self.fill(rect(right - 1.0, top, right, bottom), scheme.cursor);
                }
            }
        }
    }

    unsafe fn fill(&self, area: D2D_RECT_F, color: Rgb) {
        unsafe {
            self.brush.SetColor(&color_f(color));
            self.d2d_context.FillRectangle(&area, &self.brush);
        }
    }

    unsafe fn read_back_buffer(&self) -> Result<PixelImage> {
        unsafe {
            let back_buffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
            let mut description = D3D11_TEXTURE2D_DESC::default();
            back_buffer.GetDesc(&mut description);
            description.Usage = D3D11_USAGE_STAGING;
            description.BindFlags = 0;
            description.CPUAccessFlags = D3D11_CPU_ACCESS_READ as u32;
            description.MiscFlags = 0;

            let mut staging: Option<ID3D11Texture2D> = None;
            self.d3d_device.CreateTexture2D(&description, None, Some(&mut staging)).ok()?;
            let staging = staging.ok_or_else(windows_core::Error::empty)?;
            self.d3d_context.CopyResource(&staging, &back_buffer);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.d3d_context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).ok()?;
            let (width, height) = (description.Width, description.Height);
            let mut bgra = Vec::with_capacity((width * height * 4) as usize);
            for row in 0..height {
                let source = (mapped.pData as *const u8).add((row * mapped.RowPitch) as usize);
                bgra.extend_from_slice(core::slice::from_raw_parts(source, (width * 4) as usize));
            }
            self.d3d_context.Unmap(&staging, 0);
            Ok(PixelImage { width, height, bgra })
        }
    }
}

fn text_format_index(cell: &SnapshotCell) -> usize {
    usize::from(cell.attributes.bold) + 2 * usize::from(cell.attributes.italic)
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F { left, top, right, bottom }
}

fn color_f(color: Rgb) -> D2D_COLOR_F {
    D2D_COLOR_F {
        r: f32::from(color.red) / 255.0,
        g: f32::from(color.green) / 255.0,
        b: f32::from(color.blue) / 255.0,
        a: 1.0,
    }
}

fn wide_null_terminated(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut last_error = None;
    for driver_type in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let result = unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                driver_type,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT as u32,
                None,
                D3D11_SDK_VERSION as u32,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        match (result.ok(), device, context) {
            (Ok(()), Some(device), Some(context)) => return Ok((device, context)),
            (Err(error), _, _) => last_error = Some(error),
            _ => {}
        }
    }
    Err(last_error.unwrap_or_else(windows_core::Error::empty))
}

/// The requested family if installed, otherwise Consolas.
unsafe fn resolve_font_family(dwrite: &IDWriteFactory, requested: &str) -> Result<String> {
    unsafe {
        let mut collection: Option<IDWriteFontCollection> = None;
        dwrite.GetSystemFontCollection(&mut collection, false).ok()?;
        let collection = collection.ok_or_else(windows_core::Error::empty)?;
        let name = wide_null_terminated(requested);
        let mut index = 0u32;
        let mut exists = windows_core::BOOL::default();
        collection.FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists).ok()?;
        Ok(if exists.as_bool() { requested.to_owned() } else { FALLBACK_FONT_FAMILY.to_owned() })
    }
}

unsafe fn create_text_format(
    dwrite: &IDWriteFactory,
    family: &str,
    weight: i32,
    style: i32,
    size_px: f32,
) -> Result<IDWriteTextFormat> {
    let family = wide_null_terminated(family);
    let locale = wide_null_terminated("en-us");
    unsafe {
        let format = dwrite.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None::<&IDWriteFontCollection>,
            weight,
            style,
            DWRITE_FONT_STRETCH_NORMAL,
            size_px,
            PCWSTR(locale.as_ptr()),
        )?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
        Ok(format)
    }
}

/// Cell width = average advance of ten `M`; height = line height rounded up to whole pixels.
unsafe fn measure_cell(dwrite: &IDWriteFactory, format: &IDWriteTextFormat) -> Result<CellMetrics> {
    let sample: Vec<u16> = "MMMMMMMMMM".encode_utf16().collect();
    unsafe {
        let layout = dwrite.CreateTextLayout(&sample, format, 10_000.0, 10_000.0)?;
        let mut text_metrics = DWRITE_TEXT_METRICS::default();
        layout.GetMetrics(&mut text_metrics).ok()?;
        let mut line_metrics = DWRITE_LINE_METRICS::default();
        let mut line_count = 0u32;
        layout.GetLineMetrics(Some(&mut line_metrics), 1, &mut line_count).ok()?;
        Ok(CellMetrics {
            width: text_metrics.width / 10.0,
            height: line_metrics.height.ceil(),
            baseline: line_metrics.baseline,
        })
    }
}
