//! Named color schemes: a few built in, more imported from Windows Terminal JSON or Konsole `.colorscheme` files.
//!
//! Replaces Konsole's profile color schemes, which Yakuake gets for free from the Konsole part.

use crate::color::{ColorScheme, Rgb};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedScheme {
    pub name: String,
    pub scheme: ColorScheme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeImportError {
    pub reason: String,
}

impl fmt::Display for SchemeImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot import color scheme: {}", self.reason)
    }
}

impl std::error::Error for SchemeImportError {}

fn error(reason: impl Into<String>) -> SchemeImportError {
    SchemeImportError { reason: reason.into() }
}

fn hex(value: u32) -> Rgb {
    Rgb::new((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn ansi_from(values: [u32; 16]) -> [Rgb; 16] {
    values.map(hex)
}

pub fn builtin_schemes() -> Vec<NamedScheme> {
    let campbell = ColorScheme::campbell();
    let one_half_dark = ColorScheme {
        foreground: hex(0xDCDFE4),
        background: hex(0x282C34),
        cursor: hex(0xFFFFFF),
        selection_background: hex(0x3E4451),
        ansi: ansi_from([
            0x282C34, 0xE06C75, 0x98C379, 0xE5C07B, 0x61AFEF, 0xC678DD, 0x56B6C2, 0xDCDFE4, 0x5A6374, 0xE06C75,
            0x98C379, 0xE5C07B, 0x61AFEF, 0xC678DD, 0x56B6C2, 0xDCDFE4,
        ]),
        ..campbell.clone()
    };
    let solarized_ansi = ansi_from([
        0x002B36, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5, 0x073642, 0xCB4B16, 0x586E75,
        0x657B83, 0x839496, 0x6C71C4, 0x93A1A1, 0xFDF6E3,
    ]);
    let solarized_dark = ColorScheme {
        foreground: hex(0x839496),
        background: hex(0x002B36),
        cursor: hex(0xFFFFFF),
        selection_background: hex(0x073642),
        ansi: solarized_ansi,
        ..campbell.clone()
    };
    let solarized_light = ColorScheme {
        foreground: hex(0x657B83),
        background: hex(0xFDF6E3),
        cursor: hex(0x586E75),
        selection_background: hex(0xEEE8D5),
        ansi: solarized_ansi,
        ..campbell.clone()
    };
    vec![
        NamedScheme { name: "Campbell".to_owned(), scheme: campbell },
        NamedScheme { name: "One Half Dark".to_owned(), scheme: one_half_dark },
        NamedScheme { name: "Solarized Dark".to_owned(), scheme: solarized_dark },
        NamedScheme { name: "Solarized Light".to_owned(), scheme: solarized_light },
    ]
}

/// Case-insensitive lookup by name.
pub fn find<'a>(schemes: &'a [NamedScheme], name: &str) -> Option<&'a NamedScheme> {
    schemes.iter().find(|candidate| candidate.name.eq_ignore_ascii_case(name.trim()))
}

/// Built-in schemes followed by imported ones. An imported scheme replaces a built-in of the same name.
pub fn catalog(imported: Vec<NamedScheme>) -> Vec<NamedScheme> {
    let mut schemes = builtin_schemes();
    for scheme in imported {
        schemes.retain(|existing| !existing.name.eq_ignore_ascii_case(&scheme.name));
        schemes.push(scheme);
    }
    schemes
}

/// Parses `#RRGGBB` or `#RGB`.
pub fn parse_hex_color(text: &str) -> Option<Rgb> {
    let digits = text.trim().strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(hex(value)),
        3 => {
            let expand = |nibble: u32| (nibble * 17) as u8;
            Some(Rgb::new(expand((value >> 8) & 0xF), expand((value >> 4) & 0xF), expand(value & 0xF)))
        }
        _ => None,
    }
}

const WINDOWS_TERMINAL_ANSI_KEYS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "purple",
    "cyan",
    "white",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightPurple",
    "brightCyan",
    "brightWhite",
];

/// Imports one scheme object, or every entry of a `schemes` array (the shape of Windows Terminal's `settings.json`).
pub fn from_windows_terminal_json(text: &str) -> Result<Vec<NamedScheme>, SchemeImportError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|parse| error(parse.to_string()))?;
    let objects: Vec<&serde_json::Value> = match value.get("schemes").and_then(serde_json::Value::as_array) {
        Some(list) => list.iter().collect(),
        None => vec![&value],
    };
    let mut schemes = Vec::new();
    for object in objects {
        schemes.push(windows_terminal_scheme(object)?);
    }
    if schemes.is_empty() {
        return Err(error("no schemes found"));
    }
    Ok(schemes)
}

fn windows_terminal_scheme(object: &serde_json::Value) -> Result<NamedScheme, SchemeImportError> {
    let name = object.get("name").and_then(serde_json::Value::as_str).ok_or_else(|| error("a scheme has no name"))?;
    let color = |key: &str| -> Result<Rgb, SchemeImportError> {
        let text = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| error(format!("`{name}` has no `{key}`")))?;
        parse_hex_color(text).ok_or_else(|| error(format!("`{name}`: `{key}` is not a color")))
    };
    let optional = |key: &str| object.get(key).and_then(serde_json::Value::as_str).and_then(parse_hex_color);
    let fallback = ColorScheme::campbell();
    let mut ansi = fallback.ansi;
    for (slot, key) in ansi.iter_mut().zip(WINDOWS_TERMINAL_ANSI_KEYS) {
        *slot = color(key)?;
    }
    let background = color("background")?;
    let foreground = color("foreground")?;
    Ok(NamedScheme {
        name: name.to_owned(),
        scheme: ColorScheme {
            foreground,
            background,
            cursor: optional("cursorColor").unwrap_or(foreground),
            selection_background: optional("selectionBackground").unwrap_or(fallback.selection_background),
            selection_foreground: None,
            ansi,
            bold_uses_bright_colors: fallback.bold_uses_bright_colors,
        },
    })
}

/// Imports a Konsole `.colorscheme` file. `file_stem` names the scheme when the file has no `Description`.
pub fn from_konsole_colorscheme(text: &str, file_stem: &str) -> Result<NamedScheme, SchemeImportError> {
    let mut section = String::new();
    let mut name = None;
    let mut background = None;
    let mut foreground = None;
    let mut ansi: [Option<Rgb>; 16] = [None; 16];
    for line in
        text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
    {
        if let Some(header) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            section = header.to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        match (section.as_str(), key.trim()) {
            ("General", "Description") => name = Some(value.trim().to_owned()),
            ("Background", "Color") => background = parse_konsole_color(value),
            ("Foreground", "Color") => foreground = parse_konsole_color(value),
            (other, "Color") => {
                if let Some(index) = konsole_ansi_index(other) {
                    ansi[index] = parse_konsole_color(value);
                }
            }
            _ => {}
        }
    }
    let background = background.ok_or_else(|| error("no [Background] color"))?;
    let foreground = foreground.ok_or_else(|| error("no [Foreground] color"))?;
    let fallback = ColorScheme::campbell();
    let mut resolved = fallback.ansi;
    for (slot, imported) in resolved.iter_mut().zip(ansi) {
        if let Some(color) = imported {
            *slot = color;
        }
    }
    Ok(NamedScheme {
        name: name.filter(|name| !name.is_empty()).unwrap_or_else(|| file_stem.to_owned()),
        scheme: ColorScheme {
            foreground,
            background,
            cursor: foreground,
            selection_background: fallback.selection_background,
            selection_foreground: None,
            ansi: resolved,
            bold_uses_bright_colors: fallback.bold_uses_bright_colors,
        },
    })
}

/// `[Color3]` is index 3, `[Color3Intense]` is 11; fainter variants are not representable and are ignored.
fn konsole_ansi_index(section: &str) -> Option<usize> {
    let rest = section.strip_prefix("Color")?;
    let (digits, offset) = match rest.strip_suffix("Intense") {
        Some(digits) => (digits, 8),
        None if rest.ends_with("Faint") => return None,
        None => (rest, 0),
    };
    let index: usize = digits.parse().ok()?;
    (index < 8).then_some(index + offset)
}

fn parse_konsole_color(text: &str) -> Option<Rgb> {
    let mut parts = text.split(',').map(|part| part.trim().parse::<u8>());
    let (red, green, blue) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    Some(Rgb::new(red, green, blue))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS_TERMINAL_SCHEME: &str = r##"{
        "name": "Ocean", "background": "#001122", "foreground": "#EEEEEE", "cursorColor": "#FFCC00", "selectionBackground": "#334455",
        "black": "#000000", "red": "#110000", "green": "#001100", "yellow": "#111100", "blue": "#000011", "purple": "#110011",
        "cyan": "#001111", "white": "#CCCCCC", "brightBlack": "#777777", "brightRed": "#FF0000", "brightGreen": "#00FF00",
        "brightYellow": "#FFFF00", "brightBlue": "#0000FF", "brightPurple": "#FF00FF", "brightCyan": "#00FFFF", "brightWhite": "#FFFFFF"
    }"##;

    #[test]
    fn given_the_built_in_catalog_then_campbell_and_three_others_are_present_and_found_case_insensitively() {
        let schemes = builtin_schemes();
        assert_eq!(schemes.len(), 4);
        assert_eq!(find(&schemes, "campbell").map(|scheme| scheme.scheme.clone()), Some(ColorScheme::campbell()));
        assert!(find(&schemes, " Solarized DARK ").is_some());
        assert!(find(&schemes, "nope").is_none());
    }

    #[test]
    fn given_an_imported_scheme_with_a_built_in_name_when_building_the_catalog_then_it_replaces_it() {
        let imported = NamedScheme { name: "campbell".to_owned(), scheme: builtin_schemes()[1].scheme.clone() };
        let schemes = catalog(vec![imported.clone()]);
        assert_eq!(schemes.len(), 4);
        assert_eq!(find(&schemes, "Campbell").map(|scheme| &scheme.scheme), Some(&imported.scheme));
    }

    #[test]
    fn given_hex_text_when_parsing_colors_then_six_and_three_digit_forms_work() {
        assert_eq!(parse_hex_color("#102030"), Some(Rgb::new(0x10, 0x20, 0x30)));
        assert_eq!(parse_hex_color("#fa0"), Some(Rgb::new(0xff, 0xaa, 0x00)));
        assert_eq!(parse_hex_color("102030"), None);
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#zzzzzz"), None);
    }

    #[test]
    fn given_a_windows_terminal_scheme_when_importing_then_all_colors_are_read() {
        let schemes = from_windows_terminal_json(WINDOWS_TERMINAL_SCHEME).expect("imports");
        assert_eq!(schemes.len(), 1);
        let scheme = &schemes[0];
        assert_eq!(scheme.name, "Ocean");
        assert_eq!(scheme.scheme.background, Rgb::new(0, 0x11, 0x22));
        assert_eq!(scheme.scheme.cursor, Rgb::new(0xff, 0xcc, 0));
        assert_eq!(scheme.scheme.selection_background, Rgb::new(0x33, 0x44, 0x55));
        assert_eq!(scheme.scheme.ansi[1], Rgb::new(0x11, 0, 0));
        assert_eq!(scheme.scheme.ansi[15], Rgb::new(255, 255, 255));
    }

    #[test]
    fn given_a_settings_file_with_a_schemes_array_when_importing_then_every_scheme_is_returned() {
        let text =
            format!(r#"{{"profiles": {{}}, "schemes": [{WINDOWS_TERMINAL_SCHEME}, {WINDOWS_TERMINAL_SCHEME}]}}"#);
        assert_eq!(from_windows_terminal_json(&text).expect("imports").len(), 2);
        assert!(from_windows_terminal_json(r#"{"schemes": []}"#).is_err());
    }

    #[test]
    fn given_an_incomplete_or_broken_windows_terminal_scheme_when_importing_then_the_problem_is_named() {
        let missing = WINDOWS_TERMINAL_SCHEME.replace("\"red\": \"#110000\",", "");
        assert!(from_windows_terminal_json(&missing).unwrap_err().reason.contains("red"));
        assert!(from_windows_terminal_json("not json").is_err());
        assert!(from_windows_terminal_json("{}").unwrap_err().reason.contains("name"));
        let bad_color = WINDOWS_TERMINAL_SCHEME.replace("#001122", "navy");
        assert!(from_windows_terminal_json(&bad_color).unwrap_err().reason.contains("background"));
    }

    #[test]
    fn given_a_windows_terminal_scheme_without_a_cursor_color_when_importing_then_the_foreground_is_used() {
        let text = WINDOWS_TERMINAL_SCHEME.replace("\"cursorColor\": \"#FFCC00\",", "");
        let scheme = &from_windows_terminal_json(&text).expect("imports")[0];
        assert_eq!(scheme.scheme.cursor, scheme.scheme.foreground);
    }

    const KONSOLE_SCHEME: &str = "[General]\nDescription=Breeze Mini\nOpacity=1\n\n[Background]\nColor=10,20,30\n\n[Foreground]\nColor=200,210,220\n\n[Color1]\nColor=255,0,0\n\n[Color1Intense]\nColor=255,128,128\n\n[Color1Faint]\nColor=1,1,1\n\n[Color7]\nColor=250,250,250\n";

    #[test]
    fn given_a_konsole_scheme_when_importing_then_normal_and_intense_colors_land_in_the_right_slots() {
        let scheme = from_konsole_colorscheme(KONSOLE_SCHEME, "breeze").expect("imports");
        assert_eq!(scheme.name, "Breeze Mini");
        assert_eq!(scheme.scheme.background, Rgb::new(10, 20, 30));
        assert_eq!(scheme.scheme.foreground, Rgb::new(200, 210, 220));
        assert_eq!(scheme.scheme.ansi[1], Rgb::new(255, 0, 0));
        assert_eq!(scheme.scheme.ansi[9], Rgb::new(255, 128, 128));
        assert_eq!(scheme.scheme.ansi[7], Rgb::new(250, 250, 250));
        assert_eq!(scheme.scheme.ansi[2], ColorScheme::campbell().ansi[2], "unlisted colors keep the default");
    }

    #[test]
    fn given_a_konsole_scheme_without_a_description_when_importing_then_the_file_name_is_used() {
        let text = KONSOLE_SCHEME.replace("Description=Breeze Mini\n", "");
        assert_eq!(from_konsole_colorscheme(&text, "my-scheme").expect("imports").name, "my-scheme");
    }

    #[test]
    fn given_a_konsole_scheme_missing_its_base_colors_when_importing_then_it_fails() {
        assert!(from_konsole_colorscheme("[General]\nDescription=x\n", "x").is_err());
        assert!(
            from_konsole_colorscheme("[Background]\nColor=1,2,3\n", "x").unwrap_err().reason.contains("Foreground")
        );
        let bad = KONSOLE_SCHEME.replace("10,20,30", "300,0,0");
        assert!(from_konsole_colorscheme(&bad, "x").is_err());
    }

    #[test]
    fn given_konsole_section_names_when_mapping_then_only_the_eight_base_colors_and_their_intense_forms_count() {
        assert_eq!(konsole_ansi_index("Color0"), Some(0));
        assert_eq!(konsole_ansi_index("Color7Intense"), Some(15));
        assert_eq!(konsole_ansi_index("Color8"), None);
        assert_eq!(konsole_ansi_index("Color2Faint"), None);
        assert_eq!(konsole_ansi_index("Other"), None);
    }

    #[test]
    fn given_an_import_error_when_displayed_then_it_says_what_failed() {
        assert!(error("boom").to_string().contains("boom"));
    }
}
