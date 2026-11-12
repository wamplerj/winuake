//! All settings, loaded from and saved to TOML. Groups, keys, defaults and ranges follow Yakuake's `yakuake.kcfg`.
//!
//! Loading never fails: a syntax error or a bad group falls back to defaults for that part and is reported as a problem, so one typo in
//! the file cannot stop the app from starting. This module does no file IO; the app reads, writes and watches the file.

use crate::profile::Profile;
use crate::window::WindowSettings;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DURATION_MAX_MILLISECONDS: u32 = 86_400_000;
pub const POLL_INTERVAL_MINIMUM_MILLISECONDS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub skin: String,
    pub skin_installed_with_kns: bool,
    pub translucency: bool,
    /// `#RRGGBB`.
    pub background_color: String,
    pub background_color_opacity: u8,
    pub terminal_highlight_on_manual_activation: bool,
    pub hide_skin_borders: bool,
    pub terminal_highlight_opacity: f64,
    pub terminal_highlight_duration_ms: u32,
    pub keyboard_input_block_indicator_color: String,
    pub keyboard_input_block_indicator_opacity: f64,
    pub keyboard_input_block_indicator_duration_ms: u32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            skin: "default".to_owned(),
            skin_installed_with_kns: false,
            translucency: false,
            background_color: "#000000".to_owned(),
            background_color_opacity: 50,
            terminal_highlight_on_manual_activation: false,
            hide_skin_borders: false,
            terminal_highlight_opacity: 0.4,
            terminal_highlight_duration_ms: 250,
            keyboard_input_block_indicator_color: "#FF0000".to_owned(),
            keyboard_input_block_indicator_opacity: 0.5,
            keyboard_input_block_indicator_duration_ms: 250,
        }
    }
}

impl AppearanceSettings {
    /// Forces values into range. Colors that are not `#RRGGBB` revert to their defaults; each reverted key is returned.
    fn clamped(mut self) -> (Self, Vec<&'static str>) {
        let defaults = Self::default();
        let mut reverted = Vec::new();
        if !is_hex_color(&self.background_color) {
            self.background_color = defaults.background_color;
            reverted.push("background_color");
        }
        if !is_hex_color(&self.keyboard_input_block_indicator_color) {
            self.keyboard_input_block_indicator_color = defaults.keyboard_input_block_indicator_color;
            reverted.push("keyboard_input_block_indicator_color");
        }
        self.background_color_opacity = self.background_color_opacity.min(100);
        self.terminal_highlight_opacity = clamp_unit(self.terminal_highlight_opacity);
        self.keyboard_input_block_indicator_opacity = clamp_unit(self.keyboard_input_block_indicator_opacity);
        self.terminal_highlight_duration_ms = self.terminal_highlight_duration_ms.min(DURATION_MAX_MILLISECONDS);
        self.keyboard_input_block_indicator_duration_ms =
            self.keyboard_input_block_indicator_duration_ms.min(DURATION_MAX_MILLISECONDS);
        (self, reverted)
    }
}

fn clamp_unit(value: f64) -> f64 {
    if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) }
}

/// `#RRGGBB` with hexadecimal digits.
pub fn is_hex_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|character| character.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoOpenSettings {
    pub poll_mouse: bool,
    pub poll_interval_ms: u32,
}

impl Default for AutoOpenSettings {
    fn default() -> Self {
        Self { poll_mouse: false, poll_interval_ms: 500 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogSettings {
    pub first_run: bool,
    pub confirm_quit: bool,
}

impl Default for DialogSettings {
    fn default() -> Self {
        Self { first_run: true, confirm_quit: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub window: WindowSettings,
    pub appearance: AppearanceSettings,
    pub auto_open: AutoOpenSettings,
    pub dialogs: DialogSettings,
    /// Global toggle hotkey as chord text (`F12`, `Ctrl+Alt+Q`); unset means try the built-in candidates.
    pub toggle_hotkey: Option<String>,
    /// Shortcut overrides: action name to chord text, or `none` to unbind.
    pub shortcuts: BTreeMap<String, String>,
    pub profiles: Vec<Profile>,
    /// Name of the profile new terminals use; unset means the first profile.
    pub default_profile: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window: WindowSettings::default(),
            appearance: AppearanceSettings::default(),
            auto_open: AutoOpenSettings::default(),
            dialogs: DialogSettings::default(),
            toggle_hotkey: None,
            shortcuts: BTreeMap::new(),
            profiles: vec![Profile::default()],
            default_profile: None,
        }
    }
}

/// Something in the file that was ignored or corrected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsProblem {
    /// Table the problem is in, such as `window`; empty for the whole file.
    pub group: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSettings {
    pub settings: Settings,
    pub problems: Vec<SettingsProblem>,
    /// The text was not valid TOML, so `settings` is just the defaults. A running app should keep what it has instead.
    pub unreadable: bool,
}

// ---- on-disk groups ----

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct WindowGroup {
    screen: u32,
    width: u8,
    height: u8,
    position: u8,
    show_on_all_desktops: bool,
    keep_open: bool,
    keep_open_after_last_session_closes: bool,
    toggle_to_focus: bool,
    keep_above: bool,
    show_title_bar: bool,
    show_tab_bar: bool,
    dynamic_tab_titles: bool,
    show_tray_icon: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct BehaviorGroup {
    start_with_windows: bool,
    focus_follows_mouse: bool,
    open_after_start: bool,
    remember_fullscreen: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct AnimationGroup {
    frames: u32,
}

impl Default for WindowGroup {
    fn default() -> Self {
        Self::from(&WindowSettings::default())
    }
}

impl Default for BehaviorGroup {
    fn default() -> Self {
        Self::from(&WindowSettings::default())
    }
}

impl Default for AnimationGroup {
    fn default() -> Self {
        Self::from(&WindowSettings::default())
    }
}

impl From<&WindowSettings> for WindowGroup {
    fn from(window: &WindowSettings) -> Self {
        Self {
            screen: window.screen,
            width: window.width_percent,
            height: window.height_percent,
            position: window.position_percent,
            show_on_all_desktops: window.show_on_all_desktops,
            keep_open: window.keep_open,
            keep_open_after_last_session_closes: window.keep_open_after_last_session_closes,
            toggle_to_focus: window.toggle_to_focus,
            keep_above: window.keep_above,
            show_title_bar: window.show_title_bar,
            show_tab_bar: window.show_tab_bar,
            dynamic_tab_titles: window.dynamic_tab_titles,
            show_tray_icon: window.show_tray_icon,
        }
    }
}

impl From<&WindowSettings> for BehaviorGroup {
    fn from(window: &WindowSettings) -> Self {
        Self {
            start_with_windows: window.start_with_windows,
            focus_follows_mouse: window.focus_follows_mouse,
            open_after_start: window.open_after_start,
            remember_fullscreen: window.remember_fullscreen,
        }
    }
}

impl From<&WindowSettings> for AnimationGroup {
    fn from(window: &WindowSettings) -> Self {
        Self { frames: window.frames }
    }
}

#[derive(Serialize)]
struct SettingsFile<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    toggle_hotkey: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_profile: &'a Option<String>,
    window: WindowGroup,
    behavior: BehaviorGroup,
    appearance: &'a AppearanceSettings,
    animation: AnimationGroup,
    auto_open: &'a AutoOpenSettings,
    dialogs: &'a DialogSettings,
    shortcuts: &'a BTreeMap<String, String>,
    #[serde(rename = "profile")]
    profiles: &'a [Profile],
}

impl Settings {
    /// Parses a settings file. Never fails; see the module docs.
    pub fn parse(text: &str) -> LoadedSettings {
        let mut problems = Vec::new();
        let table: toml::Table = match text.parse() {
            Ok(table) => table,
            Err(error) => {
                let message = format!("not valid TOML, using defaults: {}", error.message());
                problems.push(SettingsProblem { group: String::new(), message });
                return LoadedSettings { settings: Settings::default(), problems, unreadable: true };
            }
        };

        let window_group: WindowGroup = read_group(&table, "window", &mut problems);
        let behavior: BehaviorGroup = read_group(&table, "behavior", &mut problems);
        let animation: AnimationGroup = read_group(&table, "animation", &mut problems);
        let window = WindowSettings {
            screen: window_group.screen,
            width_percent: window_group.width,
            height_percent: window_group.height,
            position_percent: window_group.position,
            show_on_all_desktops: window_group.show_on_all_desktops,
            keep_open: window_group.keep_open,
            keep_open_after_last_session_closes: window_group.keep_open_after_last_session_closes,
            toggle_to_focus: window_group.toggle_to_focus,
            keep_above: window_group.keep_above,
            frames: animation.frames,
            open_after_start: behavior.open_after_start,
            remember_fullscreen: behavior.remember_fullscreen,
            show_tray_icon: window_group.show_tray_icon,
            show_title_bar: window_group.show_title_bar,
            show_tab_bar: window_group.show_tab_bar,
            dynamic_tab_titles: window_group.dynamic_tab_titles,
            focus_follows_mouse: behavior.focus_follows_mouse,
            start_with_windows: behavior.start_with_windows,
        };
        let clamped_window = window.clone().clamped();
        if clamped_window != window {
            problems.push(SettingsProblem {
                group: "window".to_owned(),
                message: "a value was outside its allowed range and was adjusted".to_owned(),
            });
        }

        let (appearance, reverted) = read_group::<AppearanceSettings>(&table, "appearance", &mut problems).clamped();
        for key in reverted {
            problems.push(SettingsProblem {
                group: "appearance".to_owned(),
                message: format!("`{key}` is not a #RRGGBB color; the default was used"),
            });
        }

        let mut auto_open: AutoOpenSettings = read_group(&table, "auto_open", &mut problems);
        auto_open.poll_interval_ms = auto_open.poll_interval_ms.max(POLL_INTERVAL_MINIMUM_MILLISECONDS);
        let dialogs = read_group(&table, "dialogs", &mut problems);

        let toggle_hotkey = read_text(&table, "toggle_hotkey", &mut problems);
        let default_profile = read_text(&table, "default_profile", &mut problems);
        let shortcuts = read_shortcuts(&table, &mut problems);
        let profiles = read_profiles(&table, &mut problems);

        let settings = Settings {
            window: clamped_window,
            appearance,
            auto_open,
            dialogs,
            toggle_hotkey,
            shortcuts,
            profiles,
            default_profile,
        };
        LoadedSettings { settings, problems, unreadable: false }
    }

    /// Serializes to TOML in the same shape `parse` reads.
    pub fn to_toml(&self) -> String {
        let file = SettingsFile {
            toggle_hotkey: &self.toggle_hotkey,
            default_profile: &self.default_profile,
            window: WindowGroup::from(&self.window),
            behavior: BehaviorGroup::from(&self.window),
            appearance: &self.appearance,
            animation: AnimationGroup::from(&self.window),
            auto_open: &self.auto_open,
            dialogs: &self.dialogs,
            shortcuts: &self.shortcuts,
            profiles: &self.profiles,
        };
        toml::to_string_pretty(&file).expect("settings always serialize")
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    /// Name of the profile new terminals use.
    pub fn default_profile_name(&self) -> String {
        self.active_profile().name
    }

    /// `base`, or `base 2`, `base 3` ... when the name is taken.
    pub fn unique_profile_name(&self, base: &str) -> String {
        let base = base.trim();
        let base = if base.is_empty() { "Profile" } else { base };
        if self.profile(base).is_none() {
            return base.to_owned();
        }
        (2..)
            .map(|number| format!("{base} {number}"))
            .find(|candidate| self.profile(candidate).is_none())
            .expect("a free name exists")
    }

    /// Adds a profile with default settings and returns its (made unique) name.
    pub fn add_profile(&mut self, name: &str) -> String {
        let name = self.unique_profile_name(name);
        self.profiles.push(Profile { name: name.clone(), ..Profile::default() });
        name
    }

    /// Copies a profile under `<name> copy`. `None` when it does not exist.
    pub fn duplicate_profile(&mut self, name: &str) -> Option<String> {
        let source = self.profile(name)?.clone();
        let copy_name = self.unique_profile_name(&format!("{} copy", source.name));
        self.profiles.push(Profile { name: copy_name.clone(), ..source });
        Some(copy_name)
    }

    /// Deletes a profile. The last profile cannot be deleted; deleting the default makes the first remaining one the default.
    pub fn delete_profile(&mut self, name: &str) -> bool {
        if self.profiles.len() <= 1 {
            return false;
        }
        let Some(index) = self.profiles.iter().position(|profile| profile.name == name) else { return false };
        let was_default = self.default_profile_name() == name;
        self.profiles.remove(index);
        if was_default || self.default_profile.as_deref() == Some(name) {
            self.default_profile = Some(self.profiles[0].name.clone());
        }
        true
    }

    pub fn set_default_profile(&mut self, name: &str) -> bool {
        if self.profile(name).is_none() {
            return false;
        }
        self.default_profile = Some(name.to_owned());
        true
    }

    /// Adds detected profiles that are not already there (same name, or same program and arguments). Returns how many were added.
    pub fn merge_detected_profiles(&mut self, detected: Vec<Profile>) -> usize {
        let mut added = 0;
        for profile in detected {
            let known = self.profiles.iter().any(|existing| {
                existing.name == profile.name
                    || (existing.program.as_deref().map(str::to_lowercase)
                        == profile.program.as_deref().map(str::to_lowercase)
                        && existing.arguments == profile.arguments)
            });
            if !known {
                self.profiles.push(profile);
                added += 1;
            }
        }
        added
    }

    /// The profile new terminals use: the named default, else the first, else built-in defaults.
    pub fn active_profile(&self) -> Profile {
        self.default_profile
            .as_ref()
            .and_then(|name| self.profiles.iter().find(|profile| &profile.name == name))
            .or_else(|| self.profiles.first())
            .cloned()
            .unwrap_or_default()
    }
}

fn read_group<T: for<'de> Deserialize<'de> + Default>(
    table: &toml::Table,
    name: &str,
    problems: &mut Vec<SettingsProblem>,
) -> T {
    let Some(value) = table.get(name) else { return T::default() };
    match value.clone().try_into::<T>() {
        Ok(group) => group,
        Err(error) => {
            problems.push(SettingsProblem {
                group: name.to_owned(),
                message: format!("ignored, using defaults: {}", error.message()),
            });
            T::default()
        }
    }
}

fn read_text(table: &toml::Table, key: &str, problems: &mut Vec<SettingsProblem>) -> Option<String> {
    match table.get(key)? {
        toml::Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        toml::Value::String(_) => None,
        _ => {
            problems.push(SettingsProblem { group: String::new(), message: format!("`{key}` must be text") });
            None
        }
    }
}

fn read_shortcuts(table: &toml::Table, problems: &mut Vec<SettingsProblem>) -> BTreeMap<String, String> {
    let mut shortcuts = BTreeMap::new();
    let Some(value) = table.get("shortcuts") else { return shortcuts };
    let Some(entries) = value.as_table() else {
        problems.push(SettingsProblem { group: "shortcuts".to_owned(), message: "must be a table".to_owned() });
        return shortcuts;
    };
    for (action, chord) in entries {
        match chord.as_str() {
            Some(chord) => {
                shortcuts.insert(action.clone(), chord.to_owned());
            }
            None => problems
                .push(SettingsProblem { group: "shortcuts".to_owned(), message: format!("`{action}` must be text") }),
        }
    }
    shortcuts
}

fn read_profiles(table: &toml::Table, problems: &mut Vec<SettingsProblem>) -> Vec<Profile> {
    let mut profiles: Vec<Profile> = Vec::new();
    if let Some(entries) = table.get("profile").and_then(toml::Value::as_array) {
        for (index, entry) in entries.iter().enumerate() {
            match entry.clone().try_into::<Profile>() {
                Ok(profile) if profile.name.trim().is_empty() => problems.push(SettingsProblem {
                    group: "profile".to_owned(),
                    message: format!("profile {} has no name and was skipped", index + 1),
                }),
                Ok(profile) if profiles.iter().any(|existing| existing.name == profile.name) => {
                    problems.push(SettingsProblem {
                        group: "profile".to_owned(),
                        message: format!("profile name `{}` is used twice; the later one was skipped", profile.name),
                    });
                }
                Ok(profile) => profiles.push(profile.clamped()),
                Err(error) => problems.push(SettingsProblem {
                    group: "profile".to_owned(),
                    message: format!("profile {} skipped: {}", index + 1, error.message()),
                }),
            }
        }
    }
    if profiles.is_empty() {
        profiles.push(Profile::default());
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::CursorShapeSetting;

    #[test]
    fn given_an_empty_file_when_parsing_then_every_setting_has_its_kcfg_default() {
        let loaded = Settings::parse("");
        assert!(loaded.problems.is_empty());
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.settings.dialogs.first_run && loaded.settings.dialogs.confirm_quit);
        assert_eq!(loaded.settings.auto_open.poll_interval_ms, 500);
        assert_eq!(loaded.settings.appearance.skin, "default");
    }

    #[test]
    fn given_settings_when_saved_and_parsed_then_nothing_is_lost() {
        let mut settings = Settings::default();
        settings.window.width_percent = 70;
        settings.window.keep_open = false;
        settings.window.dynamic_tab_titles = true;
        settings.window.focus_follows_mouse = true;
        settings.window.start_with_windows = true;
        settings.window.frames = 0;
        settings.appearance.translucency = true;
        settings.appearance.background_color = "#112233".to_owned();
        settings.auto_open.poll_mouse = true;
        settings.dialogs.first_run = false;
        settings.toggle_hotkey = Some("Ctrl+Alt+Q".to_owned());
        settings.shortcuts.insert("new-session".to_owned(), "Ctrl+Alt+N".to_owned());
        settings.profiles.push(Profile {
            name: "wsl".to_owned(),
            program: Some("wsl.exe".to_owned()),
            ..Profile::default()
        });
        settings.default_profile = Some("wsl".to_owned());
        let loaded = Settings::parse(&settings.to_toml());
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        assert_eq!(loaded.settings, settings);
    }

    #[test]
    fn given_a_partial_file_when_parsing_then_missing_keys_keep_defaults() {
        let loaded = Settings::parse("[window]\nwidth = 60\n[behavior]\nopen_after_start = true\n");
        assert!(loaded.problems.is_empty());
        assert_eq!(loaded.settings.window.width_percent, 60);
        assert_eq!(loaded.settings.window.height_percent, 50);
        assert!(loaded.settings.window.open_after_start);
    }

    #[test]
    fn given_out_of_range_values_when_parsing_then_they_are_clamped_and_reported() {
        let loaded = Settings::parse(
            "[window]\nwidth = 5\n[animation]\nframes = 999\n[appearance]\nbackground_color_opacity = 250\nterminal_highlight_opacity = 7.5\n[auto_open]\npoll_interval_ms = 1\n",
        );
        assert_eq!(loaded.settings.window.width_percent, 10);
        assert_eq!(loaded.settings.window.frames, 50);
        assert_eq!(loaded.settings.appearance.background_color_opacity, 100);
        assert_eq!(loaded.settings.appearance.terminal_highlight_opacity, 1.0);
        assert_eq!(loaded.settings.auto_open.poll_interval_ms, 10);
        assert!(loaded.problems.iter().any(|problem| problem.group == "window"));
    }

    #[test]
    fn given_invalid_toml_when_parsing_then_defaults_are_used_and_the_file_problem_is_reported() {
        let loaded = Settings::parse("[window\nwidth = ");
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.problems.len(), 1);
        assert_eq!(loaded.problems[0].group, "");
        assert!(loaded.unreadable);
        assert!(!Settings::parse("").unreadable);
    }

    #[test]
    fn given_a_bad_group_when_parsing_then_only_that_group_falls_back() {
        let loaded = Settings::parse("[window]\nwidth = \"wide\"\n[behavior]\nopen_after_start = true\n");
        assert_eq!(loaded.settings.window, WindowSettings { open_after_start: true, ..WindowSettings::default() });
        assert_eq!(loaded.problems.len(), 1);
        assert_eq!(loaded.problems[0].group, "window");
    }

    #[test]
    fn given_a_bad_color_when_parsing_then_the_default_color_is_used() {
        let loaded = Settings::parse(
            "[appearance]\nbackground_color = \"red\"\nkeyboard_input_block_indicator_color = \"#12345\"\n",
        );
        assert_eq!(loaded.settings.appearance.background_color, "#000000");
        assert_eq!(loaded.settings.appearance.keyboard_input_block_indicator_color, "#FF0000");
        assert_eq!(loaded.problems.len(), 2);
    }

    #[test]
    fn given_hex_colors_when_checking_then_only_rrggbb_passes() {
        assert!(is_hex_color("#aB09fF"));
        assert!(!is_hex_color("aB09fF0"));
        assert!(!is_hex_color("#abc"));
        assert!(!is_hex_color("#gggggg"));
    }

    #[test]
    fn given_unknown_keys_and_groups_when_parsing_then_they_are_ignored() {
        let loaded = Settings::parse("future = 1\n[window]\nmystery = true\nwidth = 80\n[elsewhere]\nx = 1\n");
        assert!(loaded.problems.is_empty());
        assert_eq!(loaded.settings.window.width_percent, 80);
    }

    #[test]
    fn given_shortcut_overrides_when_parsing_then_they_are_kept_by_action_name() {
        let loaded = Settings::parse("[shortcuts]\nnew-session = \"Ctrl+Alt+N\"\nclose-session = \"none\"\nbad = 5\n");
        assert_eq!(loaded.settings.shortcuts.get("new-session").map(String::as_str), Some("Ctrl+Alt+N"));
        assert_eq!(loaded.settings.shortcuts.get("close-session").map(String::as_str), Some("none"));
        assert_eq!(loaded.settings.shortcuts.len(), 2);
        assert_eq!(loaded.problems.len(), 1);
        let wrong_shape = Settings::parse("shortcuts = 3\n");
        assert_eq!(wrong_shape.problems.len(), 1);
    }

    #[test]
    fn given_toggle_hotkey_text_when_parsing_then_it_is_trimmed_and_empty_means_unset() {
        assert_eq!(Settings::parse("toggle_hotkey = \" F11 \"").settings.toggle_hotkey.as_deref(), Some("F11"));
        assert_eq!(Settings::parse("toggle_hotkey = \"\"").settings.toggle_hotkey, None);
        let wrong = Settings::parse("toggle_hotkey = 12");
        assert_eq!(wrong.settings.toggle_hotkey, None);
        assert_eq!(wrong.problems.len(), 1);
    }

    #[test]
    fn given_profiles_when_parsing_then_valid_ones_load_with_clamping_and_bad_ones_are_skipped() {
        let text = r#"
default_profile = "wsl"
[[profile]]
name = "wsl"
program = "wsl.exe"
arguments = ["-d", "Ubuntu"]
font_size_points = 500.0
cursor_shape = "Bar"
cursor_blink = false
[[profile]]
name = "wsl"
[[profile]]
name = ""
[[profile]]
name = "broken"
scrollback_lines = "lots"
"#;
        let loaded = Settings::parse(text);
        assert_eq!(loaded.settings.profiles.len(), 1);
        let profile = &loaded.settings.profiles[0];
        assert_eq!((profile.program.as_deref(), profile.arguments.len()), (Some("wsl.exe"), 2));
        assert_eq!(profile.font_size_points, 72.0);
        assert_eq!(profile.cursor_shape, CursorShapeSetting::Bar);
        assert!(!profile.cursor_blink);
        assert_eq!(loaded.problems.len(), 3);
        assert_eq!(loaded.settings.active_profile().name, "wsl");
    }

    #[test]
    fn given_no_profiles_when_parsing_then_the_built_in_default_profile_exists() {
        assert_eq!(Settings::parse("").settings.profiles, vec![Profile::default()]);
    }

    #[test]
    fn given_a_missing_default_profile_name_when_asking_for_the_active_profile_then_the_first_is_used() {
        let mut settings = Settings::default();
        settings.profiles.push(Profile { name: "second".to_owned(), ..Profile::default() });
        settings.default_profile = Some("nope".to_owned());
        assert_eq!(settings.active_profile().name, "Default");
        settings.default_profile = Some("second".to_owned());
        assert_eq!(settings.active_profile().name, "second");
        settings.profiles.clear();
        assert_eq!(settings.active_profile(), Profile::default());
    }

    #[test]
    fn given_default_settings_when_saved_then_the_file_has_the_kcfg_groups() {
        let text = Settings::default().to_toml();
        for group in [
            "[window]",
            "[behavior]",
            "[appearance]",
            "[animation]",
            "[auto_open]",
            "[dialogs]",
            "[shortcuts]",
            "[[profile]]",
        ] {
            assert!(text.contains(group), "missing {group} in:\n{text}");
        }
        assert!(!text.contains("toggle_hotkey"));
    }

    #[test]
    fn given_a_taken_name_when_adding_a_profile_then_a_numbered_name_is_used() {
        let mut settings = Settings::default();
        assert_eq!(settings.add_profile("Git Bash"), "Git Bash");
        assert_eq!(settings.add_profile("Git Bash"), "Git Bash 2");
        assert_eq!(settings.add_profile("  "), "Profile");
        assert_eq!(settings.profiles.len(), 4);
    }

    #[test]
    fn given_a_profile_when_duplicating_then_the_copy_keeps_its_settings_under_a_new_name() {
        let mut settings = Settings::default();
        settings.profiles[0].font_size_points = 15.0;
        assert_eq!(settings.duplicate_profile("Default").as_deref(), Some("Default copy"));
        assert_eq!(settings.duplicate_profile("Default").as_deref(), Some("Default copy 2"));
        assert_eq!(settings.profile("Default copy").map(|profile| profile.font_size_points), Some(15.0));
        assert_eq!(settings.duplicate_profile("nope"), None);
    }

    #[test]
    fn given_profiles_when_deleting_then_the_last_one_stays_and_a_deleted_default_is_replaced() {
        let mut settings = Settings::default();
        assert!(!settings.delete_profile("Default"), "the last profile stays");
        settings.add_profile("wsl");
        settings.set_default_profile("wsl");
        assert!(settings.delete_profile("wsl"));
        assert_eq!(settings.default_profile_name(), "Default");
        assert!(!settings.delete_profile("wsl"));
    }

    #[test]
    fn given_profiles_when_setting_the_default_then_unknown_names_are_refused() {
        let mut settings = Settings::default();
        settings.add_profile("pwsh");
        assert!(settings.set_default_profile("pwsh"));
        assert_eq!(settings.active_profile().name, "pwsh");
        assert!(!settings.set_default_profile("missing"));
        assert_eq!(settings.default_profile_name(), "pwsh");
    }

    #[test]
    fn given_detected_profiles_when_merging_then_only_new_ones_are_added() {
        let mut settings = Settings::default();
        let detected = vec![
            Profile {
                name: "Git Bash".into(),
                program: Some("C:/Git/bin/bash.exe".into()),
                arguments: vec!["--login".into()],
                ..Profile::default()
            },
            Profile {
                name: "Other name".into(),
                program: Some("c:/git/bin/BASH.exe".into()),
                arguments: vec!["--login".into()],
                ..Profile::default()
            },
            Profile { name: "Default".into(), program: Some("x.exe".into()), ..Profile::default() },
            Profile { name: "cmd".into(), program: Some("cmd.exe".into()), ..Profile::default() },
        ];
        assert_eq!(
            settings.merge_detected_profiles(detected.clone()),
            2,
            "Git Bash and cmd; the duplicate program and the existing name are skipped"
        );
        assert_eq!(settings.merge_detected_profiles(detected), 0);
    }

    #[test]
    fn given_profile_names_when_looking_up_then_the_profile_is_found() {
        let settings = Settings::default();
        assert!(settings.profile("Default").is_some());
        assert!(settings.profile("x").is_none());
    }
}
