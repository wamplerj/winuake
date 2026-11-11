//! The drop-down behavior on Windows: executes the commands of `winuake_core::drop_down::DropDownController` against the real window.
//!
//! Frameless popup window hidden from the taskbar and Alt+Tab, work-area-aware placement, a slide animation done by moving the full-size
//! window partly above the work area and clipping the overhang with a window region, a global hotkey, and focus-loss retraction.

use crate::window_bindings::{
    AttachThreadInput, BringWindowToTop, CoCreateInstance, CreateRectRgn, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_DONOTROUND, DwmSetWindowAttribute, EnumDisplayMonitors, GWL_EXSTYLE, GWL_STYLE, GetCurrentProcessId,
    GetCurrentThreadId, GetCursorPos, GetForegroundWindow, GetMonitorInfoW, GetWindowLongPtrW,
    GetWindowThreadProcessId, HDC, HMONITOR, HWND, HWND_NOTOPMOST, HWND_TOPMOST, IVirtualDesktopManager, KillTimer,
    LPARAM, MOD_NOREPEAT, MONITORINFO, POINT, RECT, RegisterHotKey, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, SetWindowRgn, ShowWindow, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    WS_POPUP,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;
use windows_core::{BOOL, GUID, IUnknown, Result};
use winuake_config::window::WindowSettings;
use winuake_core::drop_down::{
    DropDownCommand, DropDownController, DropDownEvent, FRAME_INTERVAL, Monitor, PercentStep, PlacementRequest, Point,
    Rect, SlideFrame, WindowContext, order_monitors, window_rect,
};
use winuake_term::keyboard::WindowShortcut;

const TOGGLE_HOTKEY_ID: i32 = 1;
const VK_F12: u32 = 0x7B;
const VK_OEM_3: u32 = 0xC0;
const MOD_WIN: u32 = 0x0008;
const MOD_CONTROL: u32 = 0x0002;
const MOD_ALT: u32 = 0x0001;
const FOCUS_POLL_INTERVAL_MS: u32 = 100;
const CLSCTX_ALL: u32 = 0x17;
const MONITORINFOF_PRIMARY: u32 = 1;

struct HotkeyCandidate {
    name: &'static str,
    modifiers: u32,
    virtual_key: u32,
}

const TOGGLE_HOTKEY_CANDIDATES: [HotkeyCandidate; 4] = [
    HotkeyCandidate { name: "F12", modifiers: 0, virtual_key: VK_F12 },
    HotkeyCandidate { name: "Win+`", modifiers: MOD_WIN, virtual_key: VK_OEM_3 },
    HotkeyCandidate { name: "Ctrl+`", modifiers: MOD_CONTROL, virtual_key: VK_OEM_3 },
    HotkeyCandidate { name: "Ctrl+Alt+`", modifiers: MOD_CONTROL | MOD_ALT, virtual_key: VK_OEM_3 },
];

/// CLSID_VirtualDesktopManager.
const VIRTUAL_DESKTOP_MANAGER: GUID = GUID::from_u128(0xaa509086_5ca9_4c25_8f95_589d3c07b48a);

thread_local! {
    static HOST: RefCell<Option<Rc<DropDownHost>>> = const { RefCell::new(None) };
}

fn with_host(action: impl FnOnce(&DropDownHost)) {
    let host = HOST.with(|slot| slot.borrow().clone());
    if let Some(host) = host {
        action(&host);
    }
}

pub struct DropDownHost {
    window: HWND,
    controller: RefCell<DropDownController>,
    desktop_manager: Option<IVirtualDesktopManager>,
    /// Gives keyboard focus to the terminal once the window is foreground.
    focus_terminal: Box<dyn Fn()>,
    animation_timer: Cell<usize>,
    animation_started: Cell<Option<Instant>>,
    focus_timer: Cell<usize>,
    last_focus_state: Cell<Option<(bool, bool)>>,
}

impl DropDownHost {
    /// Takes over `window_handle`: restyles it, parks it hidden, registers the global hotkey, and installs the host on this thread.
    pub fn install(window_handle: isize, settings: WindowSettings, focus_terminal: Box<dyn Fn()>) -> Result<Rc<Self>> {
        let window = HWND(window_handle as *mut core::ffi::c_void);
        let desktop_manager = unsafe {
            CoCreateInstance::<_, IVirtualDesktopManager>(&VIRTUAL_DESKTOP_MANAGER, None::<&IUnknown>, CLSCTX_ALL)
        }
        .inspect_err(|error| tracing::warn!(%error, "virtual desktop manager unavailable; desktop handling disabled"))
        .ok();
        let host = Rc::new(Self {
            window,
            controller: RefCell::new(DropDownController::new(settings)),
            desktop_manager,
            focus_terminal,
            animation_timer: Cell::new(0),
            animation_started: Cell::new(None),
            focus_timer: Cell::new(0),
            last_focus_state: Cell::new(None),
        });
        host.apply_popup_style();
        HOST.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&host)));

        host.register_toggle_hotkey();
        Ok(host)
    }

    /// Yakuake's default is F12, but Windows reserves it for debuggers and may refuse it, so fall back to Win+` (Windows Terminal's quake key).
    fn register_toggle_hotkey(&self) {
        for candidate in TOGGLE_HOTKEY_CANDIDATES {
            if unsafe {
                RegisterHotKey(None, TOGGLE_HOTKEY_ID, candidate.modifiers | MOD_NOREPEAT as u32, candidate.virtual_key)
            }
            .as_bool()
            {
                tracing::info!(hotkey = candidate.name, "registered global toggle hotkey");
                return;
            }
            tracing::warn!(hotkey = candidate.name, error = %windows_core::Error::from_thread(), "could not register toggle hotkey");
        }
        tracing::error!("no toggle hotkey could be registered; use the tray icon or start the app again to toggle");
    }

    /// Positions the window off screen at its eventual size so XAML can lay out and the terminal gets its real grid, before hiding it.
    pub fn park_for_first_layout(&self) {
        let monitors = enumerate_monitors();
        let settings = self.controller.borrow().settings().clone();
        if let Some(monitor) = monitors.first() {
            let rect = window_rect(
                monitor,
                PlacementRequest {
                    width_percent: settings.width_percent,
                    height_percent: settings.height_percent,
                    position_percent: settings.position_percent,
                    fullscreen: false,
                },
            );
            self.set_position(Rect::from_origin_size(-32_000, -32_000, rect.width(), rect.height()));
        }
    }

    pub fn hide_now(&self) {
        tracing::info!("hiding drop-down window");
        unsafe {
            let _ = SetWindowRgn(self.window, None, false);
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }

    pub fn toggle(&self) {
        self.dispatch(DropDownEvent::Toggle);
    }

    pub fn dispatch(&self, event: DropDownEvent) {
        let monitors = enumerate_monitors();
        let context = WindowContext {
            monitors: &monitors,
            pointer: cursor_position(),
            window_is_active: unsafe { GetForegroundWindow() } == self.window,
            window_on_other_desktop: self.is_on_other_desktop(),
        };
        let commands = self.controller.borrow_mut().handle(event, &context);
        for command in commands {
            self.execute(command);
        }
    }

    fn execute(&self, command: DropDownCommand) {
        match command {
            DropDownCommand::PositionWindow(frame) => self.position(frame),
            DropDownCommand::ShowWindow => unsafe {
                tracing::info!("showing drop-down window");
                let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
            },
            DropDownCommand::HideWindow => self.hide_now(),
            DropDownCommand::ActivateWindow => self.activate(),
            DropDownCommand::MoveToCurrentDesktop => self.move_to_current_desktop(),
            DropDownCommand::SetTopmost(topmost) => unsafe {
                let order = if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST };
                let _ = SetWindowPos(
                    self.window,
                    Some(order),
                    0,
                    0,
                    0,
                    0,
                    (SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) as u32,
                );
            },
            DropDownCommand::StartAnimationTimer => {
                self.animation_started.set(Some(Instant::now()));
                self.stop_timer(&self.animation_timer);
                self.animation_timer
                    .set(unsafe { SetTimer(None, 0, FRAME_INTERVAL.as_millis() as u32, Some(animation_timer_proc)) });
            }
            DropDownCommand::StopAnimationTimer => {
                self.stop_timer(&self.animation_timer);
                self.animation_started.set(None);
            }
            DropDownCommand::StartFocusWatch => {
                self.last_focus_state.set(None);
                self.stop_timer(&self.focus_timer);
                self.focus_timer.set(unsafe { SetTimer(None, 0, FOCUS_POLL_INTERVAL_MS, Some(focus_timer_proc)) });
            }
            DropDownCommand::StopFocusWatch => self.stop_timer(&self.focus_timer),
            DropDownCommand::PersistSettings(settings) => {
                tracing::info!(
                    width = settings.width_percent,
                    height = settings.height_percent,
                    "window settings changed (not saved until Phase 5)"
                );
            }
        }
    }

    fn stop_timer(&self, timer: &Cell<usize>) {
        let id = timer.replace(0);
        if id != 0 {
            unsafe {
                let _ = KillTimer(None, id);
            }
        }
    }

    fn apply_popup_style(&self) {
        unsafe {
            let style = (WS_POPUP | WS_CLIPCHILDREN as u32 | WS_CLIPSIBLINGS as u32) as isize;
            SetWindowLongPtrW(self.window, GWL_STYLE, style);
            let extended = GetWindowLongPtrW(self.window, GWL_EXSTYLE);
            // Tool windows stay out of the taskbar and Alt+Tab (Yakuake sets SkipTaskbar and SkipPager).
            SetWindowLongPtrW(
                self.window,
                GWL_EXSTYLE,
                (extended | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize),
            );
            let _ = SetWindowPos(
                self.window,
                None,
                0,
                0,
                0,
                0,
                (SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) as u32,
            );
            // A drop-down is flush with the screen edge; rounded corners would leave gaps.
            let preference = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                self.window,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&raw const preference).cast(),
                size_of_val(&preference) as u32,
            );
        }
    }

    fn set_position(&self, rect: Rect) {
        unsafe {
            let _ = SetWindowPos(
                self.window,
                None,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                (SWP_NOZORDER | SWP_NOACTIVATE) as u32,
            );
        }
    }

    fn position(&self, frame: SlideFrame) {
        let rect = frame.rect;
        if frame.clip_top == 0 {
            tracing::info!(
                left = rect.left,
                top = rect.top,
                width = rect.width(),
                height = rect.height(),
                "drop-down window placed"
            );
        } else {
            tracing::debug!(top = rect.top, clip_top = frame.clip_top, "drop-down slide frame");
        }
        self.set_position(frame.rect);
        unsafe {
            if frame.clip_top > 0 {
                // The system owns the region after SetWindowRgn.
                let region = CreateRectRgn(0, frame.clip_top, frame.rect.width(), frame.rect.height());
                let _ = SetWindowRgn(self.window, Some(region), true);
            } else {
                let _ = SetWindowRgn(self.window, None, true);
            }
        }
    }

    /// Windows only lets the foreground process change the foreground window; briefly sharing input state with it gets around that
    /// for a window the user explicitly asked for.
    fn activate(&self) {
        unsafe {
            let foreground = GetForegroundWindow();
            let foreground_thread = GetWindowThreadProcessId(foreground, None);
            let this_thread = GetCurrentThreadId();
            let attached = foreground_thread != 0
                && foreground_thread != this_thread
                && AttachThreadInput(this_thread, foreground_thread, true).as_bool();
            let _ = BringWindowToTop(self.window);
            let _ = SetForegroundWindow(self.window);
            if attached {
                let _ = AttachThreadInput(this_thread, foreground_thread, false);
            }
        }
        (self.focus_terminal)();
    }

    fn is_on_other_desktop(&self) -> bool {
        self.desktop_manager
            .as_ref()
            .and_then(|manager| unsafe { manager.IsWindowOnCurrentVirtualDesktop(self.window) }.ok())
            .is_some_and(|on_current| !on_current.as_bool())
    }

    fn move_to_current_desktop(&self) {
        let Some(manager) = &self.desktop_manager else { return };
        unsafe {
            // The foreground window is by definition on the desktop the user is looking at.
            let Ok(desktop) = manager.GetWindowDesktopId(GetForegroundWindow()) else { return };
            let _ = manager.MoveWindowToDesktop(self.window, &desktop);
        }
    }

    fn on_animation_tick(&self) {
        if let Some(started) = self.animation_started.get() {
            self.dispatch(DropDownEvent::AnimationTick(started.elapsed()));
        }
    }

    fn on_focus_poll(&self) {
        let foreground = unsafe { GetForegroundWindow() };
        let active = foreground == self.window;
        let belongs_to_window = !active && belongs_to_this_process(foreground);
        if self.last_focus_state.replace(Some((active, belongs_to_window))) != Some((active, belongs_to_window)) {
            tracing::info!(active, belongs_to_window, "drop-down focus changed");
            self.dispatch(DropDownEvent::FocusChanged { active, belongs_to_window });
        }
    }
}

fn belongs_to_this_process(window: HWND) -> bool {
    if window.0.is_null() {
        return false;
    }
    let mut process = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    process == unsafe { GetCurrentProcessId() }
}

fn cursor_position() -> Point {
    let mut point = POINT::default();
    let _ = unsafe { GetCursorPos(&mut point) };
    Point { x: point.x, y: point.y }
}

fn to_rect(rect: RECT) -> Rect {
    Rect::new(rect.left, rect.top, rect.right, rect.bottom)
}

/// Monitors in Winuake order: primary first, then left to right.
fn enumerate_monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _device: HDC, _area: *mut RECT, data: LPARAM) -> BOOL {
        let monitors = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..MONITORINFO::default() };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            monitors.push(Monitor {
                bounds: to_rect(info.rcMonitor),
                work_area: to_rect(info.rcWork),
                is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        true.into()
    }

    let mut monitors: Vec<Monitor> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM((&raw mut monitors) as isize)) };
    order_monitors(monitors)
}

unsafe extern "system" fn animation_timer_proc(_window: HWND, _message: u32, _timer: usize, _time: u32) {
    with_host(DropDownHost::on_animation_tick);
}

unsafe extern "system" fn focus_timer_proc(_window: HWND, _message: u32, _timer: usize, _time: u32) {
    with_host(DropDownHost::on_focus_poll);
}

/// Called from the message hook for `WM_HOTKEY`.
pub fn handle_hotkey(id: i32) {
    if id == TOGGLE_HOTKEY_ID {
        with_host(DropDownHost::toggle);
    }
}

/// Toggles the drop-down from anywhere on the UI thread (tray, second instance).
pub fn toggle() {
    with_host(DropDownHost::toggle);
}

/// Window-level keyboard shortcuts (resize, full screen, quit) from the terminal view.
pub fn dispatch_shortcut(shortcut: WindowShortcut) {
    with_host(|host| match shortcut {
        WindowShortcut::IncreaseWidth => host.dispatch(DropDownEvent::AdjustWidth(PercentStep::Increase)),
        WindowShortcut::DecreaseWidth => host.dispatch(DropDownEvent::AdjustWidth(PercentStep::Decrease)),
        WindowShortcut::IncreaseHeight => host.dispatch(DropDownEvent::AdjustHeight(PercentStep::Increase)),
        WindowShortcut::DecreaseHeight => host.dispatch(DropDownEvent::AdjustHeight(PercentStep::Decrease)),
        WindowShortcut::ToggleFullscreen => host.dispatch(DropDownEvent::ToggleFullscreen),
        WindowShortcut::Quit => crate::application::quit(),
    });
}
