//! Image sizes without decoding: PNG from its header, SVG from `width`/`height` or `viewBox`.

use crate::model::ImageSize;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Size of a PNG in pixels, read from its IHDR chunk.
pub fn png_size(bytes: &[u8]) -> Option<ImageSize> {
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some(ImageSize { width, height })
}

/// Size of an SVG's root element: `width` and `height` if both are plain numbers (optionally `px`), else the `viewBox` extent.
pub fn svg_size(text: &str) -> Option<ImageSize> {
    let start = text.find("<svg")?;
    let end = text[start..].find('>')? + start;
    let root = &text[start..end];
    let number = |value: &str| -> Option<f64> {
        let trimmed = value.trim().trim_end_matches("px").trim();
        trimmed.parse::<f64>().ok().filter(|number| *number > 0.0)
    };
    let from_dimensions = || -> Option<ImageSize> {
        let width = number(&attribute(root, "width")?)?;
        let height = number(&attribute(root, "height")?)?;
        Some(rounded(width, height))
    };
    let from_view_box = || -> Option<ImageSize> {
        let view_box = attribute(root, "viewBox")?;
        let parts: Vec<f64> =
            view_box.split([' ', ',']).filter(|part| !part.is_empty()).filter_map(|part| part.parse().ok()).collect();
        if parts.len() != 4 || parts[2] <= 0.0 || parts[3] <= 0.0 {
            return None;
        }
        Some(rounded(parts[2], parts[3]))
    };
    from_dimensions().or_else(from_view_box)
}

fn rounded(width: f64, height: f64) -> ImageSize {
    ImageSize { width: width.round().max(1.0) as u32, height: height.round().max(1.0) as u32 }
}

fn attribute(element: &str, name: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(found) = element[search_from..].find(name) {
        let at = search_from + found;
        let before_ok = element[..at].chars().next_back().is_none_or(char::is_whitespace);
        let rest = &element[at + name.len()..];
        if before_ok && let Some(after_equals) = rest.trim_start().strip_prefix('=') {
            let after_equals = after_equals.trim_start();
            let quote = after_equals.chars().next().filter(|quote| *quote == '"' || *quote == '\'')?;
            let value = &after_equals[1..];
            return value.find(quote).map(|close| value[..close].to_owned());
        }
        search_from = at + name.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn given_a_png_header_when_reading_then_the_pixel_size_comes_back() {
        assert_eq!(png_size(&png_header(12, 22)), Some(ImageSize { width: 12, height: 22 }));
    }

    #[test]
    fn given_bytes_that_are_not_a_png_when_reading_then_none() {
        assert_eq!(png_size(b"GIF89a....................."), None);
        assert_eq!(png_size(&png_header(0, 5)), None);
        assert_eq!(png_size(&png_header(5, 5)[..20]), None);
        let mut wrong_chunk = png_header(5, 5);
        wrong_chunk[12..16].copy_from_slice(b"IDAT");
        assert_eq!(png_size(&wrong_chunk), None);
    }

    #[test]
    fn given_an_svg_with_a_view_box_when_reading_then_its_extent_is_the_size() {
        assert_eq!(
            svg_size(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 30"><path/></svg>"#),
            Some(ImageSize { width: 1, height: 30 })
        );
        assert_eq!(svg_size(r#"<svg viewBox="0,0,22.4,21.6">"#), Some(ImageSize { width: 22, height: 22 }));
    }

    #[test]
    fn given_an_svg_with_width_and_height_when_reading_then_they_win_over_the_view_box() {
        assert_eq!(
            svg_size(r#"<svg width="24px" height='16' viewBox="0 0 100 100">"#),
            Some(ImageSize { width: 24, height: 16 })
        );
    }

    #[test]
    fn given_an_svg_with_percentage_dimensions_when_reading_then_the_view_box_is_used() {
        assert_eq!(
            svg_size(r#"<svg width="100%" height="100%" viewBox="0 0 9 7">"#),
            Some(ImageSize { width: 9, height: 7 })
        );
    }

    #[test]
    fn given_broken_or_sizeless_svg_when_reading_then_none() {
        assert_eq!(svg_size("<html>"), None);
        assert_eq!(svg_size("<svg>"), None);
        assert_eq!(svg_size(r#"<svg viewBox="0 0 0 5">"#), None);
        assert_eq!(svg_size(r#"<svg viewBox="1 2 3">"#), None);
        assert_eq!(svg_size(r#"<svg height="5" data-width="9">"#), None);
    }
}
