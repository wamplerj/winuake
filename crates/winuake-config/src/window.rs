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
