//! Pure logic with no Windows dependencies: sessions, splits, and the drop-down window controller.
//! Ports Yakuake `sessionstack`, `session`, `splitter` and the window logic of `mainwindow`.

pub mod actions;
pub mod diagnostics;
pub mod drop_down;
pub mod ids;
pub mod notifications;
pub mod press_and_hold;
pub mod sessions;
pub mod shortcut;
