// Ported from yakuake app/mainwindow.cpp (handleTerminalActivity, handleTerminalSilence) @21c6210
//! What to tell the user when a monitored terminal becomes active or goes quiet, and when to stay silent.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorKind {
    Activity,
    Silence,
}

/// The notification text. `tab_title` is the session's tab title.
pub fn notification_text(kind: MonitorKind, tab_title: &str) -> String {
    let what = match kind {
        MonitorKind::Activity => "Activity",
        MonitorKind::Silence => "Silence",
    };
    format!("{what} detected in monitored terminal in session \"{tab_title}\".")
}

/// Whether the user is already looking at the terminal that raised the event, which makes a notification pointless.
pub fn user_is_watching(window_open: bool, window_has_focus: bool, terminal_is_active_in_raised_session: bool) -> bool {
    window_open && window_has_focus && terminal_is_active_in_raised_session
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_activity_when_building_the_text_then_it_names_the_session() {
        assert_eq!(
            notification_text(MonitorKind::Activity, "build"),
            "Activity detected in monitored terminal in session \"build\"."
        );
    }

    #[test]
    fn given_silence_when_building_the_text_then_it_says_silence() {
        assert!(notification_text(MonitorKind::Silence, "x").starts_with("Silence detected"));
    }

    #[test]
    fn given_an_open_focused_window_on_the_active_terminal_when_deciding_then_the_user_is_watching() {
        assert!(user_is_watching(true, true, true));
        assert!(!user_is_watching(false, true, true));
        assert!(!user_is_watching(true, false, true));
        assert!(!user_is_watching(true, true, false));
    }
}
