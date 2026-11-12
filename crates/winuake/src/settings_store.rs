//! The settings file `%APPDATA%\Winuake\winuake.toml`: load, atomic save, change polling, and applying settings to the running app.
//!
//! Edits made in the file apply live. Edits made by the app (window size, keep-open, title bar, the settings window) are saved and applied
//! the same way. `WINUAKE_WINDOW_SETTINGS` still overrides window settings for testing; while it is set nothing is written back.

use crate::window_bindings::{KillTimer, SetTimer};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::SystemTime;
use winuake_config::settings::Settings;
use winuake_config::window::WindowSettings;

const POLL_INTERVAL_MS: u32 = 1000;

thread_local! {
    static STATE: RefCell<Option<StoreState>> = const { RefCell::new(None) };
    static POLL_TIMER: Cell<usize> = const { Cell::new(0) };
}

struct StoreState {
    path: PathBuf,
    /// The settings the app is running with, overrides included.
    current: Settings,
    /// What the file held when we last read or wrote it, to tell our own writes from outside edits.
    last_modified: Option<SystemTime>,
    window_overrides: Option<String>,
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Winuake").join("winuake.toml")
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()
}

fn with_overrides(mut settings: Settings, overrides: Option<&str>) -> Settings {
    let Some(overrides) = overrides else { return settings };
    let (window, problems) = settings.window.with_overrides(overrides);
    for problem in problems {
        tracing::warn!(%problem, "ignored a window setting override");
    }
    settings.window = window;
    settings
}

fn log_problems(settings_path: &std::path::Path, problems: &[winuake_config::settings::SettingsProblem]) {
    for problem in problems {
        tracing::warn!(path = %settings_path.display(), group = %problem.group, message = %problem.message, "settings problem");
    }
}

/// Reads the file (creating it with defaults when absent) and returns the settings to start with. Call once, first.
pub fn initialize() -> Settings {
    let path = settings_path();
    let window_overrides = std::env::var("WINUAKE_WINDOW_SETTINGS").ok();
    let loaded = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let loaded = Settings::parse(&text);
            log_problems(&path, &loaded.problems);
            loaded.settings
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(path = %path.display(), "no settings file yet; writing defaults");
            let defaults = Settings::default();
            if window_overrides.is_none() {
                write_file(&path, &defaults);
            }
            defaults
        }
        Err(error) => {
            tracing::error!(path = %path.display(), %error, "could not read the settings file; using defaults");
            Settings::default()
        }
    };
    let current = with_overrides(loaded, window_overrides.as_deref());
    let last_modified = modified(&path);
    STATE.with(|state| {
        *state.borrow_mut() = Some(StoreState { path, current: current.clone(), last_modified, window_overrides });
    });
    current
}

/// Reads the settings without copying them; `None` before `initialize`.
pub fn with_current<T>(read: impl FnOnce(&Settings) -> T) -> Option<T> {
    STATE.with(|state| state.borrow().as_ref().map(|state| read(&state.current)))
}

pub fn current() -> Settings {
    STATE.with(|state| state.borrow().as_ref().map(|state| state.current.clone())).unwrap_or_default()
}

/// Writes through a temporary file so a crash never leaves a half-written settings file.
fn write_file(path: &std::path::Path, settings: &Settings) -> bool {
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, settings.to_toml())?;
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = &result {
        tracing::error!(path = %path.display(), %error, "could not save settings");
    }
    result.is_ok()
}

fn save(state: &mut StoreState) {
    if state.window_overrides.is_some() {
        tracing::debug!("settings not saved while WINUAKE_WINDOW_SETTINGS is set");
        return;
    }
    if write_file(&state.path, &state.current) {
        state.last_modified = modified(&state.path);
        tracing::info!("settings saved");
    }
}

/// Changes settings from the app or settings window: saved, then applied everywhere.
pub fn update(change: impl FnOnce(&mut Settings)) {
    let settings = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut()?;
        change(&mut state.current);
        state.current.window = state.current.window.clone().clamped();
        save(state);
        Some(state.current.clone())
    });
    if let Some(settings) = settings {
        apply(&settings);
    }
}

/// The drop-down controller changed the window settings itself (resize shortcuts): save them without re-applying to the window.
pub fn persist_window(window: &WindowSettings) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else { return };
        state.current.window = window.clone();
        save(state);
    });
    crate::workspace_host::apply_settings(&current());
}

/// Starts checking the file for outside edits.
pub fn start_watching() {
    let timer = unsafe { SetTimer(None, 0, POLL_INTERVAL_MS, Some(poll_timer_procedure)) };
    POLL_TIMER.with(|slot| slot.set(timer));
}

pub fn stop_watching() {
    let timer = POLL_TIMER.with(Cell::take);
    if timer != 0 {
        let _ = unsafe { KillTimer(None, timer) };
    }
}

unsafe extern "system" fn poll_timer_procedure(
    _window: crate::window_bindings::HWND,
    _message: u32,
    _timer: usize,
    _tick: u32,
) {
    reload_if_changed();
}

fn reload_if_changed() {
    let reloaded = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut()?;
        let stamp = modified(&state.path)?;
        if state.last_modified == Some(stamp) {
            return None;
        }
        state.last_modified = Some(stamp);
        let text = std::fs::read_to_string(&state.path).ok()?;
        let loaded = Settings::parse(&text);
        log_problems(&state.path, &loaded.problems);
        if loaded.unreadable {
            tracing::warn!("settings file is not valid TOML; keeping the current settings");
            return None;
        }
        let next = with_overrides(loaded.settings, state.window_overrides.as_deref());
        if next == state.current {
            return None;
        }
        tracing::info!("settings file changed; applying");
        state.current = next.clone();
        Some(next)
    });
    if let Some(settings) = reloaded {
        apply(&settings);
    }
}

/// Pushes settings into every part of the running app.
fn apply(settings: &Settings) {
    crate::drop_down::apply_settings(settings);
    crate::workspace_host::apply_settings(settings);
    crate::tray::set_icon_visible(settings.window.show_tray_icon);
    crate::autostart::apply(settings.window.start_with_windows);
}
