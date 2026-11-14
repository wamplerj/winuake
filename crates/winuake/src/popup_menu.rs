//! Win32 popup menus: shows one at the cursor and reports the choice. `show_entries` draws a `winuake_core::terminal_menu` entry list
//! (items, separators, submenus) and returns the command of the chosen item.

use crate::window_bindings::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, HWND, LPARAM, MF_CHECKED, MF_GRAYED, MF_POPUP,
    MF_SEPARATOR, MF_STRING, POINT, PostMessageW, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenuEx, WPARAM,
};
use windows_core::{HSTRING, PCWSTR};
use winuake_core::terminal_menu::MenuEntry;

/// Message that does nothing; posted after a popup menu so it dismisses properly.
const WM_NULL: u32 = 0;

pub fn flags(enabled: bool, checked: bool) -> u32 {
    let mut value = MF_STRING as u32;
    if !enabled {
        value |= MF_GRAYED as u32;
    }
    if checked {
        value |= MF_CHECKED as u32;
    }
    value
}

/// Shows a popup menu at the cursor and returns the chosen command id (0 for none).
pub fn track_menu(fill: impl FnOnce(HMENU)) -> usize {
    let owner = HWND(crate::workspace_host::window_handle() as *mut core::ffi::c_void);
    unsafe {
        let menu = CreatePopupMenu();
        fill(menu);
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // The menu only dismisses properly when its owner window is in the foreground.
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenuEx(menu, (TPM_RIGHTBUTTON | TPM_RETURNCMD) as u32, cursor.x, cursor.y, owner, None).0
            as usize;
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen
    }
}

/// Shows `entries` at the cursor; the command of the chosen item, or `None` when the menu was dismissed.
pub fn show_entries<C: Copy>(entries: &[MenuEntry<C>]) -> Option<C> {
    let mut commands: Vec<C> = Vec::new();
    let chosen = track_menu(|menu| append_entries(menu, entries, &mut commands));
    // Command ids are one-based positions in `commands`; 0 means nothing was chosen.
    chosen.checked_sub(1).and_then(|index| commands.get(index).copied())
}

fn append_entries<C: Copy>(menu: HMENU, entries: &[MenuEntry<C>], commands: &mut Vec<C>) {
    for entry in entries {
        unsafe {
            match entry {
                MenuEntry::Item(item) => {
                    commands.push(item.command);
                    let label = match &item.shortcut {
                        Some(shortcut) => format!("{}\t{shortcut}", item.label),
                        None => item.label.clone(),
                    };
                    let _ = AppendMenuW(menu, flags(item.enabled, item.checked), commands.len(), &HSTRING::from(label));
                }
                MenuEntry::Submenu { label, entries } => {
                    let submenu = CreatePopupMenu();
                    append_entries(submenu, entries, commands);
                    let _ = AppendMenuW(menu, MF_POPUP as u32, submenu.0 as usize, &HSTRING::from(label.as_str()));
                }
                MenuEntry::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
                }
            }
        }
    }
}
