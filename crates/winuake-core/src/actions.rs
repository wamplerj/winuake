// Ported from yakuake app/mainwindow.cpp (setupActions) @21c6210
//! Every action Yakuake registers in its action collection, with its default shortcut.

use crate::shortcut::Chord;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    ViewFullScreen,
    Quit,
    ToggleWindowState,
    KeepOpen,
    ManageProfiles,
    EditProfile,
    IncreaseWindowWidth,
    DecreaseWindowWidth,
    IncreaseWindowHeight,
    DecreaseWindowHeight,
    NewSession,
    NewSessionTwoHorizontal,
    NewSessionTwoVertical,
    NewSessionQuad,
    CloseSession,
    PreviousSession,
    NextSession,
    MoveSessionLeft,
    MoveSessionRight,
    GrowTerminalRight,
    GrowTerminalLeft,
    GrowTerminalTop,
    GrowTerminalBottom,
    RenameSession,
    PreviousTerminal,
    NextTerminal,
    CloseActiveTerminal,
    SplitLeftRight,
    SplitTopBottom,
    SplitAuto,
    ToggleSessionPreventClosing,
    ToggleSessionKeyboardInput,
    ToggleSessionMonitorActivity,
    ToggleSessionMonitorSilence,
    ToggleTitlebar,
    /// One-based session position, 1..=10.
    SwitchToSession(u8),
}

/// Name and default shortcut text of each fixed action, in Yakuake's registration order.
const FIXED_ACTIONS: [(Action, &str, Option<&str>); 35] = [
    (Action::ViewFullScreen, "view-full-screen", Some("Ctrl+Shift+F11")),
    (Action::Quit, "file_quit", Some("Ctrl+Shift+Q")),
    (Action::ToggleWindowState, "toggle-window-state", Some("F12")),
    (Action::KeepOpen, "keep-open", None),
    (Action::ManageProfiles, "manage-profiles", None),
    (Action::EditProfile, "edit-profile", None),
    (Action::IncreaseWindowWidth, "increase-window-width", Some("Alt+Shift+Right")),
    (Action::DecreaseWindowWidth, "decrease-window-width", Some("Alt+Shift+Left")),
    (Action::IncreaseWindowHeight, "increase-window-height", Some("Alt+Shift+Down")),
    (Action::DecreaseWindowHeight, "decrease-window-height", Some("Alt+Shift+Up")),
    (Action::NewSession, "new-session", Some("Ctrl+Shift+T")),
    (Action::NewSessionTwoHorizontal, "new-session-two-horizontal", None),
    (Action::NewSessionTwoVertical, "new-session-two-vertical", None),
    (Action::NewSessionQuad, "new-session-quad", None),
    (Action::CloseSession, "close-session", Some("Ctrl+Shift+W")),
    (Action::PreviousSession, "previous-session", Some("Shift+Left")),
    (Action::NextSession, "next-session", Some("Shift+Right")),
    (Action::MoveSessionLeft, "move-session-left", Some("Ctrl+Shift+Left")),
    (Action::MoveSessionRight, "move-session-right", Some("Ctrl+Shift+Right")),
    (Action::GrowTerminalRight, "grow-terminal-right", Some("Ctrl+Alt+Right")),
    (Action::GrowTerminalLeft, "grow-terminal-left", Some("Ctrl+Alt+Left")),
    (Action::GrowTerminalTop, "grow-terminal-top", Some("Ctrl+Alt+Up")),
    (Action::GrowTerminalBottom, "grow-terminal-bottom", Some("Ctrl+Alt+Down")),
    (Action::RenameSession, "rename-session", Some("Ctrl+Alt+S")),
    (Action::PreviousTerminal, "previous-terminal", Some("Ctrl+Shift+Tab")),
    (Action::NextTerminal, "next-terminal", Some("Ctrl+Tab")),
    (Action::CloseActiveTerminal, "close-active-terminal", Some("Ctrl+Shift+R")),
    (Action::SplitLeftRight, "split-left-right", Some("Ctrl+(")),
    (Action::SplitTopBottom, "split-top-bottom", Some("Ctrl+)")),
    (Action::SplitAuto, "split-auto", Some("Ctrl+*")),
    (Action::ToggleSessionPreventClosing, "toggle-session-prevent-closing", None),
    (Action::ToggleSessionKeyboardInput, "toggle-session-keyboard-input", None),
    (Action::ToggleSessionMonitorActivity, "toggle-session-monitor-activity", Some("Ctrl+Shift+A")),
    (Action::ToggleSessionMonitorSilence, "toggle-session-monitor-silence", Some("Ctrl+Shift+I")),
    (Action::ToggleTitlebar, "toggle-titlebar", Some("Ctrl+Shift+M")),
];

const SWITCH_PREFIX: &str = "switch-to-session-";

impl Action {
    /// All 45 actions: the 35 fixed ones and `switch-to-session-1..10`.
    pub fn all() -> Vec<Action> {
        FIXED_ACTIONS.iter().map(|(action, _, _)| *action).chain((1..=10).map(Action::SwitchToSession)).collect()
    }

    /// Name as Yakuake's action collection stores it (and as the shortcut config will).
    pub fn name(self) -> String {
        match self {
            Action::SwitchToSession(number) => format!("{SWITCH_PREFIX}{number}"),
            fixed => Self::fixed_entry(fixed).1.to_owned(),
        }
    }

    pub fn from_name(name: &str) -> Option<Action> {
        if let Some(number) = name.strip_prefix(SWITCH_PREFIX) {
            return number.parse::<u8>().ok().filter(|number| (1..=10).contains(number)).map(Action::SwitchToSession);
        }
        FIXED_ACTIONS.iter().find(|(_, candidate, _)| *candidate == name).map(|(action, _, _)| *action)
    }

    /// Yakuake's default binding. Alt+1..Alt+9 for sessions 1..9 and Alt+0 for the tenth.
    pub fn default_shortcut(self) -> Option<Chord> {
        let text = match self {
            Action::SwitchToSession(number @ 1..=9) => return Chord::parse(&format!("Alt+{number}")).ok(),
            Action::SwitchToSession(10) => return Chord::parse("Alt+0").ok(),
            Action::SwitchToSession(_) => return None,
            fixed => Self::fixed_entry(fixed).2?,
        };
        Chord::parse(text).ok()
    }

    fn fixed_entry(action: Action) -> &'static (Action, &'static str, Option<&'static str>) {
        FIXED_ACTIONS.iter().find(|(candidate, _, _)| *candidate == action).expect("fixed action is in the table")
    }
}

/// Which chord triggers which action.
#[derive(Debug, Clone)]
pub struct ShortcutMap {
    bindings: HashMap<Chord, Action>,
}

impl ShortcutMap {
    /// Yakuake's default bindings. `toggle-window-state` is global and handled by the hotkey code, so it is left out here.
    pub fn with_defaults() -> Self {
        let bindings = Action::all()
            .into_iter()
            .filter(|action| *action != Action::ToggleWindowState)
            .filter_map(|action| action.default_shortcut().map(|chord| (chord, action)))
            .collect();
        Self { bindings }
    }

    pub fn lookup(&self, chord: Chord) -> Option<Action> {
        self.bindings.get(&chord).copied()
    }

    pub fn shortcut_for(&self, action: Action) -> Option<Chord> {
        self.bindings.iter().find(|(_, bound)| **bound == action).map(|(chord, _)| *chord)
    }

    /// Rebinds `action` to `chord`, dropping its old chord and whatever the new chord did before.
    pub fn bind(&mut self, action: Action, chord: Chord) {
        self.bindings.retain(|_, bound| *bound != action);
        self.bindings.insert(chord, action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_the_action_table_when_listing_all_then_there_are_45_with_unique_names() {
        let actions = Action::all();
        assert_eq!(actions.len(), 45);
        let mut names: Vec<String> = actions.iter().map(|action| action.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 45);
    }

    #[test]
    fn given_every_action_when_round_tripping_its_name_then_it_is_found() {
        for action in Action::all() {
            assert_eq!(Action::from_name(&action.name()), Some(action));
        }
        assert_eq!(Action::from_name("switch-to-session-11"), None);
        assert_eq!(Action::from_name("switch-to-session-x"), None);
        assert_eq!(Action::from_name("no-such-action"), None);
    }

    #[test]
    fn given_yakukes_defaults_when_asking_then_the_documented_chords_come_back() {
        let expected = [
            (Action::NewSession, "Ctrl+Shift+T"),
            (Action::CloseSession, "Ctrl+Shift+W"),
            (Action::NextSession, "Shift+Right"),
            (Action::MoveSessionLeft, "Ctrl+Shift+Left"),
            (Action::GrowTerminalTop, "Ctrl+Alt+Up"),
            (Action::SplitLeftRight, "Ctrl+("),
            (Action::SplitAuto, "Ctrl+*"),
            (Action::ToggleTitlebar, "Ctrl+Shift+M"),
            (Action::ViewFullScreen, "Ctrl+Shift+F11"),
            (Action::Quit, "Ctrl+Shift+Q"),
            (Action::SwitchToSession(1), "Alt+1"),
            (Action::SwitchToSession(9), "Alt+9"),
            (Action::SwitchToSession(10), "Alt+0"),
        ];
        for (action, text) in expected {
            assert_eq!(action.default_shortcut().map(|chord| chord.to_string()).as_deref(), Some(text), "{action:?}");
        }
        assert_eq!(Action::KeepOpen.default_shortcut(), None);
        assert_eq!(Action::SwitchToSession(11).default_shortcut(), None);
    }

    #[test]
    fn given_the_default_map_when_looking_up_a_chord_then_its_action_runs() {
        let map = ShortcutMap::with_defaults();
        assert_eq!(map.lookup(Chord::parse("Ctrl+Shift+T").unwrap()), Some(Action::NewSession));
        assert_eq!(map.lookup(Chord::parse("Alt+0").unwrap()), Some(Action::SwitchToSession(10)));
        assert_eq!(map.lookup(Chord::parse("Ctrl+Shift+C").unwrap()), None);
    }

    #[test]
    fn given_the_default_map_when_looking_up_f12_then_it_is_left_to_the_global_hotkey() {
        assert_eq!(ShortcutMap::with_defaults().lookup(Chord::parse("F12").unwrap()), None);
    }

    #[test]
    fn given_the_default_map_then_no_two_actions_share_a_chord() {
        let bound = Action::all()
            .into_iter()
            .filter(|action| *action != Action::ToggleWindowState)
            .filter_map(Action::default_shortcut)
            .count();
        assert_eq!(ShortcutMap::with_defaults().bindings.len(), bound);
    }

    #[test]
    fn given_a_rebinding_when_binding_then_the_old_chord_stops_working() {
        let mut map = ShortcutMap::with_defaults();
        let new_chord = Chord::parse("Ctrl+Alt+N").unwrap();
        map.bind(Action::NewSession, new_chord);
        assert_eq!(map.lookup(new_chord), Some(Action::NewSession));
        assert_eq!(map.lookup(Chord::parse("Ctrl+Shift+T").unwrap()), None);
        assert_eq!(map.shortcut_for(Action::NewSession), Some(new_chord));
        assert_eq!(map.shortcut_for(Action::KeepOpen), None);
    }
}
