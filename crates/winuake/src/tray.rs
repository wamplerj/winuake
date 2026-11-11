//! The hidden message window, the notification-area icon with its menu, and the "toggle" request a second instance posts.
//!
//! The window is message-only (never visible) and lives on the UI thread, so its window procedure runs inside XAML's own message loop.

use crate::window_bindings::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, GetCursorPos,
    GetModuleHandleW, HINSTANCE, HWND, HWND_MESSAGE, IDI_APPLICATION, LPARAM, LRESULT, LoadIconW, MF_SEPARATOR,
    MF_STRING, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, POINT, PostMessageW,
    RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow, Shell_NotifyIconW, TPM_BOTTOMALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_APP, WM_LBUTTONUP, WM_RBUTTONUP, WM_USER, WNDCLASSEXW, WPARAM,
};
use std::cell::Cell;
use windows_core::{PCWSTR, Result, w};
use winuake_term::keyboard::WindowShortcut;

/// Class name other instances look up to ask this one to toggle.
pub const MESSAGE_WINDOW_CLASS: PCWSTR = w!("WinuakeMessageWindow");
/// Posted to the message window by a second instance (`WM_APP + 'W'`).
pub const WM_TOGGLE_REQUEST: u32 = WM_APP as u32 + 0x57;

const WM_TRAY_CALLBACK: u32 = WM_USER as u32 + 1;
const WM_NULL: u32 = 0;
const TRAY_ICON_ID: u32 = 1;
const MENU_TOGGLE: usize = 1;
const MENU_QUIT: usize = 2;

thread_local! {
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    /// Registered message Explorer broadcasts after it restarts, when the icon has to be added again.
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static ICON_SHOWN: Cell<bool> = const { Cell::new(false) };
}

/// Creates the message window (needed for second-instance requests even without an icon) and, if wanted, the tray icon.
pub fn install(show_icon: bool) -> Result<()> {
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(PCWSTR::null());
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(message_window_procedure),
            hInstance: instance,
            lpszClassName: MESSAGE_WINDOW_CLASS,
            ..WNDCLASSEXW::default()
        };
        RegisterClassExW(&class);
        let window = CreateWindowExW(
            0,
            MESSAGE_WINDOW_CLASS,
            w!("Winuake"),
            0,
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        );
        if window.0.is_null() {
            return Err(windows_core::Error::from_thread());
        }
        WINDOW.with(|slot| slot.set(Some(window)));
        TASKBAR_CREATED.with(|slot| slot.set(RegisterWindowMessageW(w!("TaskbarCreated"))));
    }
    ICON_SHOWN.with(|slot| slot.set(show_icon));
    if show_icon {
        add_icon();
    }
    Ok(())
}

/// Removes the icon and the window. Call when the app is exiting, or Explorer keeps a ghost icon until the pointer passes over it.
pub fn remove() {
    delete_icon();
    if let Some(window) = WINDOW.with(Cell::take) {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
}

fn icon_data(window: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: TRAY_ICON_ID,
        uFlags: (NIF_MESSAGE | NIF_ICON | NIF_TIP) as u32,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: unsafe { LoadIconW(None, IDI_APPLICATION) },
        ..NOTIFYICONDATAW::default()
    };
    for (slot, unit) in data.szTip.iter_mut().zip("Winuake".encode_utf16().chain(std::iter::once(0))) {
        *slot = unit;
    }
    data
}

fn add_icon() {
    let Some(window) = WINDOW.with(Cell::get) else { return };
    if !unsafe { Shell_NotifyIconW(NIM_ADD as u32, &icon_data(window)) }.as_bool() {
        tracing::warn!(error = %windows_core::Error::from_thread(), "could not add the tray icon");
    }
}

fn delete_icon() {
    if let Some(window) = WINDOW.with(Cell::get) {
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE as u32, &icon_data(window)) };
    }
}

fn show_menu(window: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        let _ = AppendMenuW(menu, MF_STRING as u32, MENU_TOGGLE, w!("Open / Retract Winuake"));
        let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING as u32, MENU_QUIT, w!("Quit Winuake"));

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // The menu only dismisses properly when its owner window is in the foreground.
        let _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD) as u32,
            cursor.x,
            cursor.y,
            window,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        match chosen {
            MENU_TOGGLE => crate::drop_down::toggle(),
            MENU_QUIT => crate::drop_down::dispatch_shortcut(WindowShortcut::Quit),
            _ => {}
        }
    }
}

unsafe extern "system" fn message_window_procedure(
    window: HWND,
    message: u32,
    w_parameter: WPARAM,
    l_parameter: LPARAM,
) -> LRESULT {
    match message {
        WM_TRAY_CALLBACK => match l_parameter.0 as u32 & 0xFFFF {
            event if event == WM_LBUTTONUP as u32 => crate::drop_down::toggle(),
            event if event == WM_RBUTTONUP as u32 => show_menu(window),
            _ => {}
        },
        WM_TOGGLE_REQUEST => {
            tracing::info!("another instance asked this one to toggle");
            crate::drop_down::toggle();
        }
        other if other == TASKBAR_CREATED.with(Cell::get) && other != 0 && ICON_SHOWN.with(Cell::get) => add_icon(),
        _ => return unsafe { DefWindowProcW(window, message, w_parameter, l_parameter) },
    }
    LRESULT(0)
}
