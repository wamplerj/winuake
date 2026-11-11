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
mod clipboard;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod composition;
#[cfg(windows)]
mod drop_down;
#[cfg(windows)]
mod message_hook;
#[cfg(windows)]
mod swap_chain_panel_native;
#[cfg(windows)]
mod terminal_view;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod window_bindings;
#[cfg(windows)]
mod window_native;

/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
#[cfg(windows)]
const PER_MONITOR_AWARE_V2: isize = -4;

#[cfg(windows)]
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value: isize) -> i32);

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
    let _runtime = bootstrap::WindowsAppRuntime::initialize()?;
    application::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("winuake only runs on Windows 11");
    std::process::exit(1);
}
