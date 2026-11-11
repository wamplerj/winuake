//! One Winuake per user session. Starting it again asks the running copy to toggle (like launching Yakuake a second time).

use crate::tray::{MESSAGE_WINDOW_CLASS, WM_TOGGLE_REQUEST};
use crate::window_bindings::{
    CreateMutexW, ERROR_ALREADY_EXISTS, FindWindowExW, GetLastError, HWND_MESSAGE, LPARAM, PostMessageW, WPARAM,
};
use std::thread;
use std::time::Duration;
use windows_core::{PCWSTR, w};

const LOOKUP_ATTEMPTS: u32 = 10;
const LOOKUP_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    First,
    AlreadyRunning,
}

/// Takes the session-wide instance lock. The lock is held until the process exits.
/// `WINUAKE_ALLOW_MULTIPLE` (any value) skips the check, for testing next to a stuck copy.
pub fn claim() -> InstanceRole {
    if std::env::var_os("WINUAKE_ALLOW_MULTIPLE").is_some() {
        return InstanceRole::First;
    }
    unsafe {
        let _mutex = CreateMutexW(None, false, w!("Local\\Winuake.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS as u32 { InstanceRole::AlreadyRunning } else { InstanceRole::First }
    }
}

/// Tells the running instance to toggle. Retries briefly because that instance may still be starting up.
pub fn request_toggle() {
    for _ in 0..LOOKUP_ATTEMPTS {
        let window = unsafe { FindWindowExW(Some(HWND_MESSAGE), None, MESSAGE_WINDOW_CLASS, PCWSTR::null()) };
        if !window.0.is_null() {
            let _ = unsafe { PostMessageW(Some(window), WM_TOGGLE_REQUEST, WPARAM(0), LPARAM(0)) };
            return;
        }
        thread::sleep(LOOKUP_DELAY);
    }
    eprintln!("Winuake is already running but did not answer");
}
