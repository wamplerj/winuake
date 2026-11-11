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
mod swap_chain_panel_native;
#[cfg(windows)]
mod terminal_view;
#[cfg(windows)]
mod window_native;

#[cfg(windows)]
fn main() -> windows_core::Result<()> {
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
