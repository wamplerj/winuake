//! A thread-level `WH_GETMESSAGE` hook: sees every message the UI thread pulls from its queue, before XAML does.
//!
//! Used for text (`WM_CHAR`/`WM_SYSCHAR`, because XAML's `CharacterReceived` is never raised for this unpackaged app) and for the
//! global hotkey and cross-instance commands, which are posted to the thread rather than a window.

use windows_core::Result;

const WH_GETMESSAGE: i32 = 3;
const HC_ACTION: i32 = 0;
const PM_REMOVE: usize = 1;
const WM_CHAR: u32 = 0x0102;
const WM_SYSCHAR: u32 = 0x0106;
const WM_HOTKEY: u32 = 0x0312;

#[repr(C)]
struct WindowMessage {
    window: isize,
    message: u32,
    w_parameter: usize,
    l_parameter: isize,
    time: u32,
    point_x: i32,
    point_y: i32,
}

type HookCallback = unsafe extern "system" fn(code: i32, w_parameter: usize, l_parameter: isize) -> isize;

windows_core::link!("user32.dll" "system" fn SetWindowsHookExW(hook_kind: i32, callback: HookCallback, module: isize, thread_id: u32) -> isize);
windows_core::link!("user32.dll" "system" fn CallNextHookEx(hook: isize, code: i32, w_parameter: usize, l_parameter: isize) -> isize);
windows_core::link!("kernel32.dll" "system" fn GetCurrentThreadId() -> u32);

/// Installs the hook on the calling (UI) thread. Call once.
pub fn install() -> Result<()> {
    let hook = unsafe { SetWindowsHookExW(WH_GETMESSAGE, message_hook, 0, GetCurrentThreadId()) };
    if hook == 0 {
        return Err(windows_core::Error::from_thread());
    }
    Ok(())
}

unsafe extern "system" fn message_hook(code: i32, w_parameter: usize, l_parameter: isize) -> isize {
    if code == HC_ACTION && w_parameter == PM_REMOVE {
        let message = unsafe { &*(l_parameter as *const WindowMessage) };
        match message.message {
            WM_CHAR | WM_SYSCHAR => crate::terminal_view::handle_text_message(message.w_parameter as u16),
            WM_HOTKEY => crate::drop_down::handle_hotkey(message.w_parameter as i32),
            other => crate::drop_down::handle_thread_message(other),
        }
    }
    unsafe { CallNextHookEx(0, code, w_parameter, l_parameter) }
}
