//! SVG to PNG, so WinUI (which shows PNG from a file URI reliably) can draw the default skin's vector images crisply at any DPI.

use crate::model::ImageSize;
use resvg::{tiny_skia, usvg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterizedSvg {
    pub png: Vec<u8>,
    /// Size in DIPs: the image drawn at 1x. The PNG itself is `scale` times larger.
    pub size: ImageSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterError {
    pub reason: String,
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "cannot rasterize svg: {}", self.reason)
    }
}

impl std::error::Error for RasterError {}

/// Renders SVG text at `scale` times its natural size.
pub fn svg_to_png(svg: &[u8], scale: u32) -> Result<RasterizedSvg, RasterError> {
    let error = |reason: &str| RasterError { reason: reason.to_owned() };
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
        .map_err(|parse| RasterError { reason: parse.to_string() })?;
    let natural = tree.size();
    let size =
        ImageSize { width: natural.width().round().max(1.0) as u32, height: natural.height().round().max(1.0) as u32 };
    let scale = scale.max(1);
    let mut pixmap = tiny_skia::Pixmap::new(size.width * scale, size.height * scale)
        .ok_or_else(|| error("the image is too large"))?;
    let factor = tiny_skia::Transform::from_scale(
        (size.width * scale) as f32 / natural.width(),
        (size.height * scale) as f32 / natural.height(),
    );
    resvg::render(&tree, factor, &mut pixmap.as_mut());
    let png = pixmap.encode_png().map_err(|encode| RasterError { reason: encode.to_string() })?;
    Ok(RasterizedSvg { png, size })
}

/// Renders SVG text into a square PNG of exactly `pixels` by `pixels` (the image is scaled to fit and centered), for icons.
pub fn svg_to_square_png(svg: &[u8], pixels: u32) -> Result<Vec<u8>, RasterError> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
        .map_err(|parse| RasterError { reason: parse.to_string() })?;
    let natural = tree.size();
    let pixels = pixels.max(1);
    let mut pixmap = tiny_skia::Pixmap::new(pixels, pixels)
        .ok_or_else(|| RasterError { reason: "the image is too large".to_owned() })?;
    let scale = pixels as f32 / natural.width().max(natural.height());
    let offset_x = (pixels as f32 - natural.width() * scale) / 2.0;
    let offset_y = (pixels as f32 - natural.height() * scale) / 2.0;
    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|encode| RasterError { reason: encode.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_info::png_size;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 2"><rect width="4" height="2" fill="#ff0000"/></svg>"##;

    #[test]
    fn given_an_svg_when_rasterized_at_3x_then_the_png_is_three_times_larger_and_the_dip_size_is_natural() {
        let image = svg_to_png(SQUARE.as_bytes(), 3).expect("renders");
        assert_eq!(image.size, ImageSize { width: 4, height: 2 });
        assert_eq!(png_size(&image.png), Some(ImageSize { width: 12, height: 6 }));
    }

    #[test]
    fn given_an_svg_when_rasterized_then_the_shape_is_painted() {
        let image = svg_to_png(SQUARE.as_bytes(), 1).expect("renders");
        let decoded = tiny_skia::Pixmap::decode_png(&image.png).expect("a valid png");
        let pixel = decoded.pixel(1, 1).expect("inside");
        assert_eq!((pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()), (255, 0, 0, 255));
    }

    #[test]
    fn given_a_zero_scale_when_rasterizing_then_it_is_treated_as_one() {
        let image = svg_to_png(SQUARE.as_bytes(), 0).expect("renders");
        assert_eq!(png_size(&image.png), Some(ImageSize { width: 4, height: 2 }));
    }

    #[test]
    fn given_text_that_is_not_svg_when_rasterizing_then_an_error_explains() {
        let error = svg_to_png(b"not svg", 1).unwrap_err();
        assert!(error.to_string().contains("cannot rasterize"), "{error}");
    }

    #[test]
    fn given_a_wide_svg_when_rendering_a_square_icon_then_the_png_has_exactly_that_size() {
        let png = svg_to_square_png(SQUARE.as_bytes(), 32).expect("renders");
        assert_eq!(png_size(&png), Some(ImageSize { width: 32, height: 32 }));
        let decoded = tiny_skia::Pixmap::decode_png(&png).expect("a valid png");
        assert_eq!(decoded.pixel(16, 16).map(|pixel| pixel.alpha()), Some(255), "the middle is painted");
        assert_eq!(
            decoded.pixel(16, 1).map(|pixel| pixel.alpha()),
            Some(0),
            "the letterbox above the wide image is transparent"
        );
    }
}
