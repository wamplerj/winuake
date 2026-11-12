//! Window-behavior settings. Names, defaults and ranges mirror Yakuake's `yakuake.kcfg` (`Window`, `Behavior` and `Animation` groups).

pub const WIDTH_PERCENT_RANGE: (u8, u8) = (10, 100);
pub const HEIGHT_PERCENT_RANGE: (u8, u8) = (10, 100);
pub const POSITION_PERCENT_RANGE: (u8, u8) = (0, 100);
pub const FRAMES_MAX: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSettings {
    /// `Window/Screen`: 0 follows the pointer, otherwise the 1-based screen number.
    pub screen: u32,
    /// `Window/Width`: percent of the work area.
    pub width_percent: u8,
    /// `Window/Height`: percent of the work area.
    pub height_percent: u8,
    /// `Window/Position`: 0 = left edge, 50 = centered, 100 = right edge.
    pub position_percent: u8,
    /// `Window/ShowOnAllDesktops`.
    pub show_on_all_desktops: bool,
    /// `Window/KeepOpen`: stay open when focus is lost.
    pub keep_open: bool,
    /// `Window/KeepOpenAfterLastSessionCloses`.
    pub keep_open_after_last_session_closes: bool,
    /// `Window/ToggleToFocus`.
    pub toggle_to_focus: bool,
    /// `Window/KeepAbove`.
    pub keep_above: bool,
    /// `Animation/Frames`: slide animation length in 10 ms frames; 0 disables the animation.
    pub frames: u32,
    /// `Behavior/OpenAfterStart`.
    pub open_after_start: bool,
    /// `Behavior/RememberFullscreen`.
    pub remember_fullscreen: bool,
    /// `Window/ShowSystrayIcon`.
    pub show_tray_icon: bool,
    /// `Window/ShowTitleBar`.
    pub show_title_bar: bool,
    /// `Window/ShowTabBar`.
    pub show_tab_bar: bool,
    /// `Window/DynamicTabTitles`: tab titles follow the terminal title.
    pub dynamic_tab_titles: bool,
    /// `Behavior/FocusFollowsMouse`.
    pub focus_follows_mouse: bool,
    /// Winuake addition: start when the user signs in to Windows.
    pub start_with_windows: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            screen: 0,
            width_percent: 90,
            height_percent: 50,
            position_percent: 50,
            show_on_all_desktops: true,
            keep_open: true,
            keep_open_after_last_session_closes: false,
            toggle_to_focus: false,
            keep_above: true,
            frames: 17,
            open_after_start: false,
            remember_fullscreen: false,
            show_tray_icon: true,
            show_title_bar: true,
            show_tab_bar: true,
            dynamic_tab_titles: false,
            focus_follows_mouse: false,
            start_with_windows: false,
        }
    }
}

impl WindowSettings {
    /// The same settings with every value forced into its allowed range.
    pub fn clamped(mut self) -> Self {
        self.width_percent = self.width_percent.clamp(WIDTH_PERCENT_RANGE.0, WIDTH_PERCENT_RANGE.1);
        self.height_percent = self.height_percent.clamp(HEIGHT_PERCENT_RANGE.0, HEIGHT_PERCENT_RANGE.1);
        self.position_percent = self.position_percent.clamp(POSITION_PERCENT_RANGE.0, POSITION_PERCENT_RANGE.1);
        self.frames = self.frames.min(FRAMES_MAX);
        self
    }

    /// Applies `key=value` overrides separated by commas, `+` or `;` (for example `frames=50+keep_open=false`). Meant for tests and
    /// development until the settings file exists. Returns the new settings and a message for every entry that was ignored.
    pub fn with_overrides(mut self, text: &str) -> (Self, Vec<String>) {
        let mut problems = Vec::new();
        for entry in text.split([',', '+', ';']).map(str::trim).filter(|entry| !entry.is_empty()) {
            let Some((key, value)) = entry.split_once('=') else {
                problems.push(format!("`{entry}` is not key=value"));
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let applied = match key {
                "screen" => value.parse().map(|parsed| self.screen = parsed).is_ok(),
                "width" => value.parse().map(|parsed| self.width_percent = parsed).is_ok(),
                "height" => value.parse().map(|parsed| self.height_percent = parsed).is_ok(),
                "position" => value.parse().map(|parsed| self.position_percent = parsed).is_ok(),
                "frames" => value.parse().map(|parsed| self.frames = parsed).is_ok(),
                "keep_open" => value.parse().map(|parsed| self.keep_open = parsed).is_ok(),
                "keep_above" => value.parse().map(|parsed| self.keep_above = parsed).is_ok(),
                "toggle_to_focus" => value.parse().map(|parsed| self.toggle_to_focus = parsed).is_ok(),
                "show_on_all_desktops" => value.parse().map(|parsed| self.show_on_all_desktops = parsed).is_ok(),
                "show_title_bar" => value.parse().map(|parsed| self.show_title_bar = parsed).is_ok(),
                "show_tab_bar" => value.parse().map(|parsed| self.show_tab_bar = parsed).is_ok(),
                "dynamic_tab_titles" => value.parse().map(|parsed| self.dynamic_tab_titles = parsed).is_ok(),
                "keep_open_after_last_session_closes" => {
                    value.parse().map(|parsed| self.keep_open_after_last_session_closes = parsed).is_ok()
                }
                "start_with_windows" => value.parse().map(|parsed| self.start_with_windows = parsed).is_ok(),
                "open_after_start" => value.parse().map(|parsed| self.open_after_start = parsed).is_ok(),
                _ => {
                    problems.push(format!("unknown setting `{key}`"));
                    continue;
                }
            };
            if !applied {
                problems.push(format!("`{value}` is not a valid value for `{key}`"));
            }
        }
        (self.clamped(), problems)
    }

    /// Topmost unless the window is meant to stay open without being kept above (Yakuake `applyWindowProperties`).
    pub fn wants_topmost(&self) -> bool {
        !self.keep_open || self.keep_above
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_default_settings_then_they_match_yakuakes_kcfg_defaults() {
        let settings = WindowSettings::default();
        assert_eq!(
            (settings.screen, settings.width_percent, settings.height_percent, settings.position_percent),
            (0, 90, 50, 50)
        );
        assert!(settings.show_on_all_desktops && settings.keep_open && settings.keep_above && settings.show_tray_icon);
        assert!(
            !settings.toggle_to_focus && !settings.keep_open_after_last_session_closes && !settings.open_after_start
        );
        assert!(!settings.remember_fullscreen);
        assert_eq!(settings.frames, 17);
    }

    #[test]
    fn given_out_of_range_values_when_clamping_then_they_are_forced_into_range() {
        let settings = WindowSettings {
            width_percent: 5,
            height_percent: 250,
            position_percent: 200,
            frames: 999,
            ..WindowSettings::default()
        }
        .clamped();
        assert_eq!(
            (settings.width_percent, settings.height_percent, settings.position_percent, settings.frames),
            (10, 100, 100, 50)
        );
    }

    #[test]
    fn given_in_range_values_when_clamping_then_nothing_changes() {
        assert_eq!(WindowSettings::default().clamped(), WindowSettings::default());
    }

    #[test]
    fn given_overrides_when_applying_then_listed_settings_change_and_others_keep_their_defaults() {
        let (settings, problems) =
            WindowSettings::default().with_overrides("frames=50+keep_open=false, width=70;screen=2");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!((settings.frames, settings.keep_open, settings.width_percent, settings.screen), (50, false, 70, 2));
        assert_eq!(settings.height_percent, WindowSettings::default().height_percent);
    }

    #[test]
    fn given_bad_overrides_when_applying_then_they_are_reported_and_ignored() {
        let (settings, problems) = WindowSettings::default().with_overrides("bogus=1+frames=lots+oops+width=5");
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert_eq!(settings.frames, 17);
        assert_eq!(settings.width_percent, 10, "valid but out-of-range values are clamped");
    }

    #[test]
    fn given_empty_overrides_when_applying_then_nothing_changes() {
        let (settings, problems) = WindowSettings::default().with_overrides("");
        assert!(problems.is_empty());
        assert_eq!(settings, WindowSettings::default());
    }

    #[test]
    fn given_keep_open_and_not_keep_above_when_asking_for_topmost_then_it_is_not_topmost() {
        let settings = WindowSettings { keep_open: true, keep_above: false, ..WindowSettings::default() };
        assert!(!settings.wants_topmost());
    }

    #[test]
    fn given_retract_on_focus_loss_when_asking_for_topmost_then_it_is_always_topmost() {
        let settings = WindowSettings { keep_open: false, keep_above: false, ..WindowSettings::default() };
        assert!(settings.wants_topmost());
        assert!(WindowSettings::default().wants_topmost());
    }
}
