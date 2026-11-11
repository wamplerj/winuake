//! Windows virtual-key and character-event handling, as pure functions. The WinUI layer only forwards raw values.
//!
//! The split mirrors how Windows delivers typing: `KeyDown` carries a virtual key and modifier state (right for special keys
//! and Ctrl combinations), while `CharacterReceived` carries the text the keyboard layout produced (right for everything printable).

use crate::input::{Key, Modifiers};

pub const VK_BACK: i32 = 0x08;
pub const VK_TAB: i32 = 0x09;
pub const VK_RETURN: i32 = 0x0D;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const VK_MENU: i32 = 0x12;
pub const VK_ESCAPE: i32 = 0x1B;
pub const VK_SPACE: i32 = 0x20;
pub const VK_PRIOR: i32 = 0x21;
pub const VK_NEXT: i32 = 0x22;
pub const VK_END: i32 = 0x23;
pub const VK_HOME: i32 = 0x24;
pub const VK_LEFT: i32 = 0x25;
pub const VK_UP: i32 = 0x26;
pub const VK_RIGHT: i32 = 0x27;
pub const VK_DOWN: i32 = 0x28;
pub const VK_INSERT: i32 = 0x2D;
pub const VK_DELETE: i32 = 0x2E;
pub const VK_LWIN: i32 = 0x5B;
pub const VK_RWIN: i32 = 0x5C;
pub const VK_C: i32 = 0x43;
pub const VK_Q: i32 = 0x51;
pub const VK_F11: i32 = 0x7A;
pub const VK_V: i32 = 0x56;
pub const VK_F1: i32 = 0x70;
pub const VK_F12: i32 = 0x7B;
pub const VK_LSHIFT: i32 = 0xA0;
pub const VK_RMENU: i32 = 0xA5;
const VK_OEM_2: i32 = 0xBF;
const VK_OEM_4: i32 = 0xDB;
const VK_OEM_5: i32 = 0xDC;
const VK_OEM_6: i32 = 0xDD;

/// What the terminal view should do with a `KeyDown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDownAction {
    /// Send this key (with the modifiers) to the shell and mark the event handled.
    Send(Key),
    /// A modifier or Windows key on its own; nothing to do and not handled.
    Ignore,
    /// Printable input: wait for the character message, which knows the keyboard layout.
    AwaitCharacter,
    /// Copy the selection to the clipboard.
    Copy,
    /// Paste the clipboard.
    Paste,
}

pub fn decide_key_down(virtual_key: i32, modifiers: Modifiers) -> KeyDownAction {
    if is_modifier_key(virtual_key) {
        return KeyDownAction::Ignore;
    }
    if let Some(action) = clipboard_shortcut(virtual_key, modifiers) {
        return action;
    }
    if let Some(key) = special_key(virtual_key) {
        return KeyDownAction::Send(key);
    }
    // Ctrl without Alt: the layout produces a control character we cannot rely on, so encode it ourselves.
    // Ctrl+Alt together is AltGr on many layouts and yields real text, so that goes through the character path.
    if modifiers.control
        && !modifiers.alt
        && let Some(character) = control_combination_character(virtual_key)
    {
        return KeyDownAction::Send(Key::Character(character));
    }
    KeyDownAction::AwaitCharacter
}

/// Ctrl+Shift+C / Ctrl+Shift+V, Ctrl+Insert, Shift+Insert. Plain Ctrl+C and Ctrl+V still reach the shell.
fn clipboard_shortcut(virtual_key: i32, modifiers: Modifiers) -> Option<KeyDownAction> {
    match virtual_key {
        VK_C if modifiers.control && modifiers.shift && !modifiers.alt => Some(KeyDownAction::Copy),
        VK_V if modifiers.control && modifiers.shift && !modifiers.alt => Some(KeyDownAction::Paste),
        VK_INSERT if modifiers.control && !modifiers.shift && !modifiers.alt => Some(KeyDownAction::Copy),
        VK_INSERT if modifiers.shift && !modifiers.control && !modifiers.alt => Some(KeyDownAction::Paste),
        _ => None,
    }
}

pub fn is_modifier_key(virtual_key: i32) -> bool {
    matches!(virtual_key, VK_SHIFT..=VK_MENU | VK_LWIN | VK_RWIN | VK_LSHIFT..=VK_RMENU)
}

fn special_key(virtual_key: i32) -> Option<Key> {
    Some(match virtual_key {
        VK_RETURN => Key::Enter,
        VK_TAB => Key::Tab,
        VK_BACK => Key::Backspace,
        VK_ESCAPE => Key::Escape,
        VK_UP => Key::ArrowUp,
        VK_DOWN => Key::ArrowDown,
        VK_LEFT => Key::ArrowLeft,
        VK_RIGHT => Key::ArrowRight,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_INSERT => Key::Insert,
        VK_DELETE => Key::Delete,
        VK_F1..=VK_F12 => Key::Function((virtual_key - VK_F1 + 1) as u8),
        _ => return None,
    })
}

/// The base character of a key that can form a Ctrl combination.
fn control_combination_character(virtual_key: i32) -> Option<char> {
    match virtual_key {
        0x41..=0x5A => char::from_u32(virtual_key as u32 + 0x20),
        0x32..=0x38 => char::from_u32(virtual_key as u32),
        VK_SPACE => Some(' '),
        VK_OEM_2 => Some('_'),
        VK_OEM_4 => Some('['),
        VK_OEM_5 => Some('\\'),
        VK_OEM_6 => Some(']'),
        _ => None,
    }
}

/// Decides whether a received character is text to send, and with which modifiers.
/// Control characters are dropped because `KeyDown` already handled their keys.
pub fn text_for_character(character: char, modifiers: Modifiers) -> Option<(Key, Modifiers)> {
    if character.is_control() {
        return None;
    }
    // Ctrl without Alt never yields printable text; Alt alone prefixes ESC; Ctrl+Alt (AltGr) is plain text.
    if modifiers.control && !modifiers.alt {
        return None;
    }
    let forwarded = Modifiers { shift: false, control: false, alt: modifiers.alt && !modifiers.control };
    Some((Key::Character(character), forwarded))
}

/// Reassembles characters delivered as UTF-16 code units (surrogate pairs arrive as two events).
#[derive(Debug, Default)]
pub struct CharacterAssembler {
    pending_high_surrogate: Option<u16>,
}

impl CharacterAssembler {
    pub fn push(&mut self, unit: u16) -> Option<char> {
        match (self.pending_high_surrogate.take(), unit) {
            (Some(high), 0xDC00..=0xDFFF) => char::decode_utf16([high, unit]).next().and_then(Result::ok),
            (_, 0xD800..=0xDBFF) => {
                self.pending_high_surrogate = Some(unit);
                None
            }
            (_, 0xDC00..=0xDFFF) => None,
            (_, unit) => char::from_u32(u32::from(unit)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers { shift: false, control: true, alt: false };
    const ALT: Modifiers = Modifiers { shift: false, control: false, alt: true };
    const CTRL_SHIFT: Modifiers = Modifiers { shift: true, control: true, alt: false };
    const ALT_GR: Modifiers = Modifiers { shift: false, control: true, alt: true };

    #[test]
    fn given_special_keys_when_pressed_then_they_are_sent_regardless_of_modifiers() {
        assert_eq!(decide_key_down(VK_RETURN, Modifiers::NONE), KeyDownAction::Send(Key::Enter));
        assert_eq!(decide_key_down(VK_UP, CTRL), KeyDownAction::Send(Key::ArrowUp));
        assert_eq!(decide_key_down(VK_BACK, Modifiers::NONE), KeyDownAction::Send(Key::Backspace));
        assert_eq!(decide_key_down(VK_PRIOR, Modifiers::NONE), KeyDownAction::Send(Key::PageUp));
        assert_eq!(decide_key_down(VK_DELETE, ALT), KeyDownAction::Send(Key::Delete));
    }

    #[test]
    fn given_clipboard_shortcuts_when_pressed_then_copy_and_paste_actions_are_chosen() {
        assert_eq!(decide_key_down(VK_C, CTRL_SHIFT), KeyDownAction::Copy);
        assert_eq!(decide_key_down(VK_V, CTRL_SHIFT), KeyDownAction::Paste);
        assert_eq!(decide_key_down(VK_INSERT, CTRL), KeyDownAction::Copy);
        assert_eq!(decide_key_down(VK_INSERT, Modifiers { shift: true, ..Modifiers::NONE }), KeyDownAction::Paste);
    }

    #[test]
    fn given_plain_arrows_or_other_modifier_combinations_when_pressed_then_the_shell_gets_them() {
        assert_eq!(decide_key_down(VK_RIGHT, Modifiers::NONE), KeyDownAction::Send(Key::ArrowRight));
        assert_eq!(decide_key_down(VK_RIGHT, ALT), KeyDownAction::Send(Key::ArrowRight));
        assert_eq!(decide_key_down(VK_RIGHT, CTRL), KeyDownAction::Send(Key::ArrowRight));
        assert_eq!(decide_key_down(VK_Q, CTRL), KeyDownAction::Send(Key::Character('q')));
    }

    #[test]
    fn given_plain_ctrl_c_and_ctrl_v_when_pressed_then_they_still_go_to_the_shell() {
        assert_eq!(decide_key_down(VK_C, CTRL), KeyDownAction::Send(Key::Character('c')));
        assert_eq!(decide_key_down(VK_V, CTRL), KeyDownAction::Send(Key::Character('v')));
        assert_eq!(decide_key_down(VK_INSERT, Modifiers::NONE), KeyDownAction::Send(Key::Insert));
    }

    #[test]
    fn given_function_keys_when_pressed_then_their_numbers_are_sent() {
        assert_eq!(decide_key_down(VK_F1, Modifiers::NONE), KeyDownAction::Send(Key::Function(1)));
        assert_eq!(decide_key_down(VK_F12, Modifiers::NONE), KeyDownAction::Send(Key::Function(12)));
        assert_eq!(decide_key_down(0x7C, Modifiers::NONE), KeyDownAction::AwaitCharacter, "F13 is not supported");
    }

    #[test]
    fn given_modifier_keys_alone_when_pressed_then_they_are_ignored() {
        for virtual_key in [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN, VK_LSHIFT, VK_RMENU, 0xA2] {
            assert_eq!(decide_key_down(virtual_key, Modifiers::NONE), KeyDownAction::Ignore, "{virtual_key:#x}");
        }
    }

    #[test]
    fn given_ctrl_with_a_letter_when_pressed_then_the_lowercase_character_is_sent_for_encoding() {
        assert_eq!(decide_key_down(0x43, CTRL), KeyDownAction::Send(Key::Character('c')));
        assert_eq!(decide_key_down(0x5A, CTRL), KeyDownAction::Send(Key::Character('z')));
    }

    #[test]
    fn given_ctrl_with_punctuation_when_pressed_then_the_matching_character_is_sent() {
        assert_eq!(decide_key_down(VK_SPACE, CTRL), KeyDownAction::Send(Key::Character(' ')));
        assert_eq!(decide_key_down(VK_OEM_4, CTRL), KeyDownAction::Send(Key::Character('[')));
        assert_eq!(decide_key_down(VK_OEM_5, CTRL), KeyDownAction::Send(Key::Character('\\')));
        assert_eq!(decide_key_down(VK_OEM_6, CTRL), KeyDownAction::Send(Key::Character(']')));
        assert_eq!(decide_key_down(VK_OEM_2, CTRL), KeyDownAction::Send(Key::Character('_')));
    }

    #[test]
    fn given_plain_or_alt_letters_when_pressed_then_the_character_event_decides() {
        assert_eq!(decide_key_down(0x41, Modifiers::NONE), KeyDownAction::AwaitCharacter);
        assert_eq!(decide_key_down(0x41, ALT), KeyDownAction::AwaitCharacter);
        assert_eq!(decide_key_down(0x41, Modifiers { shift: true, ..Modifiers::NONE }), KeyDownAction::AwaitCharacter);
    }

    #[test]
    fn given_altgr_when_a_letter_is_pressed_then_the_character_event_decides() {
        assert_eq!(decide_key_down(0x45, ALT_GR), KeyDownAction::AwaitCharacter);
    }

    #[test]
    fn given_control_characters_when_received_then_they_are_dropped() {
        assert_eq!(text_for_character('\r', Modifiers::NONE), None);
        assert_eq!(text_for_character('\u{3}', CTRL), None);
        assert_eq!(text_for_character('\u{7f}', Modifiers::NONE), None);
    }

    #[test]
    fn given_printable_text_when_received_then_it_is_sent_without_modifiers() {
        assert_eq!(text_for_character('a', Modifiers::NONE), Some((Key::Character('a'), Modifiers::NONE)));
        assert_eq!(
            text_for_character('A', Modifiers { shift: true, ..Modifiers::NONE }),
            Some((Key::Character('A'), Modifiers::NONE))
        );
        assert_eq!(text_for_character('世', Modifiers::NONE), Some((Key::Character('世'), Modifiers::NONE)));
    }

    #[test]
    fn given_alt_with_text_when_received_then_alt_is_forwarded_for_the_escape_prefix() {
        assert_eq!(text_for_character('b', ALT), Some((Key::Character('b'), ALT)));
    }

    #[test]
    fn given_altgr_text_when_received_then_it_is_plain_text() {
        assert_eq!(text_for_character('@', ALT_GR), Some((Key::Character('@'), Modifiers::NONE)));
    }

    #[test]
    fn given_ctrl_with_printable_text_when_received_then_it_is_dropped() {
        assert_eq!(text_for_character('c', CTRL), None);
    }

    #[test]
    fn given_bmp_character_when_assembling_then_it_is_returned_immediately() {
        assert_eq!(CharacterAssembler::default().push(0x20AC), Some('€'));
    }

    #[test]
    fn given_a_surrogate_pair_when_assembling_then_the_character_is_returned_after_the_second_unit() {
        let mut assembler = CharacterAssembler::default();
        assert_eq!(assembler.push(0xD83D), None);
        assert_eq!(assembler.push(0xDE00), Some('😀'));
    }

    #[test]
    fn given_a_lone_low_surrogate_when_assembling_then_it_is_discarded() {
        assert_eq!(CharacterAssembler::default().push(0xDE00), None);
    }

    #[test]
    fn given_a_high_surrogate_followed_by_a_normal_character_when_assembling_then_the_orphan_is_dropped() {
        let mut assembler = CharacterAssembler::default();
        assert_eq!(assembler.push(0xD83D), None);
        assert_eq!(assembler.push(u16::from(b'a')), Some('a'));
    }
}
