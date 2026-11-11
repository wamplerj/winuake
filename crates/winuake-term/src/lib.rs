//! ConPTY plus `alacritty_terminal`. Replaces the Konsole KPart used by Yakuake (`terminal.cpp`).

pub mod activity;
pub mod emulator;
pub mod process_launch;
pub mod pty;
pub mod session;
pub mod shell;

#[cfg(windows)]
pub mod conpty;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod conpty_bindings;
