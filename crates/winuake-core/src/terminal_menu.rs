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
    /// Opens the terminal's folder in File Explorer.
    OpenFileManager,
    OpenFolderIn(FolderTarget),
    /// Applies the profile at this position of `TerminalMenuState::profiles` to the terminal.
    SwitchProfile(usize),
    EditCurrentProfile,
    ClearScrollback,
    ClearScrollbackAndReset,
    /// Opens the profile settings, where the scrollback length lives.
    AdjustScrollback,
    CloseSession,
}

/// Where "Open Folder With" can send the terminal's folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderTarget {
    FileExplorer,
    VisualStudioCode,
    /// A new Winuake session starting in that folder.
    NewSession,
    /// Puts the folder's path on the clipboard.
    CopyPath,
}

/// One profile in the "Switch Profile" submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    pub name: String,
    /// The profile the terminal uses now.
    pub is_current: bool,
}

/// What the menu needs to know about the terminal it was opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMenuState {
    pub has_selection: bool,
    pub read_only: bool,
    pub mouse_tracking_allowed: bool,
    /// `code` is on the PATH, so "Open Folder With" can offer Visual Studio Code.
    pub visual_studio_code_available: bool,
    pub profiles: Vec<ProfileEntry>,
}

/// Shortcut text the menu shows for the clipboard commands; they are fixed keys, not rebindable actions.
const COPY_SHORTCUT: &str = "Ctrl+Shift+C";
const PASTE_SHORTCUT: &str = "Ctrl+Shift+V";

/// The menu in Konsole's order. Groups are separated by lines; a group with nothing in it leaves no line behind.
pub fn terminal_menu(state: &TerminalMenuState, shortcuts: &ShortcutMap) -> Vec<MenuEntry<TerminalMenuCommand>> {
    use TerminalMenuCommand::*;
    let shortcut = |action: Action| shortcuts.shortcut_for(action).map(|chord| chord.to_string());
    let item = |label: &str, command: TerminalMenuCommand| MenuEntry::Item(MenuItem::new(label, command));

    let mut clipboard = Vec::new();
    if state.has_selection {
        clipboard.push(MenuEntry::Item(MenuItem::new("Copy", Copy).with_shortcut(Some(COPY_SHORTCUT.to_owned()))));
    }
    clipboard.push(MenuEntry::Item(MenuItem::new("Paste", Paste).with_shortcut(Some(PASTE_SHORTCUT.to_owned()))));

    let mut open_with = vec![item("File Explorer", OpenFolderIn(FolderTarget::FileExplorer))];
    if state.visual_studio_code_available {
        open_with.push(item("Visual Studio Code", OpenFolderIn(FolderTarget::VisualStudioCode)));
    }
    open_with.push(MenuEntry::Separator);
    open_with.push(item("New Winuake Session Here", OpenFolderIn(FolderTarget::NewSession)));
    open_with.push(item("Copy Path", OpenFolderIn(FolderTarget::CopyPath)));
    let folder = vec![
        item("Open File Manager", OpenFileManager),
        MenuEntry::Submenu { label: "Open Folder With".to_owned(), entries: open_with },
    ];

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

    let scrollback = vec![MenuEntry::Submenu {
        label: "Scrollback".to_owned(),
        entries: vec![
            MenuEntry::Item(
                MenuItem::new("Clear Scrollback", ClearScrollback).with_shortcut(shortcut(Action::ClearScrollback)),
            ),
            MenuEntry::Item(
                MenuItem::new("Clear Scrollback and Reset", ClearScrollbackAndReset)
                    .with_shortcut(shortcut(Action::ClearScrollbackAndReset)),
            ),
            MenuEntry::Separator,
            item("Adjust Scrollback...", AdjustScrollback),
        ],
    }];

    let switches = vec![
        MenuEntry::Item(MenuItem::new("Read-Only", ToggleReadOnly).checked(state.read_only)),
        MenuEntry::Item(
            MenuItem::new("Allow Mouse Tracking", ToggleMouseTracking).checked(state.mouse_tracking_allowed),
        ),
    ];

    let mut profile = Vec::new();
    if !state.profiles.is_empty() {
        let choices = state
            .profiles
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                MenuEntry::Item(MenuItem::new(&entry.name, SwitchProfile(index)).checked(entry.is_current))
            })
            .collect();
        profile.push(MenuEntry::Submenu { label: "Switch Profile".to_owned(), entries: choices });
    }
    profile.push(item("Edit Current Profile...", EditCurrentProfile));
    let close = vec![item("Close Session", CloseSession)];

    join_groups(vec![clipboard, folder, splits, scrollback, switches, profile, close])
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
        TerminalMenuState {
            has_selection: false,
            read_only: false,
            mouse_tracking_allowed: true,
            visual_studio_code_available: true,
            profiles: vec![
                ProfileEntry { name: "PowerShell 7".to_owned(), is_current: true },
                ProfileEntry { name: "Git Bash".to_owned(), is_current: false },
            ],
        }
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
        let entries = terminal_menu(&state(), &ShortcutMap::with_defaults());
        assert_eq!(
            top_level_labels(&entries),
            [
                "Paste",
                "-",
                "Open File Manager",
                "Open Folder With >",
                "-",
                "Split View >",
                "-",
                "Scrollback >",
                "-",
                "Read-Only",
                "Allow Mouse Tracking",
                "-",
                "Switch Profile >",
                "Edit Current Profile...",
                "-",
                "Close Session"
            ]
        );
    }

    #[test]
    fn given_a_selection_when_building_the_menu_then_copy_comes_before_paste() {
        let entries =
            terminal_menu(&TerminalMenuState { has_selection: true, ..state() }, &ShortcutMap::with_defaults());
        assert_eq!(&top_level_labels(&entries)[..2], ["Copy", "Paste"]);
    }

    #[test]
    fn given_read_only_and_tracking_off_when_building_the_menu_then_the_check_marks_follow() {
        let entries = terminal_menu(
            &TerminalMenuState { read_only: true, mouse_tracking_allowed: false, ..state() },
            &ShortcutMap::with_defaults(),
        );
        assert!(find_item(&entries, TerminalMenuCommand::ToggleReadOnly).is_some_and(|item| item.checked));
        assert!(find_item(&entries, TerminalMenuCommand::ToggleMouseTracking).is_some_and(|item| !item.checked));
    }

    #[test]
    fn given_the_default_shortcuts_when_building_the_menu_then_split_items_show_them() {
        let entries = terminal_menu(&state(), &ShortcutMap::with_defaults());
        let split = find_item(&entries, TerminalMenuCommand::SplitLeftRight).expect("split item");
        assert_eq!(split.shortcut.as_deref(), Some("Ctrl+("));
    }

    #[test]
    fn given_an_unbound_split_shortcut_when_building_the_menu_then_the_item_shows_no_shortcut() {
        let mut shortcuts = ShortcutMap::with_defaults();
        shortcuts.unbind(Action::SplitAuto);
        let entries = terminal_menu(&state(), &shortcuts);
        assert_eq!(find_item(&entries, TerminalMenuCommand::SplitAuto).and_then(|item| item.shortcut.clone()), None);
    }

    #[test]
    fn given_profiles_when_building_the_menu_then_switch_profile_lists_them_with_the_current_one_checked() {
        let entries = terminal_menu(&state(), &ShortcutMap::with_defaults());
        let current = find_item(&entries, TerminalMenuCommand::SwitchProfile(0)).expect("first profile");
        let other = find_item(&entries, TerminalMenuCommand::SwitchProfile(1)).expect("second profile");
        assert_eq!((current.label.as_str(), current.checked), ("PowerShell 7", true));
        assert_eq!((other.label.as_str(), other.checked), ("Git Bash", false));
    }

    #[test]
    fn given_no_profiles_when_building_the_menu_then_there_is_no_switch_profile_submenu() {
        let entries =
            terminal_menu(&TerminalMenuState { profiles: Vec::new(), ..state() }, &ShortcutMap::with_defaults());
        assert!(!top_level_labels(&entries).contains(&"Switch Profile >".to_owned()));
    }

    #[test]
    fn given_the_default_shortcuts_when_building_the_menu_then_clear_and_reset_shows_ctrl_shift_k() {
        let entries = terminal_menu(&state(), &ShortcutMap::with_defaults());
        let reset = find_item(&entries, TerminalMenuCommand::ClearScrollbackAndReset).expect("reset item");
        assert_eq!(reset.shortcut.as_deref(), Some("Ctrl+Shift+K"));
        assert_eq!(
            find_item(&entries, TerminalMenuCommand::ClearScrollback).and_then(|item| item.shortcut.clone()),
            None
        );
    }

    #[test]
    fn given_visual_studio_code_is_missing_when_building_the_menu_then_open_folder_with_does_not_offer_it() {
        let with_code = terminal_menu(&state(), &ShortcutMap::with_defaults());
        let without_code = terminal_menu(
            &TerminalMenuState { visual_studio_code_available: false, ..state() },
            &ShortcutMap::with_defaults(),
        );
        let code = TerminalMenuCommand::OpenFolderIn(FolderTarget::VisualStudioCode);
        assert!(find_item(&with_code, code).is_some());
        assert!(find_item(&without_code, code).is_none());
        assert!(find_item(&without_code, TerminalMenuCommand::OpenFolderIn(FolderTarget::FileExplorer)).is_some());
    }

    #[test]
    fn given_the_menu_when_looking_in_open_folder_with_then_new_session_and_copy_path_are_there() {
        let entries = terminal_menu(&state(), &ShortcutMap::with_defaults());
        assert!(find_item(&entries, TerminalMenuCommand::OpenFolderIn(FolderTarget::NewSession)).is_some());
        assert!(find_item(&entries, TerminalMenuCommand::OpenFolderIn(FolderTarget::CopyPath)).is_some());
        assert!(find_item(&entries, TerminalMenuCommand::OpenFileManager).is_some());
    }

    #[test]
    fn given_empty_groups_when_joining_then_separators_only_sit_between_filled_groups() {
        let item = |label: &str| MenuEntry::Item(MenuItem::new(label, 0_u8));
        let joined = join_groups(vec![Vec::new(), vec![item("a"), item("b")], Vec::new(), vec![item("c")], Vec::new()]);
        assert_eq!(joined, vec![item("a"), item("b"), MenuEntry::Separator, item("c")]);
    }
}
