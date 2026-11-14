//! One terminal on screen: a focusable XAML control hosting a `SwapChainPanel`, with the renderer, the shell session, keyboard
//! input, resizing, DPI scaling, cursor blink, and output-driven redraws wired together.
//!
//! WinUI event handlers must be `Send + 'static`, so they capture only a `TerminalId` and look the view up in a UI-thread registry.

use crate::bindings::{
    ContentControl, DispatcherQueue, DispatcherQueueHandler, FocusState, HorizontalAlignment, SwapChainPanel,
    VerticalAlignment,
};
mod pointer;

use crate::swap_chain_panel_native::ISwapChainPanelNative;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::Duration;
use windows_core::{Interface, Result};
use winuake_render::color::ColorScheme;
use winuake_render::gpu::{FontSpec, TerminalRenderer};
use winuake_term::input::{Key, Modifiers, MouseButton, encode_key};
use winuake_term::keyboard::{self, CharacterAssembler, KeyDownAction, decide_key_down, text_for_character};
use winuake_term::pointer::{ClickCounter, WheelAccumulator};
use winuake_term::pty::TerminalSize;
use winuake_term::session::{SessionEvent, TerminalId, TerminalSession};

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// High bit of `GetKeyState`: the key is currently down.
const KEY_DOWN_BIT: i16 = i16::MIN;
const INITIAL_WIDTH_PX: u32 = 800;
const INITIAL_HEIGHT_PX: u32 = 480;

thread_local! {
    /// The view that received the most recent key press; text messages that follow belong to it.
    static KEY_TARGET: Cell<Option<TerminalId>> = const { Cell::new(None) };
    static VIEWS: RefCell<HashMap<TerminalId, Rc<TerminalView>>> = RefCell::new(HashMap::new());
}

/// Stops `WM_CHAR` text from reaching a terminal, while a text box (tab rename) has the keyboard.
pub fn suspend_text_routing() {
    KEY_TARGET.with(|target| target.set(None));
}

/// The live view for a terminal, if it still exists.
pub fn find(id: TerminalId) -> Option<Rc<TerminalView>> {
    VIEWS.with(|views| views.borrow().get(&id).cloned())
}

/// Forgets a terminal's view; once nothing else holds it the shell is killed.
pub fn discard(id: TerminalId) {
    VIEWS.with(|views| views.borrow_mut().remove(&id));
}

fn with_view(id: TerminalId, action: impl FnOnce(&TerminalView)) {
    let view = VIEWS.with(|views| views.borrow().get(&id).cloned());
    if let Some(view) = view {
        action(&view);
    }
}

struct RenderState {
    renderer: TerminalRenderer,
    scale: f32,
}

pub struct TerminalView {
    id: TerminalId,
    control: ContentControl,
    panel: SwapChainPanel,
    session: Arc<TerminalSession>,
    render: RefCell<RenderState>,
    scheme: RefCell<ColorScheme>,
    /// Profile setting: when off the cursor stays solid.
    cursor_blink: Arc<AtomicBool>,
    /// A focus request has not succeeded yet (the view was not in the tree).
    focus_wanted: Cell<bool>,
    characters: RefCell<CharacterAssembler>,
    /// Win32 handle of the application window; owns clipboard operations.
    window_handle: isize,
    selecting: Cell<bool>,
    /// "Allow Mouse Tracking" in the context menu: when off, programs that ask for mouse events get none and the mouse selects text.
    mouse_tracking_allowed: Cell<bool>,
    /// The button an application (mouse reporting) currently sees as held.
    pressed_button: Cell<Option<MouseButton>>,
    clicks: RefCell<ClickCounter>,
    wheel: RefCell<WheelAccumulator>,
    redraw_pending: Arc<AtomicBool>,
    cursor_visible: Arc<AtomicBool>,
    /// Dropped with the view; background threads watch it to know when to stop.
    alive: Arc<()>,
}

impl TerminalView {
    /// Builds the view, registers it on the UI thread, and starts pumping session events into redraws.
    pub fn create(
        session: TerminalSession,
        font: &FontSpec,
        scheme: ColorScheme,
        cursor_blink: bool,
        window_handle: isize,
    ) -> Result<Rc<Self>> {
        let id = session.id();
        let session = Arc::new(session);

        let panel = SwapChainPanel::new()?;
        panel.SetHorizontalAlignment(HorizontalAlignment::Stretch)?;
        panel.SetVerticalAlignment(VerticalAlignment::Stretch)?;
        let control = ContentControl::new()?;
        control.SetContent(&panel)?;
        control.SetHorizontalContentAlignment(HorizontalAlignment::Stretch)?;
        control.SetVerticalContentAlignment(VerticalAlignment::Stretch)?;
        control.SetIsTabStop(true)?;
        control.SetUseSystemFocusVisuals(false)?;

        let renderer = TerminalRenderer::new(font, 96.0, INITIAL_WIDTH_PX, INITIAL_HEIGHT_PX)?;
        panel.cast::<ISwapChainPanelNative>()?.attach(renderer.swap_chain())?;

        let view = Rc::new(Self {
            id,
            control,
            panel,
            session,
            render: RefCell::new(RenderState { renderer, scale: 1.0 }),
            scheme: RefCell::new(scheme),
            cursor_blink: Arc::new(AtomicBool::new(cursor_blink)),
            focus_wanted: Cell::new(false),
            characters: RefCell::new(CharacterAssembler::default()),
            window_handle,
            selecting: Cell::new(false),
            mouse_tracking_allowed: Cell::new(true),
            pressed_button: Cell::new(None),
            clicks: RefCell::new(ClickCounter::default()),
            wheel: RefCell::new(WheelAccumulator::default()),
            redraw_pending: Arc::new(AtomicBool::new(false)),
            cursor_visible: Arc::new(AtomicBool::new(true)),
            alive: Arc::new(()),
        });

        view.connect_xaml_events()?;
        VIEWS.with(|views| views.borrow_mut().insert(id, Rc::clone(&view)));
        view.start_background_threads()?;
        view.apply_size();
        Ok(view)
    }

    pub fn mouse_tracking_allowed(&self) -> bool {
        self.mouse_tracking_allowed.get()
    }

    pub fn set_mouse_tracking_allowed(&self, allowed: bool) {
        self.mouse_tracking_allowed.set(allowed);
    }

    /// The folder the shell last reported, if it ever did.
    pub fn current_directory(&self) -> Option<winuake_term::working_directory::ReportedDirectory> {
        self.session.with_emulator(|emulator| emulator.current_directory())
    }

    pub fn has_selection(&self) -> bool {
        self.session.with_emulator(|emulator| emulator.has_selection())
    }

    /// Switch Profile: the font, colors, cursor and scrollback of another profile, applied to this running terminal. The shell and what is on
    /// screen stay; the grid is refitted because the cell size may change.
    pub fn apply_profile(
        &self,
        font: &FontSpec,
        scheme: ColorScheme,
        cursor_blink: bool,
        cursor: winuake_term::snapshot::CursorStyle,
        scrollback_lines: usize,
    ) {
        let scale = self.render.borrow().scale;
        if let Err(error) = self.render.borrow_mut().renderer.set_font(font, 96.0 * scale) {
            tracing::error!(terminal_id = %self.id, %error, "could not switch the terminal font");
        }
        *self.scheme.borrow_mut() = scheme;
        self.cursor_blink.store(cursor_blink, Ordering::Relaxed);
        self.session.update_emulator(|emulator| emulator.apply_defaults(scrollback_lines, cursor));
        self.apply_size();
    }

    /// Find: selects the next (or previous) match of `query` and scrolls to it.
    pub fn find(
        &self,
        query: &winuake_term::emulator::SearchQuery,
        step: winuake_term::emulator::SearchStep,
    ) -> winuake_term::emulator::SearchOutcome {
        tracing::debug!(terminal_id = %self.id, length = query.text.len(), ?step, "searching the terminal");
        let outcome = self.session.update_emulator(|emulator| emulator.find(query, step));
        self.redraw();
        outcome
    }

    /// Drops the scrollback lines of this terminal.
    pub fn clear_scrollback(&self) {
        tracing::info!(terminal_id = %self.id, "clearing scrollback");
        self.session.update_emulator(|emulator| emulator.clear_scrollback());
        self.redraw();
    }

    /// Clears screen and scrollback and resets the terminal's modes.
    pub fn clear_scrollback_and_reset(&self) {
        tracing::info!(terminal_id = %self.id, "clearing scrollback and resetting the terminal");
        self.session.update_emulator(|emulator| {
            emulator.reset_and_clear();
        });
        self.redraw();
    }

    /// Translucency: draws the default background in another color and opacity, then redraws.
    pub fn set_background(&self, style: winuake_render::gpu::BackgroundStyle) {
        self.render.borrow_mut().renderer.set_background(style);
        self.redraw();
    }

    /// Types `text` into the shell as if pasted (scripting). Only the length is logged.
    pub fn type_text(&self, text: &str) -> bool {
        tracing::info!(terminal_id = %self.id, length = text.len(), "typing scripted text");
        self.session.send_input(text.as_bytes()).is_ok()
    }

    pub fn set_monitoring(&self, activity: bool, silence: bool) {
        self.session.set_monitoring_activity(activity);
        self.session.set_monitoring_silence(silence);
    }

    /// The user looked at this terminal: the next output counts as new activity.
    pub fn acknowledge_monitor(&self) {
        self.session.acknowledge_monitor();
    }

    pub fn element(&self) -> &ContentControl {
        &self.control
    }

    /// Focuses the control now and again once XAML has laid it out: a view added a moment ago refuses focus until it is in the tree.
    pub fn focus(&self) {
        self.focus_wanted.set(true);
        self.focus_now();
        let id = self.id;
        if let Ok(dispatcher) = DispatcherQueue::GetForCurrentThread() {
            let handler = DispatcherQueueHandler::new(move || {
                with_view(id, TerminalView::focus_now);
                Ok(())
            });
            let _ = dispatcher.TryEnqueue(&handler);
        }
    }

    /// Tries to give the control focus; a request that fails stays pending and is retried when the view gets its first real size.
    fn focus_now(&self) {
        match self.control.Focus(FocusState::Programmatic) {
            Ok(true) => self.focus_wanted.set(false),
            Ok(false) => {}
            Err(error) => tracing::warn!(terminal_id = %self.id, %error, "could not focus terminal"),
        }
    }

    fn connect_xaml_events(&self) -> Result<()> {
        let id = self.id;
        // The handlers live as long as the elements; revokers are deliberately forgotten.
        std::mem::forget(self.panel.SizeChanged(move |_, _| with_view(id, TerminalView::apply_size))?);
        std::mem::forget(self.panel.CompositionScaleChanged(move |_, _| with_view(id, TerminalView::apply_size))?);
        std::mem::forget(self.control.KeyDown(move |_, args| {
            if let Some(args) = args.as_ref() {
                with_view(id, |view| view.handle_key_down(args));
            }
        })?);
        std::mem::forget(self.control.GotFocus(move |_, _| crate::workspace_host::terminal_activated(id))?);
        // Text from the message queue belongs to the focused terminal only; once focus leaves, other windows' text boxes get their own.
        std::mem::forget(self.control.LostFocus(move |_, _| {
            KEY_TARGET.with(|target| {
                if target.get() == Some(id) {
                    target.set(None);
                }
            });
        })?);
        std::mem::forget(self.control.PointerPressed(move |_, args| {
            if let Some(args) = args.as_ref() {
                with_view(id, |view| view.handle_pointer_pressed(args));
            }
        })?);
        std::mem::forget(self.control.PointerMoved(move |_, args| {
            crate::workspace_host::pointer_over_terminal(id);
            if let Some(args) = args.as_ref() {
                with_view(id, |view| view.handle_pointer_moved(args));
            }
        })?);
        std::mem::forget(self.control.PointerReleased(move |_, args| {
            if let Some(args) = args.as_ref() {
                with_view(id, |view| view.handle_pointer_released(args));
            }
        })?);
        std::mem::forget(self.control.PointerWheelChanged(move |_, args| {
            if let Some(args) = args.as_ref() {
                with_view(id, |view| view.handle_pointer_wheel(args));
            }
        })?);
        Ok(())
    }

    fn start_background_threads(&self) -> Result<()> {
        let dispatcher = DispatcherQueue::GetForCurrentThread()?;
        spawn_event_pump(
            self.id,
            Arc::downgrade(&self.session),
            dispatcher.clone(),
            Arc::clone(&self.redraw_pending),
            Arc::downgrade(&self.alive),
        );
        // Always running: switching to a profile that blinks (or stops blinking) only flips the flag it watches.
        spawn_cursor_blinker(
            self.id,
            dispatcher,
            Arc::clone(&self.cursor_blink),
            Arc::clone(&self.cursor_visible),
            Arc::clone(&self.redraw_pending),
            Arc::downgrade(&self.alive),
        );
        Ok(())
    }

    /// Fits the swap chain, fonts and shell to the panel's current size and DPI, then redraws.
    fn apply_size(&self) {
        if let Err(error) = self.try_apply_size() {
            tracing::error!(terminal_id = %self.id, %error, "failed to resize terminal view");
        }
    }

    fn try_apply_size(&self) -> Result<()> {
        let scale = self.panel.CompositionScaleX()?.max(0.5);
        let width = (self.panel.ActualWidth()? as f32 * scale).round() as u32;
        let height = (self.panel.ActualHeight()? as f32 * scale).round() as u32;
        if width == 0 || height == 0 {
            return Ok(());
        }
        if self.focus_wanted.get() {
            self.focus_now();
        }

        let grid = {
            let mut state = self.render.borrow_mut();
            if (state.scale - scale).abs() > f32::EPSILON {
                state.renderer.set_dpi(96.0 * scale)?;
                state.scale = scale;
            }
            state.renderer.set_composition_scale(scale)?;
            state.renderer.resize(width, height)?;
            state.renderer.grid_size_for(width, height)
        };
        tracing::debug!(terminal_id = %self.id, width, height, columns = grid.columns, rows = grid.rows, scale, "terminal view resized");
        if let Err(error) = self.session.resize(TerminalSize::new(grid.columns, grid.rows)) {
            tracing::warn!(terminal_id = %self.id, %error, "could not resize shell");
        }
        self.redraw();
        Ok(())
    }

    fn redraw(&self) {
        let snapshot = self.session.with_emulator(|emulator| emulator.snapshot());
        let cursor_visible = self.cursor_visible.load(Ordering::Relaxed);
        if let Err(error) = self.render.borrow_mut().renderer.render(&snapshot, &self.scheme.borrow(), cursor_visible) {
            tracing::error!(terminal_id = %self.id, %error, "failed to draw terminal");
        }
    }

    fn handle_key_down(&self, args: &crate::bindings::KeyRoutedEventArgs) {
        KEY_TARGET.with(|target| target.set(Some(self.id)));
        let Ok(virtual_key) = args.Key() else { return };
        let modifiers = current_modifiers();
        if crate::workspace_host::handle_shortcut(virtual_key.0, modifiers) {
            let _ = args.SetHandled(true);
            return;
        }
        if !crate::workspace_host::keyboard_input_enabled(self.id) {
            // Input is locked for this terminal: swallow typing but leave the window shortcuts above working.
            if !modifiers.control && !modifiers.alt && !modifiers.shift {
                crate::workspace_host::indicate_blocked_input(self.id);
            }
            let _ = args.SetHandled(true);
            return;
        }
        match decide_key_down(virtual_key.0, modifiers) {
            KeyDownAction::Send(key) => {
                self.send_key(key, modifiers);
                let _ = args.SetHandled(true);
            }
            KeyDownAction::Copy => {
                self.copy_selection();
                let _ = args.SetHandled(true);
            }
            KeyDownAction::Paste => {
                self.paste_clipboard();
                let _ = args.SetHandled(true);
            }
            KeyDownAction::Ignore | KeyDownAction::AwaitCharacter => {}
        }
    }

    /// A UTF-16 unit from the message queue (`WM_CHAR` / `WM_SYSCHAR`): the text the keyboard layout and IME produced.
    fn handle_character_unit(&self, unit: u16) {
        let Some(character) = self.characters.borrow_mut().push(unit) else { return };
        if !crate::workspace_host::keyboard_input_enabled(self.id) {
            return;
        }
        if let Some((key, modifiers)) = text_for_character(character, current_modifiers()) {
            self.send_key(key, modifiers);
        }
    }

    fn send_key(&self, key: Key, modifiers: Modifiers) {
        let modes = self.session.with_emulator(|emulator| emulator.input_modes());
        let Some(bytes) = encode_key(key, modifiers, &modes) else { return };
        // Typing returns to the live screen, drops any selection, and makes the cursor solid.
        self.session.update_emulator(|emulator| {
            emulator.clear_selection();
            emulator.scroll_view(winuake_term::emulator::ScrollRequest::Bottom);
        });
        self.cursor_visible.store(true, Ordering::Relaxed);
        if let Err(error) = self.session.send_input(&bytes) {
            tracing::warn!(terminal_id = %self.id, %error, "could not send input to shell");
        }
        self.redraw();
    }
}

impl Drop for TerminalView {
    fn drop(&mut self) {
        tracing::debug!(terminal_id = %self.id, "terminal view dropped");
    }
}

windows_core::link!("user32.dll" "system" fn GetKeyState(virtual_key: i32) -> i16);

pub(crate) fn current_modifiers() -> Modifiers {
    let is_down = |virtual_key: i32| unsafe { GetKeyState(virtual_key) } & KEY_DOWN_BIT != 0;
    Modifiers {
        shift: is_down(keyboard::VK_SHIFT),
        control: is_down(keyboard::VK_CONTROL),
        alt: is_down(keyboard::VK_MENU),
    }
}

/// Asks the UI thread to redraw, at most once until it has done so.
fn request_redraw(id: TerminalId, dispatcher: &DispatcherQueue, pending: &AtomicBool) {
    if pending.swap(true, Ordering::AcqRel) {
        return;
    }
    let handler = DispatcherQueueHandler::new(move || {
        with_view(id, |view| {
            view.redraw_pending.store(false, Ordering::Release);
            view.redraw();
        });
        Ok(())
    });
    if !dispatcher.TryEnqueue(&handler).unwrap_or(false) {
        pending.store(false, Ordering::Release);
    }
}

/// Runs `action` on the UI thread. The pump threads must never touch XAML or the workspace directly.
fn run_on_ui_thread(dispatcher: &DispatcherQueue, action: impl FnOnce() + Send + 'static) {
    let action = std::sync::Mutex::new(Some(action));
    let handler = DispatcherQueueHandler::new(move || {
        if let Some(action) = action.lock().ok().and_then(|mut slot| slot.take()) {
            action();
        }
        Ok(())
    });
    if !dispatcher.TryEnqueue(&handler).unwrap_or(false) {
        tracing::warn!("could not queue work on the UI thread");
    }
}

fn spawn_event_pump(
    id: TerminalId,
    session: Weak<TerminalSession>,
    dispatcher: DispatcherQueue,
    pending: Arc<AtomicBool>,
    alive: Weak<()>,
) {
    let spawned = thread::Builder::new().name(format!("terminal-{id}-events")).spawn(move || {
        while alive.upgrade().is_some() {
            let Some(session) = session.upgrade() else { break };
            if let Some(notification) = session.poll_monitor_notification(std::time::Instant::now()) {
                let kind = match notification {
                    winuake_term::activity::MonitorNotification::ActivityDetected => {
                        winuake_core::notifications::MonitorKind::Activity
                    }
                    winuake_term::activity::MonitorNotification::SilenceDetected => {
                        winuake_core::notifications::MonitorKind::Silence
                    }
                };
                run_on_ui_thread(&dispatcher, move || crate::workspace_host::terminal_monitor_event(id, kind));
            }
            match session.next_event_timeout(EVENT_POLL_INTERVAL) {
                Some(SessionEvent::Exited(status)) => {
                    tracing::info!(terminal_id = %id, exit_code = ?status.code, "shell exited");
                    run_on_ui_thread(&dispatcher, move || crate::workspace_host::terminal_exited(id));
                    break;
                }
                Some(SessionEvent::TitleChanged(title)) => {
                    tracing::debug!(terminal_id = %id, length = title.len(), "terminal title changed");
                    run_on_ui_thread(&dispatcher, move || crate::workspace_host::terminal_title_changed(id, &title));
                    request_redraw(id, &dispatcher, &pending);
                }
                Some(SessionEvent::TitleReset) => {
                    run_on_ui_thread(&dispatcher, move || crate::workspace_host::terminal_title_changed(id, ""));
                }
                Some(_) => request_redraw(id, &dispatcher, &pending),
                None => {}
            }
        }
    });
    if let Err(error) = spawned {
        tracing::error!(terminal_id = %id, %error, "could not start terminal event thread");
    }
}

fn spawn_cursor_blinker(
    id: TerminalId,
    dispatcher: DispatcherQueue,
    blink_enabled: Arc<AtomicBool>,
    cursor_visible: Arc<AtomicBool>,
    pending: Arc<AtomicBool>,
    alive: Weak<()>,
) {
    let spawned = thread::Builder::new().name(format!("terminal-{id}-blink")).spawn(move || {
        while alive.upgrade().is_some() {
            thread::sleep(CURSOR_BLINK_INTERVAL);
            if blink_enabled.load(Ordering::Relaxed) {
                cursor_visible.fetch_xor(true, Ordering::Relaxed);
                request_redraw(id, &dispatcher, &pending);
            } else if !cursor_visible.swap(true, Ordering::Relaxed) {
                // Blinking was switched off while the cursor was hidden: show it again.
                request_redraw(id, &dispatcher, &pending);
            }
        }
    });
    if let Err(error) = spawned {
        tracing::error!(terminal_id = %id, %error, "could not start cursor blink thread");
    }
}

/// Routes a UTF-16 unit from `WM_CHAR` / `WM_SYSCHAR` to the view that received the latest key press.
pub fn handle_text_message(unit: u16) {
    if let Some(id) = KEY_TARGET.with(Cell::get) {
        with_view(id, |view| view.handle_character_unit(unit));
    }
}
