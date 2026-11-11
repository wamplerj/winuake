//! Winuake: drop-down terminal for Windows 11 (WinUI 3). Port of KDE Yakuake.

#[cfg(windows)]
mod application;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod bindings {
    include!("bindings.rs");
    include!("bindings_extensions.rs");
}
#[cfg(windows)]
mod bootstrap;
#[cfg(windows)]
mod chrome;
#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod composition;
#[cfg(windows)]
mod drop_down;
#[cfg(windows)]
mod first_run;
#[cfg(windows)]
mod message_hook;
#[cfg(windows)]
mod settings_store;
#[cfg(windows)]
mod settings_window;
#[cfg(windows)]
mod single_instance;
#[cfg(windows)]
mod skin_assets;
#[cfg(windows)]
mod skin_chrome;
#[cfg(windows)]
mod swap_chain_panel_native;
#[cfg(windows)]
mod terminal_view;
#[cfg(windows)]
mod title_drag;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod window_bindings;
#[cfg(windows)]
mod window_native;
#[cfg(windows)]
mod workspace_host;

/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
#[cfg(windows)]
const PER_MONITOR_AWARE_V2: isize = -4;

#[cfg(windows)]
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value: isize) -> i32);
#[cfg(windows)]
windows_core::link!("kernel32.dll" "system" fn TerminateProcess(process: isize, exit_code: u32) -> i32);

/// Ends the process without `ExitProcess`'s DLL detach phase. With XAML, DirectComposition and the swap chain loaded, that phase can
/// hang forever (a dead process that keeps its memory, handles, hotkeys and single-instance lock). Call only after our own cleanup.
#[cfg(windows)]
fn terminate_process_now(exit_code: u32) -> ! {
    const CURRENT_PROCESS: isize = -1;
    unsafe { TerminateProcess(CURRENT_PROCESS, exit_code) };
    // TerminateProcess on the current process does not return; this satisfies the type system if it somehow does.
    std::process::abort()
}

#[cfg(windows)]
fn main() -> windows_core::Result<()> {
    // Without this Windows treats the app as DPI-unaware: coordinates are virtualized to 100% and the whole window is bitmap-scaled.
    // It must happen before any window exists.
    if unsafe { SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) } == 0 {
        eprintln!("could not enable per-monitor DPI awareness: {}", windows_core::Error::from_thread());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "winuake=info,winuake_term=info".into()),
        )
        .init();
    if single_instance::claim() == single_instance::InstanceRole::AlreadyRunning {
        single_instance::request_toggle();
        return Ok(());
    }
    let runtime = bootstrap::WindowsAppRuntime::initialize()?;
    let outcome = application::run();
    tray::remove();
    // Shutting the Windows App SDK runtime down while XAML's threads are still alive hangs process exit (one thread waits forever and the
    // dead process keeps its memory, handles and single-instance lock). Everything we own is already cleaned up, so leave directly.
    std::mem::forget(runtime);
    terminate_process_now(u32::from(outcome.is_err()))
}

#[cfg(not(windows))]
fn main() {
    eprintln!("winuake only runs on Windows 11");
    std::process::exit(1);
}
