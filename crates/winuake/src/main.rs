//! Winuake: drop-down terminal for Windows 11 (WinUI 3). Port of KDE Yakuake.

#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod bindings;
#[cfg(windows)]
mod bootstrap;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod composition;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod graphics;
#[cfg(windows)]
mod spike;
#[cfg(windows)]
mod swap_chain_clear;

#[cfg(windows)]
fn main() -> windows_core::Result<()> {
    let _runtime = bootstrap::WindowsAppRuntime::initialize()?;
    spike::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("winuake only runs on Windows 11");
    std::process::exit(1);
}
