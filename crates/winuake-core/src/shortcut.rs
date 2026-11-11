// Ported from yakuake app/mainwindow.cpp (setupActions default shortcuts) @21c6210
//! Key chords such as `Ctrl+Shift+T`: parsing, display, and conversion from a Windows virtual-key event.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChordModifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChordKey {
    /// Upper-case `A`..`Z`.
    Letter(char),
    /// `0`..`9` on the main row.
    Digit(u8),
    /// `F1`..`F24`.
    Function(u8),
    Left,
    Right,
    Up,
    Down,
    Tab,
    ParenLeft,
    ParenRight,
    Asterisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub modifiers: ChordModifiers,
    pub key: ChordKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordParseError {
    pub chord: String,
}

impl fmt::Display for ChordParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot parse key chord {:?}", self.chord)
    }
}

impl std::error::Error for ChordParseError {}

// Windows virtual-key codes (winuser.h).
const VK_TAB: i32 = 0x09;
const VK_LEFT: i32 = 0x25;
const VK_UP: i32 = 0x26;
const VK_RIGHT: i32 = 0x27;
const VK_DOWN: i32 = 0x28;
const VK_DIGIT_0: i32 = 0x30;
const VK_DIGIT_9: i32 = 0x39;
const VK_LETTER_A: i32 = 0x41;
const VK_LETTER_Z: i32 = 0x5A;
const VK_NUMPAD_MULTIPLY: i32 = 0x6A;
const VK_F1: i32 = 0x70;
const VK_F24: i32 = 0x87;

impl Chord {
    pub const fn new(control: bool, shift: bool, alt: bool, key: ChordKey) -> Self {
        Self { modifiers: ChordModifiers { control, shift, alt }, key }
    }

    /// Builds the chord for a key-down. Like Qt, shifted symbols are folded into the symbol: Shift+9 is `(`, Shift+0 is `)`,
    /// Shift+8 is `*` (US layout positions, since Windows reports the physical key). Returns `None` for keys no chord uses.
    pub fn from_virtual_key(virtual_key: i32, modifiers: ChordModifiers) -> Option<Self> {
        let key = match virtual_key {
            VK_TAB => ChordKey::Tab,
            VK_LEFT => ChordKey::Left,
            VK_RIGHT => ChordKey::Right,
            VK_UP => ChordKey::Up,
            VK_DOWN => ChordKey::Down,
            VK_NUMPAD_MULTIPLY => ChordKey::Asterisk,
            VK_LETTER_A..=VK_LETTER_Z => ChordKey::Letter(char::from(virtual_key as u8)),
            VK_DIGIT_0..=VK_DIGIT_9 => ChordKey::Digit((virtual_key - VK_DIGIT_0) as u8),
            VK_F1..=VK_F24 => ChordKey::Function((virtual_key - VK_F1 + 1) as u8),
            _ => return None,
        };
        let (key, modifiers) = match (key, modifiers.shift) {
            (ChordKey::Digit(9), true) => (ChordKey::ParenLeft, ChordModifiers { shift: false, ..modifiers }),
            (ChordKey::Digit(0), true) => (ChordKey::ParenRight, ChordModifiers { shift: false, ..modifiers }),
            (ChordKey::Digit(8), true) => (ChordKey::Asterisk, ChordModifiers { shift: false, ..modifiers }),
            _ => (key, modifiers),
        };
        Some(Self { modifiers, key })
    }

    /// Parses `Ctrl+Shift+T`, `Alt+1`, `F12`, `Ctrl+Tab`, `Ctrl+(`. Modifier and key names are case-insensitive.
    pub fn parse(text: &str) -> Result<Self, ChordParseError> {
        let error = || ChordParseError { chord: text.to_owned() };
        let mut modifiers = ChordModifiers::default();
        let mut parts: Vec<&str> = text.split('+').collect();
        // A literal "+" key would split into two empty parts; none of Yakuake's chords use it.
        let key_text = parts.pop().filter(|part| !part.is_empty()).ok_or_else(error)?;
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.control = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                _ => return Err(error()),
            }
        }
        let key = Self::parse_key(key_text).ok_or_else(error)?;
        Ok(Self { modifiers, key })
    }

    fn parse_key(text: &str) -> Option<ChordKey> {
        let lower = text.to_ascii_lowercase();
        match lower.as_str() {
            "left" => return Some(ChordKey::Left),
            "right" => return Some(ChordKey::Right),
            "up" => return Some(ChordKey::Up),
            "down" => return Some(ChordKey::Down),
            "tab" => return Some(ChordKey::Tab),
            "(" => return Some(ChordKey::ParenLeft),
            ")" => return Some(ChordKey::ParenRight),
            "*" => return Some(ChordKey::Asterisk),
            _ => {}
        }
        if let Some(number) = lower.strip_prefix('f').and_then(|digits| digits.parse::<u8>().ok()) {
            return (1..=24).contains(&number).then_some(ChordKey::Function(number));
        }
        let mut characters = text.chars();
        let (first, rest) = (characters.next()?, characters.next());
        match (first, rest) {
            (letter, None) if letter.is_ascii_alphabetic() => Some(ChordKey::Letter(letter.to_ascii_uppercase())),
            (digit, None) if digit.is_ascii_digit() => Some(ChordKey::Digit(digit as u8 - b'0')),
            _ => None,
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.control {
            formatter.write_str("Ctrl+")?;
        }
        if self.modifiers.alt {
            formatter.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            formatter.write_str("Shift+")?;
        }
        match self.key {
            ChordKey::Letter(letter) => write!(formatter, "{letter}"),
            ChordKey::Digit(digit) => write!(formatter, "{digit}"),
            ChordKey::Function(number) => write!(formatter, "F{number}"),
            ChordKey::Left => formatter.write_str("Left"),
            ChordKey::Right => formatter.write_str("Right"),
            ChordKey::Up => formatter.write_str("Up"),
            ChordKey::Down => formatter.write_str("Down"),
            ChordKey::Tab => formatter.write_str("Tab"),
            ChordKey::ParenLeft => formatter.write_str("("),
            ChordKey::ParenRight => formatter.write_str(")"),
            ChordKey::Asterisk => formatter.write_str("*"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: ChordModifiers = ChordModifiers { control: true, shift: false, alt: false };
    const CTRL_SHIFT: ChordModifiers = ChordModifiers { control: true, shift: true, alt: false };

    #[test]
    fn given_text_when_parsing_then_modifiers_and_key_are_recognised() {
        assert_eq!(Chord::parse("Ctrl+Shift+T"), Ok(Chord::new(true, true, false, ChordKey::Letter('T'))));
        assert_eq!(Chord::parse("alt+shift+right"), Ok(Chord::new(false, true, true, ChordKey::Right)));
        assert_eq!(Chord::parse("F12"), Ok(Chord::new(false, false, false, ChordKey::Function(12))));
        assert_eq!(Chord::parse("Ctrl+Alt+S"), Ok(Chord::new(true, false, true, ChordKey::Letter('S'))));
        assert_eq!(Chord::parse("Alt+1"), Ok(Chord::new(false, false, true, ChordKey::Digit(1))));
        assert_eq!(Chord::parse("Ctrl+Tab"), Ok(Chord::new(true, false, false, ChordKey::Tab)));
        assert_eq!(Chord::parse("Ctrl+("), Ok(Chord::new(true, false, false, ChordKey::ParenLeft)));
        assert_eq!(Chord::parse("Ctrl+)"), Ok(Chord::new(true, false, false, ChordKey::ParenRight)));
        assert_eq!(Chord::parse("Ctrl+*"), Ok(Chord::new(true, false, false, ChordKey::Asterisk)));
    }

    #[test]
    fn given_bad_text_when_parsing_then_an_error_names_it() {
        for bad in ["", "Ctrl+", "Hyper+T", "Ctrl+Banana", "F99", "Ctrl+Shift"] {
            assert_eq!(Chord::parse(bad), Err(ChordParseError { chord: bad.to_owned() }), "{bad}");
        }
        assert!(Chord::parse("Ctrl+").unwrap_err().to_string().contains("Ctrl+"));
    }

    #[test]
    fn given_any_chord_when_displayed_then_parsing_it_back_gives_the_same_chord() {
        for text in ["Ctrl+Shift+F11", "Alt+Shift+Down", "Ctrl+Alt+Left", "Shift+Up", "Ctrl+(", "Ctrl+*", "Alt+0", "F1"]
        {
            let chord = Chord::parse(text).expect(text);
            assert_eq!(chord.to_string(), text);
        }
    }

    #[test]
    fn given_virtual_keys_when_converting_then_letters_digits_arrows_and_function_keys_map() {
        assert_eq!(
            Chord::from_virtual_key(0x54, CTRL_SHIFT),
            Some(Chord::new(true, true, false, ChordKey::Letter('T')))
        );
        assert_eq!(Chord::from_virtual_key(0x31, CTRL), Some(Chord::new(true, false, false, ChordKey::Digit(1))));
        assert_eq!(Chord::from_virtual_key(0x25, CTRL), Some(Chord::new(true, false, false, ChordKey::Left)));
        assert_eq!(
            Chord::from_virtual_key(0x7B, ChordModifiers::default()),
            Some(Chord::new(false, false, false, ChordKey::Function(12)))
        );
        assert_eq!(Chord::from_virtual_key(0x09, CTRL), Some(Chord::new(true, false, false, ChordKey::Tab)));
        assert_eq!(Chord::from_virtual_key(0x26, CTRL), Some(Chord::new(true, false, false, ChordKey::Up)));
        assert_eq!(Chord::from_virtual_key(0x27, CTRL), Some(Chord::new(true, false, false, ChordKey::Right)));
        assert_eq!(Chord::from_virtual_key(0x28, CTRL), Some(Chord::new(true, false, false, ChordKey::Down)));
    }

    #[test]
    fn given_shifted_digits_when_converting_then_they_become_the_symbols_like_qt() {
        assert_eq!(
            Chord::from_virtual_key(0x39, CTRL_SHIFT),
            Some(Chord::new(true, false, false, ChordKey::ParenLeft))
        );
        assert_eq!(
            Chord::from_virtual_key(0x30, CTRL_SHIFT),
            Some(Chord::new(true, false, false, ChordKey::ParenRight))
        );
        assert_eq!(Chord::from_virtual_key(0x38, CTRL_SHIFT), Some(Chord::new(true, false, false, ChordKey::Asterisk)));
        assert_eq!(Chord::from_virtual_key(0x6A, CTRL), Some(Chord::new(true, false, false, ChordKey::Asterisk)));
    }

    #[test]
    fn given_keys_no_chord_uses_when_converting_then_none() {
        assert_eq!(Chord::from_virtual_key(0x1B, CTRL), None);
        assert_eq!(Chord::from_virtual_key(0x0D, CTRL), None);
    }
}
