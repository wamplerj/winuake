//! The context menu of a terminal (Konsole's, as Yakuake shows it): what entries it holds for the terminal's current state. Pure data;
//! the Windows side draws it and runs the chosen command.

use crate::actions::{Action, ShortcutMap};

/// One row of a menu. `C` is the command a chosen item stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry<C> {
    Item(MenuItem<C>),
    Submenu { label: String, entries: Vec<MenuEntry<C>> },
    Separator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem<C> {
    pub label: String,
    /// Shortcut text shown at the right edge.
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub command: C,
}

impl<C> MenuItem<C> {
    fn new(label: &str, command: C) -> Self {
        Self { label: label.to_owned(), shortcut: None, enabled: true, checked: false, command }
    }

    fn with_shortcut(mut self, shortcut: Option<String>) -> Self {
        self.shortcut = shortcut;
        self
    }

    fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMenuCommand {
    Copy,
    Paste,
    SplitLeftRight,
    SplitTopBottom,
    SplitAuto,
    CloseTerminal,
    ToggleReadOnly,
    ToggleMouseTracking,
    EditCurrentProfile,
    CloseSession,
}

/// What the menu needs to know about the terminal it was opened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMenuState {
    pub has_selection: bool,
    pub read_only: bool,
    pub mouse_tracking_allowed: bool,
}

/// Shortcut text the menu shows for the clipboard commands; they are fixed keys, not rebindable actions.
const COPY_SHORTCUT: &str = "Ctrl+Shift+C";
const PASTE_SHORTCUT: &str = "Ctrl+Shift+V";

/// The menu in Konsole's order. Groups are separated by lines; a group with nothing in it leaves no line behind.
pub fn terminal_menu(state: TerminalMenuState, shortcuts: &ShortcutMap) -> Vec<MenuEntry<TerminalMenuCommand>> {
    use TerminalMenuCommand::*;
    let shortcut = |action: Action| shortcuts.shortcut_for(action).map(|chord| chord.to_string());
    let item = |label: &str, command: TerminalMenuCommand| MenuEntry::Item(MenuItem::new(label, command));

    let mut clipboard = Vec::new();
    if state.has_selection {
        clipboard.push(MenuEntry::Item(MenuItem::new("Copy", Copy).with_shortcut(Some(COPY_SHORTCUT.to_owned()))));
    }
    clipboard.push(MenuEntry::Item(MenuItem::new("Paste", Paste).with_shortcut(Some(PASTE_SHORTCUT.to_owned()))));

    let splits = vec![MenuEntry::Submenu {
        label: "Split View".to_owned(),
        entries: vec![
            MenuEntry::Item(
                MenuItem::new("Split Left/Right", SplitLeftRight).with_shortcut(shortcut(Action::SplitLeftRight)),
            ),
            MenuEntry::Item(
                MenuItem::new("Split Top/Bottom", SplitTopBottom).with_shortcut(shortcut(Action::SplitTopBottom)),
            ),
            MenuEntry::Item(MenuItem::new("Split Automatically", SplitAuto).with_shortcut(shortcut(Action::SplitAuto))),
            MenuEntry::Separator,
            MenuEntry::Item(
                MenuItem::new("Close Terminal", CloseTerminal).with_shortcut(shortcut(Action::CloseActiveTerminal)),
            ),
        ],
    }];

    let switches = vec![
        MenuEntry::Item(MenuItem::new("Read-Only", ToggleReadOnly).checked(state.read_only)),
        MenuEntry::Item(
            MenuItem::new("Allow Mouse Tracking", ToggleMouseTracking).checked(state.mouse_tracking_allowed),
        ),
    ];

    let profile = vec![item("Edit Current Profile...", EditCurrentProfile)];
    let close = vec![item("Close Session", CloseSession)];

    join_groups(vec![clipboard, splits, switches, profile, close])
}

/// Flattens groups into one list with a separator between each pair of non-empty groups.
fn join_groups<C>(groups: Vec<Vec<MenuEntry<C>>>) -> Vec<MenuEntry<C>> {
    let mut entries = Vec::new();
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
        if !entries.is_empty() {
            entries.push(MenuEntry::Separator);
        }
        entries.extend(group);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TerminalMenuState {
        TerminalMenuState { has_selection: false, read_only: false, mouse_tracking_allowed: true }
    }

    fn top_level_labels(entries: &[MenuEntry<TerminalMenuCommand>]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Item(item) => item.label.clone(),
                MenuEntry::Submenu { label, .. } => format!("{label} >"),
                MenuEntry::Separator => "-".to_owned(),
            })
            .collect()
    }

    fn find_item(
        entries: &[MenuEntry<TerminalMenuCommand>],
        command: TerminalMenuCommand,
    ) -> Option<&MenuItem<TerminalMenuCommand>> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Item(item) if item.command == command => Some(item),
            MenuEntry::Submenu { entries, .. } => find_item(entries, command),
            _ => None,
        })
    }

    #[test]
    fn given_no_selection_when_building_the_menu_then_copy_is_absent_and_the_groups_follow_konsole() {
        let entries = terminal_menu(state(), &ShortcutMap::with_defaults());
        assert_eq!(
            top_level_labels(&entries),
            [
                "Paste",
                "-",
                "Split View >",
                "-",
                "Read-Only",
                "Allow Mouse Tracking",
                "-",
                "Edit Current Profile...",
                "-",
                "Close Session"
            ]
        );
    }

    #[test]
    fn given_a_selection_when_building_the_menu_then_copy_comes_before_paste() {
        let entries =
            terminal_menu(TerminalMenuState { has_selection: true, ..state() }, &ShortcutMap::with_defaults());
        assert_eq!(&top_level_labels(&entries)[..2], ["Copy", "Paste"]);
    }

    #[test]
    fn given_read_only_and_tracking_off_when_building_the_menu_then_the_check_marks_follow() {
        let entries = terminal_menu(
            TerminalMenuState { read_only: true, mouse_tracking_allowed: false, ..state() },
            &ShortcutMap::with_defaults(),
        );
        assert!(find_item(&entries, TerminalMenuCommand::ToggleReadOnly).is_some_and(|item| item.checked));
        assert!(find_item(&entries, TerminalMenuCommand::ToggleMouseTracking).is_some_and(|item| !item.checked));
    }

    #[test]
    fn given_the_default_shortcuts_when_building_the_menu_then_split_items_show_them() {
        let entries = terminal_menu(state(), &ShortcutMap::with_defaults());
        let split = find_item(&entries, TerminalMenuCommand::SplitLeftRight).expect("split item");
        assert_eq!(split.shortcut.as_deref(), Some("Ctrl+("));
    }

    #[test]
    fn given_an_unbound_split_shortcut_when_building_the_menu_then_the_item_shows_no_shortcut() {
        let mut shortcuts = ShortcutMap::with_defaults();
        shortcuts.unbind(Action::SplitAuto);
        let entries = terminal_menu(state(), &shortcuts);
        assert_eq!(find_item(&entries, TerminalMenuCommand::SplitAuto).and_then(|item| item.shortcut.clone()), None);
    }

    #[test]
    fn given_empty_groups_when_joining_then_separators_only_sit_between_filled_groups() {
        let item = |label: &str| MenuEntry::Item(MenuItem::new(label, 0_u8));
        let joined = join_groups(vec![Vec::new(), vec![item("a"), item("b")], Vec::new(), vec![item("c")], Vec::new()]);
        assert_eq!(joined, vec![item("a"), item("b"), MenuEntry::Separator, item("c")]);
    }
}
