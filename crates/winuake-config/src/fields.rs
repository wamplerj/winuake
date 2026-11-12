//! A table describing every setting the settings window edits: page, label, kind, and how to read and write it. One table drives the
//! UI (so pages cannot drift from the settings) and is tested without any UI. Labels follow Yakuake's `windowsettings.ui`,
//! `behaviorsettings.ui` and `appearancesettings.ui`.

use crate::profile::CursorShapeSetting;
use crate::settings::{Settings, is_hex_color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Window,
    Behavior,
    Appearance,
    Profile,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Window => "Window",
            Page::Behavior => "Behavior",
            Page::Appearance => "Appearance",
            Page::Profile => "Profile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Toggle,
    /// Whole number within an inclusive range.
    Number {
        minimum: i64,
        maximum: i64,
    },
    Text,
    /// `#RRGGBB`.
    Color,
    /// One of the listed names.
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub struct FieldDescriptor {
    pub page: Page,
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Bool(bool),
    Number(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub key: String,
    pub message: String,
}

const CURSOR_SHAPES: &[&str] = &["Block", "Underline", "Bar"];

const fn field(page: Page, key: &'static str, label: &'static str, kind: FieldKind) -> FieldDescriptor {
    FieldDescriptor { page, key, label, kind }
}

const PERCENT: FieldKind = FieldKind::Number { minimum: 10, maximum: 100 };
const OPACITY_PERCENT: FieldKind = FieldKind::Number { minimum: 0, maximum: 100 };
const MILLISECONDS: FieldKind = FieldKind::Number { minimum: 0, maximum: 86_400_000 };

pub fn descriptors() -> Vec<FieldDescriptor> {
    use FieldKind::{Color, Text, Toggle};
    use Page::{Appearance, Behavior, Profile, Window};
    vec![
        field(
            Window,
            "window.screen",
            "Screen (0 follows the mouse pointer)",
            FieldKind::Number { minimum: 0, maximum: 64 },
        ),
        field(Window, "window.width", "Width (% of the screen)", PERCENT),
        field(Window, "window.height", "Height (% of the screen)", PERCENT),
        field(Window, "window.position", "Horizontal position (0 left, 50 centered, 100 right)", OPACITY_PERCENT),
        field(
            Window,
            "animation.frames",
            "Animation frames (0 turns the slide off)",
            FieldKind::Number { minimum: 0, maximum: 50 },
        ),
        field(Window, "window.keep_open", "Keep window open when it loses focus", Toggle),
        field(Window, "window.keep_above", "Keep window above others", Toggle),
        field(Window, "window.show_on_all_desktops", "Show on all desktops", Toggle),
        field(Window, "window.toggle_to_focus", "Toggle focuses an open window instead of closing it", Toggle),
        field(Window, "window.show_title_bar", "Show title bar", Toggle),
        field(Window, "window.show_tab_bar", "Show tab bar", Toggle),
        field(Window, "window.show_tray_icon", "Show tray icon", Toggle),
        field(Behavior, "behavior.start_with_windows", "Start Winuake when I sign in to Windows", Toggle),
        field(Behavior, "behavior.open_after_start", "Open window after start", Toggle),
        field(Behavior, "behavior.remember_fullscreen", "Remember full screen state", Toggle),
        field(Behavior, "behavior.focus_follows_mouse", "Focus terminal under the mouse pointer", Toggle),
        field(
            Behavior,
            "window.keep_open_after_last_session_closes",
            "Keep window open after the last session closes",
            Toggle,
        ),
        field(Behavior, "window.dynamic_tab_titles", "Tab titles follow the terminal title", Toggle),
        field(Behavior, "dialogs.confirm_quit", "Confirm quit with multiple sessions", Toggle),
        field(Behavior, "auto_open.poll_mouse", "Open window when the pointer touches the top screen edge", Toggle),
        field(
            Behavior,
            "auto_open.poll_interval_ms",
            "Pointer check interval (ms)",
            FieldKind::Number { minimum: 10, maximum: 60_000 },
        ),
        field(Behavior, "toggle_hotkey", "Global toggle hotkey (empty uses the default)", Text),
        field(Appearance, "appearance.skin", "Skin", Text),
        field(Appearance, "appearance.hide_skin_borders", "Hide skin borders", Toggle),
        field(Appearance, "appearance.translucency", "Translucent window", Toggle),
        field(Appearance, "appearance.background_color", "Background color", Color),
        field(Appearance, "appearance.background_color_opacity", "Background opacity (%)", OPACITY_PERCENT),
        field(
            Appearance,
            "appearance.terminal_highlight_on_manual_activation",
            "Highlight a terminal when it is activated by hand",
            Toggle,
        ),
        field(Appearance, "appearance.terminal_highlight_opacity", "Highlight opacity (%)", OPACITY_PERCENT),
        field(Appearance, "appearance.terminal_highlight_duration_ms", "Highlight duration (ms)", MILLISECONDS),
        field(Appearance, "appearance.keyboard_input_block_indicator_color", "Blocked-input indicator color", Color),
        field(
            Appearance,
            "appearance.keyboard_input_block_indicator_opacity",
            "Blocked-input indicator opacity (%)",
            OPACITY_PERCENT,
        ),
        field(
            Appearance,
            "appearance.keyboard_input_block_indicator_duration_ms",
            "Blocked-input indicator duration (ms)",
            MILLISECONDS,
        ),
        field(Profile, "profile.name", "Profile name", Text),
        field(Profile, "profile.program", "Program (empty uses PowerShell, then cmd)", Text),
        field(Profile, "profile.arguments", "Arguments (space separated)", Text),
        field(Profile, "profile.working_directory", "Starting directory (empty uses your user folder)", Text),
        field(Profile, "profile.font_family", "Font", Text),
        field(Profile, "profile.font_size_points", "Font size (points)", FieldKind::Number { minimum: 6, maximum: 72 }),
        field(Profile, "profile.color_scheme", "Color scheme", Text),
        field(
            Profile,
            "profile.scrollback_lines",
            "Scrollback lines",
            FieldKind::Number { minimum: 0, maximum: 1_000_000 },
        ),
        field(Profile, "profile.cursor_shape", "Cursor shape", FieldKind::Choice(CURSOR_SHAPES)),
        field(Profile, "profile.cursor_blink", "Blinking cursor", Toggle),
    ]
}

pub fn descriptor(key: &str) -> Option<FieldDescriptor> {
    descriptors().into_iter().find(|candidate| candidate.key == key)
}

fn text(value: &Option<String>) -> FieldValue {
    FieldValue::Text(value.clone().unwrap_or_default())
}

/// Current value of a field. The `profile.*` fields read the active profile.
pub fn read(settings: &Settings, key: &str) -> Option<FieldValue> {
    let window = &settings.window;
    let appearance = &settings.appearance;
    let profile = settings.active_profile();
    Some(match key {
        "window.screen" => FieldValue::Number(i64::from(window.screen)),
        "window.width" => FieldValue::Number(i64::from(window.width_percent)),
        "window.height" => FieldValue::Number(i64::from(window.height_percent)),
        "window.position" => FieldValue::Number(i64::from(window.position_percent)),
        "animation.frames" => FieldValue::Number(i64::from(window.frames)),
        "window.keep_open" => FieldValue::Bool(window.keep_open),
        "window.keep_above" => FieldValue::Bool(window.keep_above),
        "window.show_on_all_desktops" => FieldValue::Bool(window.show_on_all_desktops),
        "window.toggle_to_focus" => FieldValue::Bool(window.toggle_to_focus),
        "window.show_title_bar" => FieldValue::Bool(window.show_title_bar),
        "window.show_tab_bar" => FieldValue::Bool(window.show_tab_bar),
        "window.show_tray_icon" => FieldValue::Bool(window.show_tray_icon),
        "behavior.start_with_windows" => FieldValue::Bool(window.start_with_windows),
        "behavior.open_after_start" => FieldValue::Bool(window.open_after_start),
        "behavior.remember_fullscreen" => FieldValue::Bool(window.remember_fullscreen),
        "behavior.focus_follows_mouse" => FieldValue::Bool(window.focus_follows_mouse),
        "window.keep_open_after_last_session_closes" => FieldValue::Bool(window.keep_open_after_last_session_closes),
        "window.dynamic_tab_titles" => FieldValue::Bool(window.dynamic_tab_titles),
        "dialogs.confirm_quit" => FieldValue::Bool(settings.dialogs.confirm_quit),
        "auto_open.poll_mouse" => FieldValue::Bool(settings.auto_open.poll_mouse),
        "auto_open.poll_interval_ms" => FieldValue::Number(i64::from(settings.auto_open.poll_interval_ms)),
        "toggle_hotkey" => text(&settings.toggle_hotkey),
        "appearance.skin" => FieldValue::Text(appearance.skin.clone()),
        "appearance.hide_skin_borders" => FieldValue::Bool(appearance.hide_skin_borders),
        "appearance.translucency" => FieldValue::Bool(appearance.translucency),
        "appearance.background_color" => FieldValue::Text(appearance.background_color.clone()),
        "appearance.background_color_opacity" => FieldValue::Number(i64::from(appearance.background_color_opacity)),
        "appearance.terminal_highlight_on_manual_activation" => {
            FieldValue::Bool(appearance.terminal_highlight_on_manual_activation)
        }
        "appearance.terminal_highlight_opacity" => {
            FieldValue::Number((appearance.terminal_highlight_opacity * 100.0).round() as i64)
        }
        "appearance.terminal_highlight_duration_ms" => {
            FieldValue::Number(i64::from(appearance.terminal_highlight_duration_ms))
        }
        "appearance.keyboard_input_block_indicator_color" => {
            FieldValue::Text(appearance.keyboard_input_block_indicator_color.clone())
        }
        "appearance.keyboard_input_block_indicator_opacity" => {
            FieldValue::Number((appearance.keyboard_input_block_indicator_opacity * 100.0).round() as i64)
        }
        "appearance.keyboard_input_block_indicator_duration_ms" => {
            FieldValue::Number(i64::from(appearance.keyboard_input_block_indicator_duration_ms))
        }
        "profile.name" => FieldValue::Text(profile.name),
        "profile.program" => text(&profile.program),
        "profile.arguments" => FieldValue::Text(profile.arguments.join(" ")),
        "profile.working_directory" => text(&profile.working_directory),
        "profile.font_family" => FieldValue::Text(profile.font_family),
        "profile.font_size_points" => FieldValue::Number(profile.font_size_points.round() as i64),
        "profile.color_scheme" => FieldValue::Text(profile.color_scheme),
        "profile.scrollback_lines" => FieldValue::Number(i64::from(profile.scrollback_lines)),
        "profile.cursor_shape" => FieldValue::Text(format!("{:?}", profile.cursor_shape)),
        "profile.cursor_blink" => FieldValue::Bool(profile.cursor_blink),
        _ => return None,
    })
}

fn bad(key: &str, message: &str) -> FieldError {
    FieldError { key: key.to_owned(), message: message.to_owned() }
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Sets a field, checking the value against the field's kind. Numbers outside the range are rejected, not clamped, so the UI can say so.
pub fn write(settings: &mut Settings, key: &str, value: FieldValue) -> Result<(), FieldError> {
    let descriptor = descriptor(key).ok_or_else(|| bad(key, "unknown setting"))?;
    let value = validate(&descriptor, value)?;
    let window = &mut settings.window;
    let appearance = &mut settings.appearance;
    match (key, value) {
        ("window.screen", FieldValue::Number(number)) => window.screen = number as u32,
        ("window.width", FieldValue::Number(number)) => window.width_percent = number as u8,
        ("window.height", FieldValue::Number(number)) => window.height_percent = number as u8,
        ("window.position", FieldValue::Number(number)) => window.position_percent = number as u8,
        ("animation.frames", FieldValue::Number(number)) => window.frames = number as u32,
        ("window.keep_open", FieldValue::Bool(flag)) => window.keep_open = flag,
        ("window.keep_above", FieldValue::Bool(flag)) => window.keep_above = flag,
        ("window.show_on_all_desktops", FieldValue::Bool(flag)) => window.show_on_all_desktops = flag,
        ("window.toggle_to_focus", FieldValue::Bool(flag)) => window.toggle_to_focus = flag,
        ("window.show_title_bar", FieldValue::Bool(flag)) => window.show_title_bar = flag,
        ("window.show_tab_bar", FieldValue::Bool(flag)) => window.show_tab_bar = flag,
        ("window.show_tray_icon", FieldValue::Bool(flag)) => window.show_tray_icon = flag,
        ("behavior.start_with_windows", FieldValue::Bool(flag)) => window.start_with_windows = flag,
        ("behavior.open_after_start", FieldValue::Bool(flag)) => window.open_after_start = flag,
        ("behavior.remember_fullscreen", FieldValue::Bool(flag)) => window.remember_fullscreen = flag,
        ("behavior.focus_follows_mouse", FieldValue::Bool(flag)) => window.focus_follows_mouse = flag,
        ("window.keep_open_after_last_session_closes", FieldValue::Bool(flag)) => {
            window.keep_open_after_last_session_closes = flag
        }
        ("window.dynamic_tab_titles", FieldValue::Bool(flag)) => window.dynamic_tab_titles = flag,
        ("dialogs.confirm_quit", FieldValue::Bool(flag)) => settings.dialogs.confirm_quit = flag,
        ("auto_open.poll_mouse", FieldValue::Bool(flag)) => settings.auto_open.poll_mouse = flag,
        ("auto_open.poll_interval_ms", FieldValue::Number(number)) => {
            settings.auto_open.poll_interval_ms = number as u32
        }
        ("toggle_hotkey", FieldValue::Text(text)) => settings.toggle_hotkey = optional_text(text),
        ("appearance.skin", FieldValue::Text(text)) => appearance.skin = text.trim().to_owned(),
        ("appearance.hide_skin_borders", FieldValue::Bool(flag)) => appearance.hide_skin_borders = flag,
        ("appearance.translucency", FieldValue::Bool(flag)) => appearance.translucency = flag,
        ("appearance.background_color", FieldValue::Text(text)) => appearance.background_color = text,
        ("appearance.background_color_opacity", FieldValue::Number(number)) => {
            appearance.background_color_opacity = number as u8
        }
        ("appearance.terminal_highlight_on_manual_activation", FieldValue::Bool(flag)) => {
            appearance.terminal_highlight_on_manual_activation = flag
        }
        ("appearance.terminal_highlight_opacity", FieldValue::Number(number)) => {
            appearance.terminal_highlight_opacity = number as f64 / 100.0
        }
        ("appearance.terminal_highlight_duration_ms", FieldValue::Number(number)) => {
            appearance.terminal_highlight_duration_ms = number as u32
        }
        ("appearance.keyboard_input_block_indicator_color", FieldValue::Text(text)) => {
            appearance.keyboard_input_block_indicator_color = text
        }
        ("appearance.keyboard_input_block_indicator_opacity", FieldValue::Number(number)) => {
            appearance.keyboard_input_block_indicator_opacity = number as f64 / 100.0
        }
        ("appearance.keyboard_input_block_indicator_duration_ms", FieldValue::Number(number)) => {
            appearance.keyboard_input_block_indicator_duration_ms = number as u32
        }
        (profile_key, value) if profile_key.starts_with("profile.") => write_profile(settings, profile_key, value)?,
        _ => return Err(bad(key, "the value has the wrong type")),
    }
    Ok(())
}

fn validate(descriptor: &FieldDescriptor, value: FieldValue) -> Result<FieldValue, FieldError> {
    let key = descriptor.key;
    match (descriptor.kind, &value) {
        (FieldKind::Toggle, FieldValue::Bool(_)) | (FieldKind::Text, FieldValue::Text(_)) => Ok(value),
        (FieldKind::Number { minimum, maximum }, FieldValue::Number(number)) => {
            if (minimum..=maximum).contains(number) {
                Ok(value)
            } else {
                Err(bad(key, &format!("must be between {minimum} and {maximum}")))
            }
        }
        (FieldKind::Color, FieldValue::Text(text)) => {
            if is_hex_color(text.trim()) {
                Ok(FieldValue::Text(text.trim().to_owned()))
            } else {
                Err(bad(key, "must be a color like #RRGGBB"))
            }
        }
        (FieldKind::Choice(choices), FieldValue::Text(text)) => choices
            .iter()
            .find(|choice| choice.eq_ignore_ascii_case(text.trim()))
            .map(|choice| FieldValue::Text((*choice).to_owned()))
            .ok_or_else(|| bad(key, &format!("must be one of {}", choices.join(", ")))),
        _ => Err(bad(key, "the value has the wrong type")),
    }
}

/// Edits the active profile, creating it in the list if the list is empty.
fn write_profile(settings: &mut Settings, key: &str, value: FieldValue) -> Result<(), FieldError> {
    if settings.profiles.is_empty() {
        settings.profiles.push(Default::default());
    }
    let active_name = settings.active_profile().name;
    let index = settings.profiles.iter().position(|profile| profile.name == active_name).unwrap_or(0);
    let renamed_to = match (key, &value) {
        ("profile.name", FieldValue::Text(name)) => {
            let name = name.trim();
            if name.is_empty() {
                return Err(bad(key, "a profile needs a name"));
            }
            if settings.profiles.iter().enumerate().any(|(other, profile)| other != index && profile.name == name) {
                return Err(bad(key, "another profile already has this name"));
            }
            Some(name.to_owned())
        }
        _ => None,
    };
    let profile = &mut settings.profiles[index];
    match (key, value) {
        ("profile.name", FieldValue::Text(_)) => profile.name = renamed_to.clone().unwrap_or_default(),
        ("profile.program", FieldValue::Text(text)) => profile.program = optional_text(text),
        ("profile.arguments", FieldValue::Text(text)) => {
            profile.arguments = text.split_whitespace().map(str::to_owned).collect()
        }
        ("profile.working_directory", FieldValue::Text(text)) => profile.working_directory = optional_text(text),
        ("profile.font_family", FieldValue::Text(text)) => profile.font_family = text.trim().to_owned(),
        ("profile.font_size_points", FieldValue::Number(number)) => profile.font_size_points = number as f32,
        ("profile.color_scheme", FieldValue::Text(text)) => profile.color_scheme = text.trim().to_owned(),
        ("profile.scrollback_lines", FieldValue::Number(number)) => profile.scrollback_lines = number as u32,
        ("profile.cursor_shape", FieldValue::Text(text)) => {
            profile.cursor_shape = match text.as_str() {
                "Underline" => CursorShapeSetting::Underline,
                "Bar" => CursorShapeSetting::Bar,
                _ => CursorShapeSetting::Block,
            }
        }
        ("profile.cursor_blink", FieldValue::Bool(flag)) => profile.cursor_blink = flag,
        _ => return Err(bad(key, "the value has the wrong type")),
    }
    if let Some(new_name) = renamed_to
        && settings.default_profile.as_deref() == Some(active_name.as_str())
    {
        settings.default_profile = Some(new_name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_every_descriptor_when_reading_default_settings_then_a_value_of_the_right_kind_comes_back() {
        let settings = Settings::default();
        for descriptor in descriptors() {
            let value = read(&settings, descriptor.key).unwrap_or_else(|| panic!("no value for {}", descriptor.key));
            let matches = match descriptor.kind {
                FieldKind::Toggle => matches!(value, FieldValue::Bool(_)),
                FieldKind::Number { .. } => matches!(value, FieldValue::Number(_)),
                FieldKind::Text | FieldKind::Color | FieldKind::Choice(_) => matches!(value, FieldValue::Text(_)),
            };
            assert!(matches, "{} has the wrong value kind: {value:?}", descriptor.key);
        }
    }

    #[test]
    fn given_every_descriptor_when_writing_back_its_own_value_then_it_is_accepted_and_nothing_changes() {
        let original = Settings::default();
        for descriptor in descriptors() {
            let mut settings = original.clone();
            let value = read(&settings, descriptor.key).expect("readable");
            write(&mut settings, descriptor.key, value)
                .unwrap_or_else(|error| panic!("{}: {}", descriptor.key, error.message));
            assert_eq!(settings, original, "writing {} back changed something", descriptor.key);
        }
    }

    #[test]
    fn given_descriptor_keys_then_they_are_unique() {
        let mut keys: Vec<&str> = descriptors().iter().map(|descriptor| descriptor.key).collect();
        keys.sort_unstable();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count);
    }

    #[test]
    fn given_a_number_in_range_when_writing_then_the_setting_changes() {
        let mut settings = Settings::default();
        write(&mut settings, "window.width", FieldValue::Number(70)).expect("valid");
        assert_eq!(settings.window.width_percent, 70);
        write(&mut settings, "appearance.terminal_highlight_opacity", FieldValue::Number(25)).expect("valid");
        assert_eq!(settings.appearance.terminal_highlight_opacity, 0.25);
        assert_eq!(read(&settings, "appearance.terminal_highlight_opacity"), Some(FieldValue::Number(25)));
    }

    #[test]
    fn given_a_number_out_of_range_when_writing_then_it_is_rejected_with_the_allowed_range() {
        let mut settings = Settings::default();
        let error = write(&mut settings, "window.width", FieldValue::Number(5)).unwrap_err();
        assert!(error.message.contains("10") && error.message.contains("100"), "{error:?}");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn given_wrong_value_types_or_unknown_keys_when_writing_then_they_are_rejected() {
        let mut settings = Settings::default();
        assert!(write(&mut settings, "window.width", FieldValue::Text("wide".into())).is_err());
        assert!(write(&mut settings, "window.keep_open", FieldValue::Number(1)).is_err());
        assert!(write(&mut settings, "nonsense", FieldValue::Bool(true)).is_err());
        assert!(read(&settings, "nonsense").is_none());
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn given_colors_when_writing_then_only_rrggbb_is_accepted() {
        let mut settings = Settings::default();
        write(&mut settings, "appearance.background_color", FieldValue::Text(" #aabbcc ".into())).expect("valid");
        assert_eq!(settings.appearance.background_color, "#aabbcc");
        assert!(write(&mut settings, "appearance.background_color", FieldValue::Text("blue".into())).is_err());
    }

    #[test]
    fn given_the_hotkey_field_when_set_to_blank_then_it_is_unset() {
        let mut settings = Settings::default();
        write(&mut settings, "toggle_hotkey", FieldValue::Text("Ctrl+Alt+Q".into())).expect("valid");
        assert_eq!(settings.toggle_hotkey.as_deref(), Some("Ctrl+Alt+Q"));
        write(&mut settings, "toggle_hotkey", FieldValue::Text("  ".into())).expect("valid");
        assert_eq!(settings.toggle_hotkey, None);
    }

    #[test]
    fn given_profile_fields_when_writing_then_the_active_profile_changes() {
        let mut settings = Settings::default();
        write(&mut settings, "profile.program", FieldValue::Text("wsl.exe".into())).expect("valid");
        write(&mut settings, "profile.arguments", FieldValue::Text("-d  Ubuntu".into())).expect("valid");
        write(&mut settings, "profile.cursor_shape", FieldValue::Text("bar".into())).expect("valid");
        write(&mut settings, "profile.font_size_points", FieldValue::Number(14)).expect("valid");
        let profile = settings.active_profile();
        assert_eq!(profile.program.as_deref(), Some("wsl.exe"));
        assert_eq!(profile.arguments, vec!["-d".to_owned(), "Ubuntu".to_owned()]);
        assert_eq!(profile.cursor_shape, CursorShapeSetting::Bar);
        assert_eq!(profile.font_size_points, 14.0);
        assert!(write(&mut settings, "profile.cursor_shape", FieldValue::Text("hexagon".into())).is_err());
    }

    #[test]
    fn given_the_default_profile_when_renamed_then_the_default_profile_pointer_follows() {
        let mut settings = Settings { default_profile: Some("Default".to_owned()), ..Settings::default() };
        write(&mut settings, "profile.name", FieldValue::Text("Main".into())).expect("valid");
        assert_eq!(settings.default_profile.as_deref(), Some("Main"));
        assert_eq!(settings.active_profile().name, "Main");
        assert!(write(&mut settings, "profile.name", FieldValue::Text("  ".into())).is_err());
    }

    #[test]
    fn given_two_profiles_when_renaming_the_active_one_to_the_others_name_then_it_is_rejected() {
        let mut settings = Settings::default();
        settings.profiles.push(crate::profile::Profile { name: "wsl".to_owned(), ..Default::default() });
        assert!(write(&mut settings, "profile.name", FieldValue::Text("wsl".into())).is_err());
    }

    #[test]
    fn given_no_profiles_when_writing_a_profile_field_then_one_is_created() {
        let mut settings = Settings { profiles: Vec::new(), ..Settings::default() };
        write(&mut settings, "profile.font_family", FieldValue::Text("Consolas".into())).expect("valid");
        assert_eq!(settings.profiles.len(), 1);
        assert_eq!(settings.profiles[0].font_family, "Consolas");
    }

    #[test]
    fn given_pages_then_each_has_a_title_and_fields() {
        for page in [Page::Window, Page::Behavior, Page::Appearance, Page::Profile] {
            assert!(!page.title().is_empty());
            assert!(descriptors().iter().any(|descriptor| descriptor.page == page));
        }
        assert!(descriptor("window.width").is_some());
        assert!(descriptor("nope").is_none());
    }
}
