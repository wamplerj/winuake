//! Turns user input (keys, paste, mouse, focus) into the VT byte sequences a shell expects. xterm-compatible; pure.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self { shift: false, control: false, alt: false };

    pub fn any(self) -> bool {
        self.shift || self.control || self.alt
    }

    /// xterm modifier parameter: 1 + shift(1) + alt(2) + control(4).
    fn xterm_parameter(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.control)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Text produced by the keyboard layout (already reflects Shift / AltGr).
    Character(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// F1..=F12.
    Function(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseReporting {
    #[default]
    Off,
    /// Press and release only.
    Click,
    /// Press, release, and motion while a button is held.
    Drag,
    /// Everything, including motion without buttons.
    Motion,
}

/// Terminal mode flags that change how input is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputModes {
    pub application_cursor_keys: bool,
    pub bracketed_paste: bool,
    pub mouse_reporting: MouseReporting,
    pub sgr_mouse: bool,
    pub focus_reporting: bool,
    pub alternate_scroll: bool,
    pub alternate_screen: bool,
}

pub fn encode_key(key: Key, modifiers: Modifiers, modes: &InputModes) -> Option<Vec<u8>> {
    match key {
        Key::Character(character) => encode_character(character, modifiers),
        Key::Enter => Some(with_alt_prefix(b"\r", modifiers)),
        Key::Tab if modifiers.shift => Some(b"\x1b[Z".to_vec()),
        Key::Tab => Some(with_alt_prefix(b"\t", modifiers)),
        Key::Backspace if modifiers.control => Some(with_alt_prefix(&[0x08], modifiers)),
        Key::Backspace => Some(with_alt_prefix(&[0x7f], modifiers)),
        Key::Escape => Some(with_alt_prefix(&[0x1b], modifiers)),
        Key::ArrowUp => Some(cursor_key(b'A', modifiers, modes)),
        Key::ArrowDown => Some(cursor_key(b'B', modifiers, modes)),
        Key::ArrowRight => Some(cursor_key(b'C', modifiers, modes)),
        Key::ArrowLeft => Some(cursor_key(b'D', modifiers, modes)),
        Key::Home => Some(cursor_key(b'H', modifiers, modes)),
        Key::End => Some(cursor_key(b'F', modifiers, modes)),
        Key::Insert => Some(tilde_key(2, modifiers)),
        Key::Delete => Some(tilde_key(3, modifiers)),
        Key::PageUp => Some(tilde_key(5, modifiers)),
        Key::PageDown => Some(tilde_key(6, modifiers)),
        Key::Function(number) => encode_function_key(number, modifiers),
    }
}

/// Text to send for a paste. Newlines become carriage returns, as if typed; bracketed paste wraps and sanitizes.
pub fn encode_paste(text: &str, modes: &InputModes) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if !modes.bracketed_paste {
        return normalized.into_bytes();
    }
    // A pasted end-marker would let clipboard content escape the bracket and run as typed input.
    let sanitized = normalized.replace("\x1b[201~", "").replace("\x1b[200~", "");
    let mut bytes = Vec::with_capacity(sanitized.len() + 12);
    bytes.extend_from_slice(b"\x1b[200~");
    bytes.extend_from_slice(sanitized.as_bytes());
    bytes.extend_from_slice(b"\x1b[201~");
    bytes
}

pub fn encode_focus(focused: bool, modes: &InputModes) -> Option<Vec<u8>> {
    modes.focus_reporting.then(|| if focused { b"\x1b[I".to_vec() } else { b"\x1b[O".to_vec() })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    fn code(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press(MouseButton),
    Release(MouseButton),
    /// Pointer moved; the held button, if any.
    Move(Option<MouseButton>),
    WheelUp,
    WheelDown,
}

/// Zero-based cell under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPosition {
    pub column: u16,
    pub row: u16,
}

/// `None` when the application did not ask for this kind of event, or the position cannot be encoded.
pub fn encode_mouse(
    action: MouseAction,
    position: CellPosition,
    modifiers: Modifiers,
    modes: &InputModes,
) -> Option<Vec<u8>> {
    if !is_reported(action, modes.mouse_reporting) {
        return None;
    }

    let released = matches!(action, MouseAction::Release(_));
    let mut code = match action {
        MouseAction::Press(button) => button.code(),
        MouseAction::Release(button) if modes.sgr_mouse => button.code(),
        MouseAction::Release(_) => 3,
        MouseAction::Move(Some(button)) => button.code() + 32,
        MouseAction::Move(None) => 3 + 32,
        MouseAction::WheelUp => 64,
        MouseAction::WheelDown => 65,
    };
    code += 4 * u8::from(modifiers.shift) + 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control);

    let column = u32::from(position.column) + 1;
    let row = u32::from(position.row) + 1;
    if modes.sgr_mouse {
        let final_byte = if released { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{column};{row}{final_byte}").into_bytes());
    }

    let (encoded_column, encoded_row) = (column + 32, row + 32);
    if encoded_column > 255 || encoded_row > 255 {
        return None;
    }
    Some(vec![0x1b, b'[', b'M', 32 + code, encoded_column as u8, encoded_row as u8])
}

/// Wheel over the alternate screen with no mouse reporting scrolls the application with arrow keys (`alternate scroll`).
/// Positive `lines` scrolls up.
pub fn encode_wheel_as_arrows(lines: i32, modes: &InputModes) -> Option<Vec<u8>> {
    if !(modes.alternate_screen && modes.alternate_scroll) || modes.mouse_reporting != MouseReporting::Off || lines == 0
    {
        return None;
    }
    let key = if lines > 0 { Key::ArrowUp } else { Key::ArrowDown };
    let sequence = encode_key(key, Modifiers::NONE, modes)?;
    Some(sequence.repeat(lines.unsigned_abs() as usize))
}

fn is_reported(action: MouseAction, reporting: MouseReporting) -> bool {
    match (action, reporting) {
        (_, MouseReporting::Off) => false,
        (MouseAction::Move(None), reporting) => reporting == MouseReporting::Motion,
        (MouseAction::Move(Some(_)), reporting) => matches!(reporting, MouseReporting::Drag | MouseReporting::Motion),
        _ => true,
    }
}

fn encode_character(character: char, modifiers: Modifiers) -> Option<Vec<u8>> {
    let base = if modifiers.control { control_code(character).map(|code| vec![code]) } else { None };
    let bytes = base.unwrap_or_else(|| character.to_string().into_bytes());
    Some(with_alt_prefix(&bytes, modifiers))
}

fn control_code(character: char) -> Option<u8> {
    match character {
        'a'..='z' => Some(character as u8 - b'a' + 1),
        'A'..='Z' => Some(character as u8 - b'A' + 1),
        '@' | ' ' | '2' => Some(0x00),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '7' | '-' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

fn with_alt_prefix(bytes: &[u8], modifiers: Modifiers) -> Vec<u8> {
    let mut sequence = Vec::with_capacity(bytes.len() + 1);
    if modifiers.alt {
        sequence.push(0x1b);
    }
    sequence.extend_from_slice(bytes);
    sequence
}

/// Arrows, Home, End: `ESC [ X`, `ESC O X` in application mode, `ESC [ 1 ; m X` with modifiers.
fn cursor_key(final_byte: u8, modifiers: Modifiers, modes: &InputModes) -> Vec<u8> {
    if modifiers.any() {
        return format!("\x1b[1;{}{}", modifiers.xterm_parameter(), final_byte as char).into_bytes();
    }
    let introducer = if modes.application_cursor_keys { 'O' } else { '[' };
    format!("\x1b{introducer}{}", final_byte as char).into_bytes()
}

/// Insert, Delete, PageUp, PageDown, F5..F12: `ESC [ n ~`, `ESC [ n ; m ~` with modifiers.
fn tilde_key(number: u8, modifiers: Modifiers) -> Vec<u8> {
    if modifiers.any() {
        format!("\x1b[{number};{}~", modifiers.xterm_parameter()).into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

fn encode_function_key(number: u8, modifiers: Modifiers) -> Option<Vec<u8>> {
    match number {
        1..=4 => {
            let final_byte = (b'P' + number - 1) as char;
            Some(if modifiers.any() {
                format!("\x1b[1;{}{final_byte}", modifiers.xterm_parameter()).into_bytes()
            } else {
                format!("\x1bO{final_byte}").into_bytes()
            })
        }
        5..=12 => {
            let code = match number {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                _ => 24,
            };
            Some(tilde_key(code, modifiers))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: InputModes = InputModes {
        application_cursor_keys: false,
        bracketed_paste: false,
        mouse_reporting: MouseReporting::Off,
        sgr_mouse: false,
        focus_reporting: false,
        alternate_scroll: false,
        alternate_screen: false,
    };

    const CTRL: Modifiers = Modifiers { shift: false, control: true, alt: false };
    const ALT: Modifiers = Modifiers { shift: false, control: false, alt: true };
    const SHIFT: Modifiers = Modifiers { shift: true, control: false, alt: false };
    const CTRL_SHIFT: Modifiers = Modifiers { shift: true, control: true, alt: false };

    fn key(key: Key, modifiers: Modifiers) -> Vec<u8> {
        encode_key(key, modifiers, &NORMAL).expect("key is encodable")
    }

    fn mouse_modes(reporting: MouseReporting, sgr: bool) -> InputModes {
        InputModes { mouse_reporting: reporting, sgr_mouse: sgr, ..NORMAL }
    }

    fn at(column: u16, row: u16) -> CellPosition {
        CellPosition { column, row }
    }

    #[test]
    fn given_printable_characters_when_encoding_then_utf8_is_sent() {
        assert_eq!(key(Key::Character('a'), Modifiers::NONE), b"a");
        assert_eq!(key(Key::Character('A'), SHIFT), b"A");
        assert_eq!(key(Key::Character('é'), Modifiers::NONE), "é".as_bytes());
        assert_eq!(key(Key::Character('世'), Modifiers::NONE), "世".as_bytes());
    }

    #[test]
    fn given_control_with_letters_when_encoding_then_control_codes_are_sent() {
        assert_eq!(key(Key::Character('c'), CTRL), [0x03]);
        assert_eq!(key(Key::Character('C'), CTRL_SHIFT), [0x03]);
        assert_eq!(key(Key::Character('z'), CTRL), [0x1a]);
        assert_eq!(key(Key::Character('d'), CTRL), [0x04]);
    }

    #[test]
    fn given_control_with_punctuation_when_encoding_then_matching_control_codes_are_sent() {
        assert_eq!(key(Key::Character('['), CTRL), [0x1b]);
        assert_eq!(key(Key::Character('\\'), CTRL), [0x1c]);
        assert_eq!(key(Key::Character(']'), CTRL), [0x1d]);
        assert_eq!(key(Key::Character(' '), CTRL), [0x00]);
        assert_eq!(key(Key::Character('?'), CTRL), [0x7f]);
    }

    #[test]
    fn given_control_with_unmapped_character_when_encoding_then_the_character_is_sent_unchanged() {
        assert_eq!(key(Key::Character('1'), CTRL), b"1");
    }

    #[test]
    fn given_alt_with_character_when_encoding_then_escape_prefix_is_added() {
        assert_eq!(key(Key::Character('x'), ALT), b"\x1bx");
        assert_eq!(key(Key::Character('b'), Modifiers { control: true, ..ALT }), b"\x1b\x02");
    }

    #[test]
    fn given_editing_keys_when_encoding_then_standard_bytes_are_sent() {
        assert_eq!(key(Key::Enter, Modifiers::NONE), b"\r");
        assert_eq!(key(Key::Enter, ALT), b"\x1b\r");
        assert_eq!(key(Key::Tab, Modifiers::NONE), b"\t");
        assert_eq!(key(Key::Tab, SHIFT), b"\x1b[Z");
        assert_eq!(key(Key::Backspace, Modifiers::NONE), [0x7f]);
        assert_eq!(key(Key::Backspace, CTRL), [0x08]);
        assert_eq!(key(Key::Escape, Modifiers::NONE), [0x1b]);
    }

    #[test]
    fn given_arrows_when_encoding_in_normal_mode_then_csi_sequences_are_sent() {
        assert_eq!(key(Key::ArrowUp, Modifiers::NONE), b"\x1b[A");
        assert_eq!(key(Key::ArrowDown, Modifiers::NONE), b"\x1b[B");
        assert_eq!(key(Key::ArrowRight, Modifiers::NONE), b"\x1b[C");
        assert_eq!(key(Key::ArrowLeft, Modifiers::NONE), b"\x1b[D");
        assert_eq!(key(Key::Home, Modifiers::NONE), b"\x1b[H");
        assert_eq!(key(Key::End, Modifiers::NONE), b"\x1b[F");
    }

    #[test]
    fn given_application_cursor_mode_when_encoding_arrows_then_ss3_sequences_are_sent() {
        let modes = InputModes { application_cursor_keys: true, ..NORMAL };
        assert_eq!(encode_key(Key::ArrowUp, Modifiers::NONE, &modes).unwrap(), b"\x1bOA");
        assert_eq!(encode_key(Key::Home, Modifiers::NONE, &modes).unwrap(), b"\x1bOH");
    }

    #[test]
    fn given_modifiers_when_encoding_arrows_then_xterm_modifier_parameter_is_used_even_in_application_mode() {
        let modes = InputModes { application_cursor_keys: true, ..NORMAL };
        assert_eq!(key(Key::ArrowRight, CTRL), b"\x1b[1;5C");
        assert_eq!(key(Key::ArrowLeft, SHIFT), b"\x1b[1;2D");
        assert_eq!(key(Key::ArrowUp, ALT), b"\x1b[1;3A");
        assert_eq!(encode_key(Key::ArrowUp, CTRL_SHIFT, &modes).unwrap(), b"\x1b[1;6A");
    }

    #[test]
    fn given_navigation_keys_when_encoding_then_tilde_sequences_are_sent() {
        assert_eq!(key(Key::Insert, Modifiers::NONE), b"\x1b[2~");
        assert_eq!(key(Key::Delete, Modifiers::NONE), b"\x1b[3~");
        assert_eq!(key(Key::PageUp, Modifiers::NONE), b"\x1b[5~");
        assert_eq!(key(Key::PageDown, Modifiers::NONE), b"\x1b[6~");
        assert_eq!(key(Key::Delete, CTRL), b"\x1b[3;5~");
    }

    #[test]
    fn given_function_keys_when_encoding_then_xterm_sequences_are_sent() {
        assert_eq!(key(Key::Function(1), Modifiers::NONE), b"\x1bOP");
        assert_eq!(key(Key::Function(4), Modifiers::NONE), b"\x1bOS");
        assert_eq!(key(Key::Function(5), Modifiers::NONE), b"\x1b[15~");
        assert_eq!(key(Key::Function(10), Modifiers::NONE), b"\x1b[21~");
        assert_eq!(key(Key::Function(12), Modifiers::NONE), b"\x1b[24~");
        assert_eq!(key(Key::Function(1), SHIFT), b"\x1b[1;2P");
        assert_eq!(key(Key::Function(5), CTRL), b"\x1b[15;5~");
    }

    #[test]
    fn given_unsupported_function_key_when_encoding_then_nothing_is_sent() {
        assert_eq!(encode_key(Key::Function(13), Modifiers::NONE, &NORMAL), None);
        assert_eq!(encode_key(Key::Function(0), Modifiers::NONE, &NORMAL), None);
    }

    #[test]
    fn given_multiline_paste_when_not_bracketed_then_newlines_become_carriage_returns() {
        assert_eq!(encode_paste("a\r\nb\nc", &NORMAL), b"a\rb\rc");
    }

    #[test]
    fn given_bracketed_paste_mode_when_pasting_then_text_is_wrapped() {
        let modes = InputModes { bracketed_paste: true, ..NORMAL };
        assert_eq!(encode_paste("ls", &modes), b"\x1b[200~ls\x1b[201~");
    }

    #[test]
    fn given_clipboard_containing_the_end_marker_when_bracketed_pasting_then_the_marker_is_removed() {
        let modes = InputModes { bracketed_paste: true, ..NORMAL };
        assert_eq!(encode_paste("a\x1b[201~rm -rf /\r", &modes), b"\x1b[200~arm -rf /\r\x1b[201~");
    }

    #[test]
    fn given_focus_reporting_when_focus_changes_then_in_and_out_sequences_are_sent() {
        let modes = InputModes { focus_reporting: true, ..NORMAL };
        assert_eq!(encode_focus(true, &modes), Some(b"\x1b[I".to_vec()));
        assert_eq!(encode_focus(false, &modes), Some(b"\x1b[O".to_vec()));
        assert_eq!(encode_focus(true, &NORMAL), None);
    }

    #[test]
    fn given_mouse_reporting_off_when_clicking_then_nothing_is_sent() {
        let action = MouseAction::Press(MouseButton::Left);
        assert_eq!(encode_mouse(action, at(0, 0), Modifiers::NONE, &NORMAL), None);
    }

    #[test]
    fn given_sgr_mouse_when_pressing_and_releasing_then_one_based_coordinates_and_final_byte_distinguish_them() {
        let modes = mouse_modes(MouseReporting::Click, true);
        assert_eq!(
            encode_mouse(MouseAction::Press(MouseButton::Left), at(0, 0), Modifiers::NONE, &modes).unwrap(),
            b"\x1b[<0;1;1M"
        );
        assert_eq!(
            encode_mouse(MouseAction::Release(MouseButton::Left), at(9, 4), Modifiers::NONE, &modes).unwrap(),
            b"\x1b[<0;10;5m"
        );
        assert_eq!(
            encode_mouse(MouseAction::Press(MouseButton::Right), at(2, 2), Modifiers::NONE, &modes).unwrap(),
            b"\x1b[<2;3;3M"
        );
    }

    #[test]
    fn given_normal_mouse_encoding_when_pressing_and_releasing_then_release_uses_button_three() {
        let modes = mouse_modes(MouseReporting::Click, false);
        assert_eq!(
            encode_mouse(MouseAction::Press(MouseButton::Left), at(0, 0), Modifiers::NONE, &modes).unwrap(),
            [0x1b, b'[', b'M', 32, 33, 33]
        );
        assert_eq!(
            encode_mouse(MouseAction::Release(MouseButton::Left), at(0, 0), Modifiers::NONE, &modes).unwrap(),
            [0x1b, b'[', b'M', 35, 33, 33]
        );
    }

    #[test]
    fn given_normal_mouse_encoding_when_position_is_beyond_the_limit_then_nothing_is_sent() {
        let modes = mouse_modes(MouseReporting::Click, false);
        assert_eq!(encode_mouse(MouseAction::Press(MouseButton::Left), at(300, 0), Modifiers::NONE, &modes), None);
        assert!(encode_mouse(MouseAction::Press(MouseButton::Left), at(222, 0), Modifiers::NONE, &modes).is_some());
    }

    #[test]
    fn given_modifiers_when_clicking_then_modifier_bits_are_added_to_the_button_code() {
        let modes = mouse_modes(MouseReporting::Click, true);
        let all = Modifiers { shift: true, control: true, alt: true };
        assert_eq!(
            encode_mouse(MouseAction::Press(MouseButton::Left), at(0, 0), all, &modes).unwrap(),
            b"\x1b[<28;1;1M"
        );
    }

    #[test]
    fn given_click_only_reporting_when_moving_then_motion_is_not_reported() {
        let modes = mouse_modes(MouseReporting::Click, true);
        assert_eq!(encode_mouse(MouseAction::Move(Some(MouseButton::Left)), at(1, 1), Modifiers::NONE, &modes), None);
        assert_eq!(encode_mouse(MouseAction::Move(None), at(1, 1), Modifiers::NONE, &modes), None);
    }

    #[test]
    fn given_drag_reporting_when_dragging_then_motion_with_the_held_button_is_reported_but_plain_motion_is_not() {
        let modes = mouse_modes(MouseReporting::Drag, true);
        assert_eq!(
            encode_mouse(MouseAction::Move(Some(MouseButton::Left)), at(1, 1), Modifiers::NONE, &modes).unwrap(),
            b"\x1b[<32;2;2M"
        );
        assert_eq!(encode_mouse(MouseAction::Move(None), at(1, 1), Modifiers::NONE, &modes), None);
    }

    #[test]
    fn given_motion_reporting_when_moving_without_buttons_then_motion_is_reported() {
        let modes = mouse_modes(MouseReporting::Motion, true);
        assert_eq!(encode_mouse(MouseAction::Move(None), at(4, 6), Modifiers::NONE, &modes).unwrap(), b"\x1b[<35;5;7M");
    }

    #[test]
    fn given_mouse_reporting_when_scrolling_then_wheel_buttons_are_reported() {
        let modes = mouse_modes(MouseReporting::Click, true);
        assert_eq!(encode_mouse(MouseAction::WheelUp, at(0, 0), Modifiers::NONE, &modes).unwrap(), b"\x1b[<64;1;1M");
        assert_eq!(encode_mouse(MouseAction::WheelDown, at(0, 0), Modifiers::NONE, &modes).unwrap(), b"\x1b[<65;1;1M");
    }

    #[test]
    fn given_alternate_screen_with_alternate_scroll_when_scrolling_then_arrow_keys_repeat_per_line() {
        let modes = InputModes { alternate_screen: true, alternate_scroll: true, ..NORMAL };
        assert_eq!(encode_wheel_as_arrows(3, &modes).unwrap(), b"\x1b[A\x1b[A\x1b[A");
        assert_eq!(encode_wheel_as_arrows(-2, &modes).unwrap(), b"\x1b[B\x1b[B");
    }

    #[test]
    fn given_application_cursor_mode_when_scrolling_alternate_screen_then_arrows_use_ss3() {
        let modes =
            InputModes { alternate_screen: true, alternate_scroll: true, application_cursor_keys: true, ..NORMAL };
        assert_eq!(encode_wheel_as_arrows(1, &modes).unwrap(), b"\x1bOA");
    }

    #[test]
    fn given_primary_screen_or_mouse_reporting_or_no_scroll_when_scrolling_then_no_arrows_are_sent() {
        let alternate = InputModes { alternate_screen: true, alternate_scroll: true, ..NORMAL };
        assert_eq!(encode_wheel_as_arrows(1, &NORMAL), None);
        assert_eq!(encode_wheel_as_arrows(1, &InputModes { alternate_screen: false, ..alternate }), None);
        assert_eq!(
            encode_wheel_as_arrows(1, &InputModes { mouse_reporting: MouseReporting::Click, ..alternate }),
            None
        );
        assert_eq!(encode_wheel_as_arrows(0, &alternate), None);
    }
}
