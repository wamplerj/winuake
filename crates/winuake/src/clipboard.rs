//! Plain-text clipboard through Win32. Synchronous on purpose: the WinRT clipboard API is asynchronous and cannot be awaited on the UI
//! thread, and a terminal needs paste to be immediate.

use core::ffi::c_void;
use std::thread;
use std::time::Duration;
use windows_core::{Error, Result};

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const OPEN_ATTEMPTS: u32 = 8;
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(15);

windows_core::link!("user32.dll" "system" fn OpenClipboard(owner: isize) -> i32);
windows_core::link!("user32.dll" "system" fn CloseClipboard() -> i32);
windows_core::link!("user32.dll" "system" fn EmptyClipboard() -> i32);
windows_core::link!("user32.dll" "system" fn SetClipboardData(format: u32, memory: isize) -> isize);
windows_core::link!("user32.dll" "system" fn GetClipboardData(format: u32) -> isize);
windows_core::link!("kernel32.dll" "system" fn GlobalAlloc(flags: u32, bytes: usize) -> isize);
windows_core::link!("kernel32.dll" "system" fn GlobalFree(memory: isize) -> isize);
windows_core::link!("kernel32.dll" "system" fn GlobalLock(memory: isize) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalUnlock(memory: isize) -> i32);

/// Keeps the clipboard open for the duration of a scope. Another program may hold it briefly, so opening retries.
struct OpenClipboardGuard;

impl OpenClipboardGuard {
    fn open(owner: isize) -> Result<Self> {
        for attempt in 0..OPEN_ATTEMPTS {
            if unsafe { OpenClipboard(owner) } != 0 {
                return Ok(Self);
            }
            if attempt + 1 < OPEN_ATTEMPTS {
                thread::sleep(OPEN_RETRY_DELAY);
            }
        }
        Err(Error::from_thread())
    }
}

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

/// Replaces the clipboard with `text`. `owner` is the window handle of the application window.
pub fn set_text(owner: isize, text: &str) -> Result<()> {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = units.len() * size_of::<u16>();
    let _clipboard = OpenClipboardGuard::open(owner)?;
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if memory == 0 {
            return Err(Error::from_thread());
        }
        let destination = GlobalLock(memory).cast::<u16>();
        if destination.is_null() {
            GlobalFree(memory);
            return Err(Error::from_thread());
        }
        core::ptr::copy_nonoverlapping(units.as_ptr(), destination, units.len());
        GlobalUnlock(memory);

        EmptyClipboard();
        // On success the clipboard owns `memory`; on failure it is still ours to free.
        if SetClipboardData(CF_UNICODETEXT, memory) == 0 {
            let error = Error::from_thread();
            GlobalFree(memory);
            return Err(error);
        }
    }
    Ok(())
}

/// The clipboard's text, or `None` when it holds no text.
pub fn get_text(owner: isize) -> Result<Option<String>> {
    let _clipboard = OpenClipboardGuard::open(owner)?;
    unsafe {
        let memory = GetClipboardData(CF_UNICODETEXT);
        if memory == 0 {
            return Ok(None);
        }
        let source = GlobalLock(memory).cast::<u16>();
        if source.is_null() {
            return Err(Error::from_thread());
        }
        let mut length = 0;
        while *source.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(core::slice::from_raw_parts(source, length));
        GlobalUnlock(memory);
        Ok(Some(text))
    }
}
