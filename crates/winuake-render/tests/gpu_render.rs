//! Renders terminal snapshots on the real GPU and inspects the pixels. Windows only.
#![cfg(windows)]

use winuake_render::color::{ColorScheme, Rgb};
use winuake_render::gpu::{CellMetrics, FontSpec, PixelImage, TerminalRenderer};
use winuake_term::emulator::{CellSide, SelectionKind, TerminalEmulator, ViewportCell};
use winuake_term::pty::TerminalSize;

/// Composition swap chains need an interactive desktop (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` otherwise, for example over SSH or in
/// some CI sessions). Tests skip loudly there; run them with `scripts/run-interactive-tests` style tooling or on a logged-in desktop.
fn no_interactive_desktop() -> bool {
    match TerminalRenderer::new(&FontSpec::default(), 96.0, 64, 64) {
        Ok(_) => false,
        Err(error) if error.code().0 as u32 == 0x887A_0022 => {
            eprintln!("SKIPPED: no interactive desktop available for DirectComposition ({error})");
            true
        }
        Err(error) => panic!("renderer creation failed: {error}"),
    }
}

struct Frame {
    image: PixelImage,
    metrics: CellMetrics,
    scheme: ColorScheme,
}

impl Frame {
    /// Pixel inside cell (row, column), `dx`/`dy` pixels from its top-left corner.
    fn pixel_in_cell(&self, row: usize, column: usize, dx: u32, dy: u32) -> Rgb {
        let left = (column as f32 * self.metrics.width).round() as u32;
        let top = (row as f32 * self.metrics.height).round() as u32;
        self.image.pixel(left + dx, top + dy)
    }

    fn cell_rect(&self, row: usize, column: usize) -> (u32, u32, u32, u32) {
        let left = (column as f32 * self.metrics.width).round() as u32;
        let right = ((column + 1) as f32 * self.metrics.width).round() as u32;
        let top = (row as f32 * self.metrics.height).round() as u32;
        let bottom = ((row + 1) as f32 * self.metrics.height).round() as u32;
        (left, top, right, bottom)
    }

    /// How many pixels in the cell differ from `background`.
    fn painted_pixels(&self, row: usize, column: usize, background: Rgb) -> usize {
        let (left, top, right, bottom) = self.cell_rect(row, column);
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| (x, y)))
            .filter(|&(x, y)| self.image.pixel(x, y) != background)
            .count()
    }

    fn brightest_channel_in_cell(&self, row: usize, column: usize) -> u8 {
        let (left, top, right, bottom) = self.cell_rect(row, column);
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| (x, y)))
            .map(|(x, y)| self.image.pixel(x, y))
            .map(|pixel| pixel.red.max(pixel.green).max(pixel.blue))
            .max()
            .unwrap_or(0)
    }
}

fn render_with(input: &str, prepare: impl FnOnce(&mut TerminalEmulator), cursor_visible: bool) -> Frame {
    let mut renderer = TerminalRenderer::new(&FontSpec::default(), 96.0, 640, 320).expect("renderer");
    let grid = renderer.grid_size_for(640, 320);
    let (columns, rows) = (grid.columns, grid.rows);
    let (width, height) = (
        (f32::from(columns) * renderer.cell_metrics().width).ceil() as u32,
        (f32::from(rows) * renderer.cell_metrics().height) as u32,
    );
    renderer.resize(width, height).expect("resize to the grid");

    let mut emulator = TerminalEmulator::new(TerminalSize::new(columns, rows), 100);
    emulator.advance(input.as_bytes());
    prepare(&mut emulator);

    let scheme = ColorScheme::campbell();
    let image = renderer.render_and_capture(&emulator.snapshot(), &scheme, cursor_visible).expect("render");
    Frame { image, metrics: renderer.cell_metrics(), scheme }
}

#[test]
fn given_the_default_font_when_measuring_then_cells_have_sensible_proportions() {
    if no_interactive_desktop() {
        return;
    }
    let renderer = TerminalRenderer::new(&FontSpec::default(), 96.0, 320, 200).expect("renderer");
    let metrics = renderer.cell_metrics();
    assert!(metrics.width > 4.0 && metrics.width < 30.0, "{metrics:?}");
    assert!(metrics.height > metrics.width && metrics.height < 60.0, "{metrics:?}");
    assert!(metrics.baseline > 0.0 && metrics.baseline < metrics.height, "{metrics:?}");
    let grid = renderer.grid_size_for(320, 200);
    assert_eq!(grid.columns, (320.0 / metrics.width).floor() as u16);
    assert_eq!(grid.rows, (200.0 / metrics.height).floor() as u16);
}

#[test]
fn given_an_unknown_font_when_creating_the_renderer_then_it_falls_back_and_still_renders() {
    if no_interactive_desktop() {
        return;
    }
    let font = FontSpec { family: "Definitely Not A Font".to_owned(), size_points: 11.0 };
    let renderer = TerminalRenderer::new(&font, 96.0, 320, 200).expect("renderer falls back to Consolas");
    assert!(renderer.cell_metrics().width > 0.0);
}

#[test]
fn given_a_larger_dpi_when_measuring_then_cells_grow_proportionally() {
    if no_interactive_desktop() {
        return;
    }
    let normal = TerminalRenderer::new(&FontSpec::default(), 96.0, 100, 100).unwrap().cell_metrics();
    let doubled = TerminalRenderer::new(&FontSpec::default(), 192.0, 100, 100).unwrap().cell_metrics();
    assert!((doubled.width / normal.width - 2.0).abs() < 0.15, "{normal:?} vs {doubled:?}");
    assert!((doubled.height / normal.height - 2.0).abs() < 0.15, "{normal:?} vs {doubled:?}");
}

#[test]
fn given_an_empty_screen_when_rendering_then_every_corner_is_the_scheme_background() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("", |_| {}, false);
    let (width, height) = (frame.image.width, frame.image.height);
    for (x, y) in [(0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1), (width / 2, height / 2)] {
        assert_eq!(frame.image.pixel(x, y), frame.scheme.background, "pixel ({x}, {y})");
    }
}

#[test]
fn given_cells_with_a_background_color_when_rendering_then_exactly_those_cells_are_filled() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("\x1b[44m   \x1b[0m", |_| {}, false);
    let blue = frame.scheme.ansi[4];
    assert_eq!(frame.pixel_in_cell(0, 0, 1, 1), blue);
    assert_eq!(frame.pixel_in_cell(0, 2, 1, 1), blue);
    assert_eq!(frame.pixel_in_cell(0, 3, 1, 1), frame.scheme.background);
    assert_eq!(frame.pixel_in_cell(1, 0, 1, 1), frame.scheme.background);
}

#[test]
fn given_text_when_rendering_then_glyph_pixels_are_drawn_in_the_foreground_color() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("W", |_| {}, false);
    assert!(frame.painted_pixels(0, 0, frame.scheme.background) > 10, "glyph left no pixels");
    assert!(frame.brightest_channel_in_cell(0, 0) > 180, "glyph is not close to the default foreground");
    assert_eq!(frame.painted_pixels(0, 1, frame.scheme.background), 0, "next cell must stay empty");
}

#[test]
fn given_colored_text_when_rendering_then_glyph_pixels_lean_toward_that_color() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("\x1b[1;31mW", |_| {}, false);
    let (left, top, right, bottom) = frame.cell_rect(0, 0);
    let reddest = (top..bottom)
        .flat_map(|y| (left..right).map(move |x| (x, y)))
        .map(|(x, y)| frame.image.pixel(x, y))
        .max_by_key(|pixel| i32::from(pixel.red) - i32::from(pixel.green.max(pixel.blue)))
        .unwrap();
    assert!(reddest.red > 200 && reddest.green < 120 && reddest.blue < 120, "{reddest:?}");
}

#[test]
fn given_a_block_cursor_when_visible_then_its_cell_is_filled_with_the_cursor_color_and_hidden_cursor_is_not() {
    if no_interactive_desktop() {
        return;
    }
    let shown = render_with("abc", |_| {}, true);
    assert_eq!(shown.pixel_in_cell(0, 3, 1, 1), shown.scheme.cursor);
    let hidden = render_with("abc", |_| {}, false);
    assert_eq!(hidden.pixel_in_cell(0, 3, 1, 1), hidden.scheme.background);
}

#[test]
fn given_a_beam_cursor_when_rendering_then_only_the_left_edge_of_the_cell_is_filled() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("ab\x1b[6 q", |_| {}, true);
    assert_eq!(frame.pixel_in_cell(0, 2, 0, 3), frame.scheme.cursor);
    let (left, _, right, _) = frame.cell_rect(0, 2);
    assert_eq!(frame.pixel_in_cell(0, 2, right - left - 2, 3), frame.scheme.background);
}

#[test]
fn given_a_selection_when_rendering_then_selected_cells_use_the_selection_background() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with(
        "abcd",
        |terminal| {
            terminal.start_selection(SelectionKind::Simple, ViewportCell { row: 0, column: 1 }, CellSide::Left);
            terminal.update_selection(ViewportCell { row: 0, column: 2 }, CellSide::Right);
        },
        false,
    );
    let bottom_row = (frame.metrics.height as u32) - 1;
    assert_eq!(frame.pixel_in_cell(0, 1, 0, bottom_row), frame.scheme.selection_background);
    assert_eq!(frame.pixel_in_cell(0, 2, 0, bottom_row), frame.scheme.selection_background);
    assert_eq!(frame.pixel_in_cell(0, 0, 0, bottom_row), frame.scheme.background);
    assert_eq!(frame.pixel_in_cell(0, 3, 0, bottom_row), frame.scheme.background);
}

#[test]
fn given_inverse_text_when_rendering_then_the_cell_background_is_the_foreground_color() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("\x1b[7m   ", |_| {}, false);
    assert_eq!(frame.pixel_in_cell(0, 1, 1, 1), frame.scheme.foreground);
}

#[test]
fn given_underlined_text_when_rendering_then_a_line_spans_the_cell_width_near_the_baseline() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("\x1b[4m   ", |_| {}, false);
    let (left, top, right, _) = frame.cell_rect(0, 1);
    let underline_y =
        (top as f32 + frame.metrics.baseline + 1.0).min(top as f32 + frame.metrics.height - 1.0).round() as u32;
    for x in left..right {
        assert_eq!(frame.image.pixel(x, underline_y), frame.scheme.foreground, "x = {x}");
    }
}

#[test]
fn given_a_wide_character_when_rendering_then_it_spans_two_cells_and_following_text_stays_on_the_grid() {
    if no_interactive_desktop() {
        return;
    }
    let frame = render_with("世b", |_| {}, false);
    let wide_pixels =
        frame.painted_pixels(0, 0, frame.scheme.background) + frame.painted_pixels(0, 1, frame.scheme.background);
    assert!(wide_pixels > 20, "wide glyph left only {wide_pixels} pixels");
    assert!(frame.painted_pixels(0, 2, frame.scheme.background) > 5, "'b' must be drawn in the third cell");
    assert_eq!(frame.painted_pixels(0, 3, frame.scheme.background), 0);
}

#[test]
fn given_a_resize_when_rendering_again_then_the_image_has_the_new_size() {
    if no_interactive_desktop() {
        return;
    }
    let mut renderer = TerminalRenderer::new(&FontSpec::default(), 96.0, 400, 300).unwrap();
    let emulator = TerminalEmulator::new(TerminalSize::new(10, 5), 10);
    let scheme = ColorScheme::campbell();

    renderer.resize(800, 600).unwrap();
    let image = renderer.render_and_capture(&emulator.snapshot(), &scheme, false).unwrap();

    assert_eq!((image.width, image.height), (800, 600));
    assert_eq!(renderer.size_px(), (800, 600));
    assert!(renderer.grid_size_for(800, 600).columns > renderer.grid_size_for(400, 300).columns);
}
