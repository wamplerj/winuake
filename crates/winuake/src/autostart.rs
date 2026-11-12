//! Start with Windows: a value in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` pointing at this executable. A packaged (MSIX) build
//! declares a startup task in its manifest instead, which the user switches in Windows Settings, so nothing is written for it.

use crate::window_bindings::{
    ACCESS_MASK, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
    REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
};
use std::cell::Cell;
use windows_core::{PCWSTR, w};

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: PCWSTR = w!("Winuake");

thread_local! {
    static APPLIED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Makes the Run entry match `enabled`. Does nothing when that state was already applied.
pub fn apply(enabled: bool) {
    if APPLIED.with(|state| state.replace(Some(enabled))) == Some(enabled) {
        return;
    }
    if crate::bootstrap::is_packaged() {
        tracing::info!(enabled, "packaged build: autostart is managed by the package's startup task");
        return;
    }
    let result = if enabled { write_entry() } else { remove_entry() };
    match result {
        Ok(()) => tracing::info!(enabled, "autostart updated"),
        Err(code) => tracing::warn!(enabled, code, "could not update the autostart entry"),
    }
}

fn open_run_key() -> Result<HKEY, i32> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE as u32,
            ACCESS_MASK(KEY_SET_VALUE as u32),
            None,
            &mut key,
            None,
        )
    };
    if status.0 == ERROR_SUCCESS { Ok(key) } else { Err(status.0) }
}

fn write_entry() -> Result<(), i32> {
    let executable = std::env::current_exe().map_err(|_| -1)?;
    let command = format!("\"{}\"", executable.display());
    let key = open_run_key()?;
    let bytes: Vec<u8> = command.encode_utf16().chain(std::iter::once(0u16)).flat_map(u16::to_le_bytes).collect();
    let status = unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(&bytes)) };
    unsafe { RegCloseKey(key) };
    if status.0 == ERROR_SUCCESS { Ok(()) } else { Err(status.0) }
}

fn remove_entry() -> Result<(), i32> {
    let key = open_run_key()?;
    let status = unsafe { RegDeleteValueW(key, VALUE_NAME) };
    unsafe { RegCloseKey(key) };
    if status.0 == ERROR_SUCCESS || status.0 == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(status.0) }
}
