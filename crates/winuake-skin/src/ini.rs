//! Reader for the INI dialect of KDE's `KConfig::SimpleConfig`: `[Group]` headers and `key=value` lines. Keys are case sensitive,
//! values are not quoted, `#` and `;` start comment lines.

use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IniFile {
    groups: HashMap<String, HashMap<String, String>>,
}

impl IniFile {
    pub fn parse(text: &str) -> Self {
        let mut groups: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut current = String::new();
        for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(header) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                current = header.to_owned();
                groups.entry(current.clone()).or_default();
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                groups.entry(current.clone()).or_default().insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
        Self { groups }
    }

    pub fn group<'a>(&'a self, name: &str) -> IniGroup<'a> {
        IniGroup { entries: self.groups.get(name) }
    }

    pub fn has_group(&self, name: &str) -> bool {
        self.groups.contains_key(name)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IniGroup<'a> {
    entries: Option<&'a HashMap<String, String>>,
}

impl IniGroup<'_> {
    pub fn text(&self, key: &str, default: &str) -> String {
        self.entries.and_then(|entries| entries.get(key)).cloned().unwrap_or_else(|| default.to_owned())
    }

    pub fn int(&self, key: &str, default: i32) -> i32 {
        self.entries.and_then(|entries| entries.get(key)).and_then(|value| value.parse().ok()).unwrap_or(default)
    }

    /// `true`/`false` (case-insensitive); anything else keeps the default.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        match self.entries.and_then(|entries| entries.get(key)).map(|value| value.to_ascii_lowercase()).as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}# comment\n[Description]\nSkin = Breeze\nAuthor=Someone\n\n[Border]\nred=61\nwidth=2\nbold=TRUE\nnonsense=abc\n; other\n";

    #[test]
    fn given_ini_text_when_reading_then_groups_keys_and_values_are_found() {
        let file = IniFile::parse(SAMPLE);
        assert!(file.has_group("Description"));
        assert!(!file.has_group("Missing"));
        assert_eq!(file.group("Description").text("Skin", ""), "Breeze");
        assert_eq!(file.group("Description").text("Author", ""), "Someone");
    }

    #[test]
    fn given_missing_or_bad_values_when_reading_then_defaults_are_used() {
        let file = IniFile::parse(SAMPLE);
        let border = file.group("Border");
        assert_eq!(border.int("red", 0), 61);
        assert_eq!(border.int("green", 7), 7);
        assert_eq!(border.int("nonsense", 9), 9);
        assert_eq!(file.group("Nowhere").int("x", 3), 3);
        assert_eq!(file.group("Nowhere").text("x", "fallback"), "fallback");
    }

    #[test]
    fn given_flags_when_reading_then_true_and_false_are_case_insensitive_and_others_keep_the_default() {
        let border = IniFile::parse(SAMPLE);
        let border = border.group("Border");
        assert!(border.flag("bold", false));
        assert!(!border.flag("nonsense", false));
        assert!(border.flag("nonsense", true));
        assert!(border.flag("absent", true));
    }

    #[test]
    fn given_keys_before_any_group_when_reading_then_they_belong_to_the_unnamed_group() {
        let file = IniFile::parse("loose=1\n[A]\nb=2\n");
        assert_eq!(file.group("").int("loose", 0), 1);
        assert_eq!(file.group("A").int("b", 0), 2);
    }
}
