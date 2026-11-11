//! Carries out what `winuake_core::sessions::workspace::Workspace` decides: starts and drops shells and terminal views, lays split panes out,
//! owns the dividers, runs every action, and keeps the chrome (title bar, tab bar) in step.
//!
//! The workspace is pure state. Each operation returns effects; this module applies them. The workspace is never borrowed while effects run,
//! because applying one (for example focusing a terminal) can call straight back in.

use crate::bindings::{
    ContentControl, FrameworkElement, Grid, HorizontalAlignment, PointerRoutedEventArgs, Thickness, UIElement,
    VerticalAlignment, XamlReader,
};
use crate::chrome::Chrome;
use crate::terminal_view::{self, TerminalView};
use crate::window_bindings::{IDOK, MB_DEFBUTTON2, MB_ICONWARNING, MB_OKCANCEL, MessageBoxW};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use windows_core::{HSTRING, Interface, Result};
use winuake_config::profile::{CursorShapeSetting, Profile};
use winuake_config::settings::Settings;
use winuake_core::actions::{Action, ShortcutMap};
use winuake_core::drop_down::{DropDownEvent, PercentStep};
use winuake_core::ids::{SessionId, TerminalId};
use winuake_core::sessions::session::{SessionKind, TerminalSwitch};
use winuake_core::sessions::split_tree::{GrowthDirection, PaneRect, SplitPath};
use winuake_core::sessions::workspace::{
    CloseOutcome, CloseQuestion, Confirmation, PaneGeometry, QuitCheck, Workspace, WorkspaceEffect, WorkspaceSettings,
};
use winuake_core::shortcut::{Chord, ChordModifiers};
use winuake_render::color::ColorScheme;
use winuake_render::gpu::FontSpec;
use winuake_render::scheme_catalog::{self, NamedScheme, find as find_scheme};
use winuake_term::conpty::ConPtyBackend;
use winuake_term::input::Modifiers;
use winuake_term::pty::{EnvironmentVariable, PtySpawnRequest, TerminalSize};
use winuake_term::session::{SessionOptions, TerminalSession};
use winuake_term::shell::{PathVariableLocator, ShellResolver};
use winuake_term::snapshot::CursorStyle;

/// Thickness of the draggable divider between split panes, in DIPs.
const DIVIDER_THICKNESS: i32 = 4;
/// A pane never shrinks below this when a divider moves or a terminal grows, in DIPs.
const MINIMUM_PANE_EXTENT: i32 = 40;
/// Yakuake's default for `tryGrowTerminal*`.
const GROW_STEP: i32 = 10;
const DIVIDER_MARKUP: &str =
    "<Rectangle xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation' Fill='#FF3A3A3A'/>";

thread_local! {
    static HOST: RefCell<Option<Rc<WorkspaceHost>>> = const { RefCell::new(None) };
    /// The divider being dragged, so only that element reacts to pointer movement.
    static DRAGGED_DIVIDER: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn host() -> Option<Rc<WorkspaceHost>> {
    HOST.with(|slot| slot.borrow().clone())
}

/// The panes of one session and their dividers.
struct SessionView {
    grid: Grid,
    dividers: RefCell<Vec<DividerView>>,
}

struct DividerView {
    path: SplitPath,
    element: FrameworkElement,
}

pub struct WorkspaceHost {
    workspace: RefCell<Workspace>,
    shortcuts: RefCell<ShortcutMap>,
    window_handle: isize,
    content: ContentControl,
    sessions: RefCell<HashMap<SessionId, SessionView>>,
    profile: RefCell<Profile>,
    schemes: RefCell<Vec<NamedScheme>>,
    chrome: Chrome,
}

impl WorkspaceHost {
    /// Creates the host on the UI thread and registers it for event callbacks.
    pub fn install(window_handle: isize, settings: &Settings) -> Result<Rc<Self>> {
        let chrome = Chrome::create()?;
        let content = chrome.content();
        let host = Rc::new(Self {
            workspace: RefCell::new(Workspace::new(workspace_settings(settings))),
            shortcuts: RefCell::new(shortcut_map(settings)),
            window_handle,
            content,
            sessions: RefCell::new(HashMap::new()),
            profile: RefCell::new(settings.active_profile()),
            schemes: RefCell::new(load_schemes()),
            chrome,
        });
        HOST.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&host)));
        std::mem::forget(host.content.SizeChanged(move |_, _| {
            if let Some(host) = self::host() {
                host.relayout_active();
            }
        })?);
        host.chrome.connect(&host)?;
        host.chrome.apply_appearance(settings, &host);
        host.chrome.apply_bar_visibility(&settings.window);
        Ok(host)
    }

    /// Root element of the window content.
    pub fn root(&self) -> UIElement {
        self.chrome.root()
    }

    /// Drops every session (and so every shell). Used on quit.
    pub fn shutdown(&self) {
        let ids: Vec<TerminalId> = self.workspace.borrow().stack().terminal_ids();
        self.sessions.borrow_mut().clear();
        for id in ids {
            terminal_view::discard(id);
        }
        HOST.with(|slot| slot.borrow_mut().take());
    }

    pub fn workspace(&self) -> std::cell::Ref<'_, Workspace> {
        self.workspace.borrow()
    }

    pub fn focus_active_terminal(&self) {
        let active = self.workspace.borrow().stack().active_terminal_id();
        if let Some(view) = active.and_then(terminal_view::find) {
            view.focus();
        }
    }

    pub fn has_sessions(&self) -> bool {
        !self.workspace.borrow().tabs().is_empty()
    }

    /// Runs a workspace operation, then applies its effects with the workspace unborrowed.
    pub fn perform(&self, operation: impl FnOnce(&mut Workspace) -> Vec<WorkspaceEffect>) {
        let effects = operation(&mut self.workspace.borrow_mut());
        self.apply(effects);
    }

    pub fn apply(&self, effects: Vec<WorkspaceEffect>) {
        for effect in effects {
            if let Err(error) = self.apply_one(&effect) {
                tracing::error!(?effect, %error, "could not apply workspace effect");
            }
        }
    }

    fn apply_one(&self, effect: &WorkspaceEffect) -> Result<()> {
        match effect {
            WorkspaceEffect::SessionAdded { session, terminals, .. } => {
                tracing::info!(session_id = %session, terminal_count = terminals.len(), "session added");
                let grid = Grid::new()?;
                grid.SetHorizontalAlignment(HorizontalAlignment::Stretch)?;
                grid.SetVerticalAlignment(VerticalAlignment::Stretch)?;
                self.sessions.borrow_mut().insert(*session, SessionView { grid, dividers: RefCell::new(Vec::new()) });
                for terminal in terminals {
                    self.start_terminal(*session, *terminal)?;
                }
            }
            WorkspaceEffect::TerminalAdded { session, terminal, .. } => {
                tracing::info!(session_id = %session, terminal_id = %terminal, "terminal added");
                self.start_terminal(*session, *terminal)?;
                self.relayout(*session);
            }
            WorkspaceEffect::TerminalRemoved { session, terminal } => {
                tracing::info!(session_id = %session, terminal_id = %terminal, "terminal removed");
                if let Some(view) = terminal_view::find(*terminal)
                    && let Some(session_view) = self.sessions.borrow().get(session)
                {
                    remove_child(&session_view.grid, view.element())?;
                }
                terminal_view::discard(*terminal);
            }
            WorkspaceEffect::SessionRemoved { session } => {
                tracing::info!(session_id = %session, "session removed");
                self.sessions.borrow_mut().remove(session);
            }
            WorkspaceEffect::SessionRaised { session } => {
                if let Some(session_view) = self.sessions.borrow().get(session) {
                    self.content.SetContent(&session_view.grid)?;
                }
                self.relayout(*session);
            }
            WorkspaceEffect::TerminalFocused { terminal } => {
                if let Some(view) = terminal_view::find(*terminal) {
                    view.focus();
                }
            }
            WorkspaceEffect::LayoutChanged { session } => self.relayout(*session),
            WorkspaceEffect::TabsChanged => self.chrome.refresh_tabs(self),
            WorkspaceEffect::ActiveTitleChanged { title } => self.chrome.set_title(title),
            WorkspaceEffect::RetractWindow => crate::drop_down::retract_if_visible(),
        }
        Ok(())
    }

    fn start_terminal(&self, session: SessionId, terminal: TerminalId) -> Result<()> {
        let profile = self.profile.borrow().clone();
        let started = start_shell(terminal, &profile)?;
        let title = started.title;
        let font = FontSpec { family: profile.font_family.clone(), size_points: profile.font_size_points };
        let scheme = self.scheme_for(&profile);
        let view = TerminalView::create(started.session, &font, scheme, profile.cursor_blink, self.window_handle)?;
        let sessions = self.sessions.borrow();
        let Some(session_view) = sessions.get(&session) else { return Ok(()) };
        let element = view.element();
        element.SetHorizontalAlignment(HorizontalAlignment::Left)?;
        element.SetVerticalAlignment(VerticalAlignment::Top)?;
        session_view.grid.Children()?.Append(&element.cast::<UIElement>()?)?;
        drop(sessions);
        let changed = self.workspace.borrow_mut().set_terminal_title(terminal, &title);
        self.apply(changed);
        Ok(())
    }

    fn scheme_for(&self, profile: &Profile) -> ColorScheme {
        let schemes = self.schemes.borrow();
        match find_scheme(&schemes, &profile.color_scheme) {
            Some(found) => found.scheme.clone(),
            None => {
                tracing::warn!(scheme = %profile.color_scheme, "unknown color scheme; using Campbell");
                ColorScheme::campbell()
            }
        }
    }

    /// Pushes each terminal's monitor switches from the workspace state into its session.
    pub fn sync_monitoring(&self) {
        let workspace = self.workspace.borrow();
        for terminal in workspace.stack().terminal_ids() {
            let Some(session) =
                workspace.stack().session_for_terminal(terminal).and_then(|id| workspace.stack().session(id))
            else {
                continue;
            };
            if let Some(view) = terminal_view::find(terminal) {
                view.set_monitoring(
                    session.switch_state(TerminalSwitch::MonitorActivity, terminal),
                    session.switch_state(TerminalSwitch::MonitorSilence, terminal),
                );
            }
        }
    }

    /// Covers a terminal's pane with a translucent rectangle for `duration_ms` (the blocked-input indicator, the activation highlight).
    pub fn flash_terminal(&self, terminal: TerminalId, color: &str, opacity: f64, duration_ms: u32) {
        let area = self.content_area();
        let (session_id, rect) = {
            let workspace = self.workspace.borrow();
            let Some(session_id) = workspace.stack().session_for_terminal(terminal) else { return };
            let Some(session) = workspace.stack().session(session_id) else { return };
            let panes = session.tree().layout(area, DIVIDER_THICKNESS);
            let Some(pane) = panes.into_iter().find(|pane| pane.terminal == terminal) else { return };
            (session_id, pane.rect)
        };
        let sessions = self.sessions.borrow();
        let Some(session_view) = sessions.get(&session_id) else { return };
        let result = (|| -> Result<()> {
            let markup = format!(
                r#"<Rectangle xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Fill="{color}" Opacity="{opacity}" IsHitTestVisible="False" HorizontalAlignment="Left" VerticalAlignment="Top"/>"#
            );
            let overlay: FrameworkElement = XamlReader::Load(&HSTRING::from(markup))?.cast()?;
            place(&overlay, rect);
            session_view.grid.Children()?.Append(&overlay.cast::<UIElement>()?)?;
            crate::flashes::track(session_view.grid.clone(), overlay.cast()?, duration_ms);
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!(%error, "could not flash a terminal");
        }
    }

    // ---- layout ----

    fn content_area(&self) -> PaneRect {
        let width = self.content.ActualWidth().unwrap_or(0.0).floor() as i32;
        let height = self.content.ActualHeight().unwrap_or(0.0).floor() as i32;
        PaneRect::new(0, 0, width, height)
    }

    fn geometry(&self) -> PaneGeometry {
        PaneGeometry { area: self.content_area(), divider: DIVIDER_THICKNESS, minimum_extent: MINIMUM_PANE_EXTENT }
    }

    fn relayout_active(&self) {
        let active = self.workspace.borrow().stack().active_session_id();
        if let Some(session) = active {
            self.relayout(session);
        }
    }

    /// Positions every pane and divider of `session` inside the content area.
    pub fn relayout(&self, session: SessionId) {
        let geometry = self.geometry();
        if geometry.area.width <= 0 || geometry.area.height <= 0 {
            return;
        }
        let (panes, dividers) = {
            let workspace = self.workspace.borrow();
            let Some(target) = workspace.stack().session(session) else { return };
            (
                target.tree().layout(geometry.area, geometry.divider),
                target.tree().dividers(geometry.area, geometry.divider),
            )
        };
        for pane in &panes {
            if let Some(view) = terminal_view::find(pane.terminal) {
                place(view.element(), pane.rect);
            }
        }
        let sessions = self.sessions.borrow();
        let Some(session_view) = sessions.get(&session) else { return };
        let mut existing = session_view.dividers.borrow_mut();
        let unchanged = existing.len() == dividers.len()
            && existing.iter().zip(&dividers).all(|(view, layout)| view.path == layout.path);
        if !unchanged && let Err(error) = rebuild_dividers(session, session_view, &mut existing, &dividers) {
            tracing::error!(session_id = %session, %error, "could not rebuild dividers");
            return;
        }
        for (view, layout) in existing.iter().zip(&dividers) {
            place(&view.element, layout.rect);
        }
    }

    /// A divider drag reached `position` (DIPs, relative to the pane grid): turn it into a split ratio.
    fn drag_divider(&self, session: SessionId, path: &SplitPath, x: f64, y: f64) {
        let geometry = self.geometry();
        let ratio = {
            let workspace = self.workspace.borrow();
            let Some(target) = workspace.stack().session(session) else { return };
            let dividers = target.tree().dividers(geometry.area, geometry.divider);
            let Some(layout) = dividers.iter().find(|candidate| &candidate.path == path) else { return };
            let half = f64::from(DIVIDER_THICKNESS) / 2.0;
            let (offset, origin, extent) = match layout.orientation {
                winuake_core::sessions::split_tree::Orientation::Horizontal => {
                    (x, layout.owner_area.x, layout.owner_area.width)
                }
                winuake_core::sessions::split_tree::Orientation::Vertical => {
                    (y, layout.owner_area.y, layout.owner_area.height)
                }
            };
            let available = f64::from(extent - DIVIDER_THICKNESS);
            if available <= 0.0 {
                return;
            }
            ((offset - f64::from(origin) - half) / available) as f32
        };
        self.perform(|workspace| workspace.set_divider_ratio(session, path, ratio));
    }

    // ---- actions ----

    pub fn run_action(&self, action: Action) {
        tracing::info!(action = %action.name(), "running action");
        match action {
            Action::ViewFullScreen => crate::drop_down::dispatch(DropDownEvent::ToggleFullscreen),
            Action::Quit => request_quit(),
            Action::ToggleWindowState => crate::drop_down::toggle(),
            Action::KeepOpen => self.chrome.toggle_keep_open(),
            Action::ManageProfiles | Action::EditProfile => {
                crate::settings_window::open(Some(winuake_config::fields::Page::Profile));
            }
            Action::IncreaseWindowWidth => {
                crate::drop_down::dispatch(DropDownEvent::AdjustWidth(PercentStep::Increase))
            }
            Action::DecreaseWindowWidth => {
                crate::drop_down::dispatch(DropDownEvent::AdjustWidth(PercentStep::Decrease))
            }
            Action::IncreaseWindowHeight => {
                crate::drop_down::dispatch(DropDownEvent::AdjustHeight(PercentStep::Increase))
            }
            Action::DecreaseWindowHeight => {
                crate::drop_down::dispatch(DropDownEvent::AdjustHeight(PercentStep::Decrease))
            }
            Action::NewSession => self.add_session(SessionKind::Single),
            Action::NewSessionTwoHorizontal => self.add_session(SessionKind::TwoHorizontal),
            Action::NewSessionTwoVertical => self.add_session(SessionKind::TwoVertical),
            Action::NewSessionQuad => self.add_session(SessionKind::Quad),
            Action::CloseSession => self.close_session(None),
            Action::PreviousSession => self.perform(Workspace::previous_session),
            Action::NextSession => self.perform(Workspace::next_session),
            Action::MoveSessionLeft => self.perform(|workspace| workspace.move_session_left(None)),
            Action::MoveSessionRight => self.perform(|workspace| workspace.move_session_right(None)),
            Action::GrowTerminalRight => self.grow(GrowthDirection::Right),
            Action::GrowTerminalLeft => self.grow(GrowthDirection::Left),
            Action::GrowTerminalTop => self.grow(GrowthDirection::Up),
            Action::GrowTerminalBottom => self.grow(GrowthDirection::Down),
            Action::RenameSession => {
                let selected = self.workspace.borrow().tabs().selected();
                if let Some(session) = selected {
                    self.chrome.begin_rename(self, session);
                }
            }
            Action::PreviousTerminal => self.perform(Workspace::focus_previous_terminal),
            Action::NextTerminal => self.perform(Workspace::focus_next_terminal),
            Action::CloseActiveTerminal => self.close_active_terminal(),
            Action::SplitLeftRight => self.perform(Workspace::split_left_right),
            Action::SplitTopBottom => self.perform(Workspace::split_top_bottom),
            Action::SplitAuto => {
                let geometry = self.geometry();
                self.perform(|workspace| workspace.split_auto(geometry));
            }
            Action::ToggleSessionPreventClosing => {
                let mut workspace = self.workspace.borrow_mut();
                let closable = workspace.stack().active_session().is_none_or(|session| session.closable());
                workspace.set_session_closable(None, !closable);
                drop(workspace);
                self.chrome.refresh_tabs(self);
            }
            Action::ToggleSessionKeyboardInput => {
                self.workspace.borrow_mut().toggle_session_switch(None, TerminalSwitch::KeyboardInput);
            }
            Action::ToggleSessionMonitorActivity => {
                self.workspace.borrow_mut().toggle_session_switch(None, TerminalSwitch::MonitorActivity);
                self.sync_monitoring();
            }
            Action::ToggleSessionMonitorSilence => {
                self.workspace.borrow_mut().toggle_session_switch(None, TerminalSwitch::MonitorSilence);
                self.sync_monitoring();
            }
            Action::ToggleTitlebar => self.chrome.toggle_title_bar(),
            Action::SwitchToSession(number) => {
                self.perform(|workspace| workspace.raise_session_at_tab(usize::from(number) - 1));
            }
        }
    }

    pub fn add_session(&self, kind: SessionKind) {
        self.perform(|workspace| workspace.add_session(kind));
    }

    /// Starts a session when the window opens with none (after the last one was closed and the window retracted).
    pub fn ensure_session(&self) {
        if !self.has_sessions() {
            self.add_session(SessionKind::Single);
        }
    }

    fn grow(&self, direction: GrowthDirection) {
        let geometry = self.geometry();
        let grown = self.workspace.borrow_mut().grow_active_terminal(direction, GROW_STEP, geometry);
        tracing::debug!(?direction, grown = ?grown, "grow terminal");
        self.relayout_active();
    }

    /// Closes a session, asking first when it is locked. `None` closes the active one.
    pub fn close_session(&self, session: Option<SessionId>) {
        let outcome = self.workspace.borrow_mut().close_session(session, Confirmation::Ask);
        self.finish_close(outcome, |workspace| workspace.close_session(session, Confirmation::Granted));
    }

    pub fn close_active_terminal(&self) {
        let outcome = self.workspace.borrow_mut().close_active_terminal(None, Confirmation::Ask);
        self.finish_close(outcome, |workspace| workspace.close_active_terminal(None, Confirmation::Granted));
    }

    fn finish_close(&self, outcome: CloseOutcome, confirmed: impl FnOnce(&mut Workspace) -> CloseOutcome) {
        match outcome {
            CloseOutcome::Done(effects) => self.apply(effects),
            CloseOutcome::ConfirmationRequired(question) => {
                let message = match question {
                    CloseQuestion::Session => {
                        "You have locked this session to prevent accidental closing of terminals.\n\nAre you sure you want to close this session?"
                    }
                    CloseQuestion::Terminal => {
                        "You have locked this session to prevent accidental closing of terminals.\n\nAre you sure you want to close this terminal?"
                    }
                };
                if confirm(self.window_handle, message, "Really Close?") {
                    let outcome = confirmed(&mut self.workspace.borrow_mut());
                    if let CloseOutcome::Done(effects) = outcome {
                        self.apply(effects);
                    }
                }
            }
        }
    }

    pub fn check_quit(&self) -> QuitCheck {
        self.workspace.borrow().check_quit()
    }

    pub fn window_handle(&self) -> isize {
        self.window_handle
    }
}

// ---- free functions the views and chrome call (all on the UI thread) ----

fn workspace_settings(settings: &Settings) -> WorkspaceSettings {
    WorkspaceSettings {
        dynamic_tab_titles: settings.window.dynamic_tab_titles,
        keep_open_after_last_session_closes: settings.window.keep_open_after_last_session_closes,
        confirm_quit: settings.dialogs.confirm_quit,
    }
}

fn shortcut_map(settings: &Settings) -> ShortcutMap {
    let outcome = ShortcutMap::with_overrides(&settings.shortcuts);
    for problem in outcome.problems {
        tracing::warn!(%problem, "ignored a shortcut override");
    }
    outcome.map
}

/// New settings arrived: update the workspace behavior, shortcut table and chrome.
pub fn apply_settings(settings: &Settings) {
    let Some(host) = host() else { return };
    let effects = host.workspace.borrow_mut().apply_settings(workspace_settings(settings));
    host.apply(effects);
    *host.shortcuts.borrow_mut() = shortcut_map(settings);
    *host.profile.borrow_mut() = settings.active_profile();
    *host.schemes.borrow_mut() = load_schemes();
    host.chrome.apply_appearance(settings, &host);
    host.chrome.apply_bar_visibility(&settings.window);
}

pub fn run_action(action: Action) {
    if let Some(host) = host() {
        host.run_action(action);
    }
}

pub fn window_handle() -> isize {
    host().map_or(0, |host| host.window_handle())
}

pub use crate::terminal_view::suspend_text_routing;

pub fn close_session(session: SessionId) {
    if let Some(host) = host() {
        host.close_session(Some(session));
    }
}

/// The skin's close button: closes the selected tab.
pub fn close_selected_session() {
    let selected = host().and_then(|host| host.workspace.borrow().tabs().selected());
    if let Some(session) = selected {
        close_session(session);
    }
}

pub fn raise_session(session: SessionId) {
    if let Some(host) = host() {
        host.perform(|workspace| workspace.raise_session(session));
    }
}

pub fn begin_rename(session: SessionId) {
    if let Some(host) = host() {
        host.chrome.begin_rename(&host, session);
    }
}

/// Ends an inline rename. `None` cancels; text commits (empty text returns the tab to dynamic titles).
pub fn finish_rename(session: SessionId, title: Option<&str>) {
    let Some(host) = host() else { return };
    if let Some(title) = title {
        host.perform(|workspace| workspace.rename_session(session, title));
    } else {
        host.chrome.refresh_tabs(&host);
    }
    host.focus_active_terminal();
}

/// Moves a tab by `steps` positions (a drag ended that far away), clamped to the ends.
pub fn drag_session_by(session: SessionId, steps: i64) {
    let Some(host) = host() else { return };
    let index = host.workspace.borrow().tabs().index_of(session);
    let Some(index) = index else { return };
    let last = host.workspace.borrow().tabs().order().len().saturating_sub(1) as i64;
    let target = (index as i64 + steps).clamp(0, last) as usize;
    host.perform(|workspace| workspace.move_session_to(session, target));
}

pub fn move_session(session: SessionId, direction: i32) {
    let Some(host) = host() else { return };
    if direction < 0 {
        host.perform(|workspace| workspace.move_session_left(Some(session)));
    } else {
        host.perform(|workspace| workspace.move_session_right(Some(session)));
    }
}

pub fn toggle_prevent_closing(session: SessionId) {
    let Some(host) = host() else { return };
    {
        let mut workspace = host.workspace.borrow_mut();
        let closable = workspace.stack().session(session).is_none_or(|target| target.closable());
        workspace.set_session_closable(Some(session), !closable);
    }
    host.chrome.refresh_tabs(&host);
}

pub fn toggle_switch(session: SessionId, switch: TerminalSwitch) {
    if let Some(host) = host() {
        host.workspace.borrow_mut().toggle_session_switch(Some(session), switch);
        host.sync_monitoring();
    }
}

/// What the tab context menu needs to draw itself.
pub struct TabMenuState {
    pub can_move_left: bool,
    pub can_move_right: bool,
    pub closable: bool,
    pub keyboard_input_disabled: bool,
    pub monitor_activity: bool,
    pub monitor_silence: bool,
}

pub fn tab_menu_state(session: SessionId) -> Option<TabMenuState> {
    let host = host()?;
    let workspace = host.workspace.borrow();
    let target = workspace.stack().session(session)?;
    let tabs = workspace.tabs();
    Some(TabMenuState {
        can_move_left: tabs.can_move_left(session),
        can_move_right: tabs.can_move_right(session),
        closable: target.closable(),
        keyboard_input_disabled: target.any_disabled(TerminalSwitch::KeyboardInput),
        monitor_activity: target.all_enabled(TerminalSwitch::MonitorActivity),
        monitor_silence: target.all_enabled(TerminalSwitch::MonitorSilence),
    })
}

pub fn focus_active_terminal() {
    if let Some(host) = host() {
        host.focus_active_terminal();
    }
}

/// Opening the window with no sessions starts one.
pub fn ensure_session() {
    if let Some(host) = host() {
        host.ensure_session();
    }
}

/// Drops every shell and view; part of quitting.
pub fn shutdown() {
    if let Some(host) = host() {
        host.shutdown();
    }
}

/// Handles a key-down as a shortcut. True when it was one (the key is consumed).
pub fn handle_shortcut(virtual_key: i32, modifiers: Modifiers) -> bool {
    let Some(host) = host() else { return false };
    let chord_modifiers = ChordModifiers { control: modifiers.control, shift: modifiers.shift, alt: modifiers.alt };
    let Some(chord) = Chord::from_virtual_key(virtual_key, chord_modifiers) else { return false };
    let Some(action) = host.shortcuts.borrow().lookup(chord) else { return false };
    host.run_action(action);
    true
}

pub fn keyboard_input_enabled(terminal: TerminalId) -> bool {
    let Some(host) = host() else { return true };
    let workspace = host.workspace.borrow();
    let Some(session) = workspace.stack().session_for_terminal(terminal).and_then(|id| workspace.stack().session(id))
    else {
        return true;
    };
    session.switch_state(TerminalSwitch::KeyboardInput, terminal)
}

/// A terminal gained keyboard focus.
pub fn terminal_activated(terminal: TerminalId) {
    if let Some(view) = terminal_view::find(terminal) {
        view.acknowledge_monitor();
    }
    if let Some(host) = host() {
        let effects = host.workspace.borrow_mut().activate_terminal(terminal);
        host.apply(effects);
    }
}

/// A monitored terminal got output or went quiet: tell the user unless they are already watching it.
pub fn terminal_monitor_event(terminal: TerminalId, kind: winuake_core::notifications::MonitorKind) {
    let Some(host) = host() else { return };
    let (tab_title, is_active) = {
        let workspace = host.workspace.borrow();
        let Some(session_id) = workspace.stack().session_for_terminal(terminal) else { return };
        let title = workspace.tabs().title(session_id).unwrap_or_default().to_owned();
        let active = workspace.stack().active_session_id() == Some(session_id)
            && workspace.stack().session(session_id).and_then(|session| session.active_terminal()) == Some(terminal);
        (title, active)
    };
    let watching = winuake_core::notifications::user_is_watching(
        crate::drop_down::is_open(),
        crate::drop_down::window_has_focus(),
        is_active,
    );
    tracing::info!(terminal_id = %terminal, ?kind, watching, "monitored terminal event");
    if !watching {
        let text = winuake_core::notifications::notification_text(kind, &tab_title);
        if !crate::tray::notify("Winuake", &text) {
            tracing::warn!("a notification was due but the tray icon is hidden");
        }
    }
}

/// Typing into a locked terminal: flash the blocked-input indicator over it.
pub fn indicate_blocked_input(terminal: TerminalId) {
    let Some(host) = host() else { return };
    let appearance = crate::settings_store::current().appearance;
    host.flash_terminal(
        terminal,
        &appearance.keyboard_input_block_indicator_color,
        appearance.keyboard_input_block_indicator_opacity,
        appearance.keyboard_input_block_indicator_duration_ms,
    );
}

pub fn terminal_title_changed(terminal: TerminalId, title: &str) {
    if let Some(host) = host() {
        let effects = host.workspace.borrow_mut().set_terminal_title(terminal, title);
        host.apply(effects);
    }
}

/// The shell of a terminal ended.
pub fn terminal_exited(terminal: TerminalId) {
    if let Some(host) = host() {
        let effects = host.workspace.borrow_mut().terminal_exited(terminal);
        host.apply(effects);
    }
}

/// Quit from the shortcut, tray menu or title bar: asks first when the workspace says so.
pub fn request_quit() {
    let Some(host) = host() else {
        crate::application::quit();
        return;
    };
    let message = match host.check_quit() {
        QuitCheck::Proceed => None,
        QuitCheck::Confirm { multiple_sessions: true, locked_sessions: true } => Some(
            "There are multiple open sessions, some of which you have locked to prevent closing them accidentally. These will be killed if you continue.",
        ),
        QuitCheck::Confirm { multiple_sessions: true, locked_sessions: false } => {
            Some("There are multiple open sessions. These will be killed if you continue.")
        }
        QuitCheck::Confirm { multiple_sessions: false, locked_sessions: _ } => Some(
            "There are one or more open sessions that you have locked to prevent closing them accidentally. These will be killed if you continue.",
        ),
    };
    if let Some(message) = message {
        let text = format!("{message}\n\nAre you sure you want to quit?");
        if !confirm(host.window_handle(), &text, "Really Quit?") {
            tracing::info!("quit cancelled");
            return;
        }
    }
    crate::application::quit();
}

/// Shows a modal OK/Cancel warning. Cancel is the default button.
fn confirm(owner: isize, text: &str, caption: &str) -> bool {
    use crate::window_bindings::HWND;
    let text = HSTRING::from(text);
    let caption = HSTRING::from(caption);
    let result = unsafe {
        MessageBoxW(
            Some(HWND(owner as *mut core::ffi::c_void)),
            &text,
            &caption,
            (MB_OKCANCEL | MB_ICONWARNING | MB_DEFBUTTON2) as u32,
        )
    };
    result == IDOK
}

// ---- panes and dividers ----

fn place<T: Interface>(element: &T, rect: winuake_core::sessions::split_tree::PaneRect) {
    let result = (|| -> Result<()> {
        let element: FrameworkElement = element.cast()?;
        element.SetWidth(f64::from(rect.width.max(0)))?;
        element.SetHeight(f64::from(rect.height.max(0)))?;
        element.SetMargin(Thickness { Left: f64::from(rect.x), Top: f64::from(rect.y), Right: 0.0, Bottom: 0.0 })
    })();
    if let Err(error) = result {
        tracing::warn!(%error, "could not place element");
    }
}

fn remove_child(grid: &Grid, element: &ContentControl) -> Result<()> {
    let children = grid.Children()?;
    let target = element.cast::<UIElement>()?;
    let mut index = 0;
    if children.IndexOf(&target, &mut index)? {
        children.RemoveAt(index)?;
    }
    Ok(())
}

fn rebuild_dividers(
    session: SessionId,
    session_view: &SessionView,
    existing: &mut Vec<DividerView>,
    layouts: &[winuake_core::sessions::split_tree::DividerLayout],
) -> Result<()> {
    let children = session_view.grid.Children()?;
    for old in existing.drain(..) {
        let mut index = 0;
        if children.IndexOf(&old.element.cast::<UIElement>()?, &mut index)? {
            children.RemoveAt(index)?;
        }
    }
    for layout in layouts {
        let element: FrameworkElement = XamlReader::Load(&HSTRING::from(DIVIDER_MARKUP))?.cast()?;
        element.SetHorizontalAlignment(HorizontalAlignment::Left)?;
        element.SetVerticalAlignment(VerticalAlignment::Top)?;
        connect_divider_drag(&element, session, session_view.grid.clone(), layout.path.clone())?;
        children.Append(&element.cast::<UIElement>()?)?;
        existing.push(DividerView { path: layout.path.clone(), element });
    }
    Ok(())
}

fn connect_divider_drag(element: &FrameworkElement, session: SessionId, grid: Grid, path: SplitPath) -> Result<()> {
    let ui: UIElement = element.cast()?;
    let capture_target = ui.clone();
    std::mem::forget(ui.PointerPressed(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        if let Ok(pointer) = args.Pointer() {
            let _ = capture_target.CapturePointer(&pointer);
        }
        DRAGGED_DIVIDER.with(|flag| flag.set(true));
        let _ = args.SetHandled(true);
    })?);
    let moved_path = path.clone();
    let moved_grid = grid.clone();
    std::mem::forget(ui.PointerMoved(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        if !DRAGGED_DIVIDER.with(Cell::get) {
            return;
        }
        if let Some(position) = pointer_position(args, &moved_grid)
            && let Some(host) = host()
        {
            host.drag_divider(session, &moved_path, position.x, position.y);
        }
    })?);
    let release_target = ui.clone();
    std::mem::forget(ui.PointerReleased(move |_, args| {
        DRAGGED_DIVIDER.with(|flag| flag.set(false));
        if let Some(args) = args.as_ref()
            && let Ok(pointer) = args.Pointer()
        {
            let _ = release_target.ReleasePointerCapture(&pointer);
        }
    })?);
    Ok(())
}

struct PointerPosition {
    x: f64,
    y: f64,
}

fn pointer_position(args: &PointerRoutedEventArgs, relative_to: &Grid) -> Option<PointerPosition> {
    let point = args.GetCurrentPoint(relative_to).ok()?.Position().ok()?;
    Some(PointerPosition { x: f64::from(point.X), y: f64::from(point.Y) })
}

// ---- shells ----

struct StartedShell {
    session: TerminalSession,
    /// Shell program name, used as the terminal's title until the shell sets its own.
    title: String,
}

fn start_shell(id: TerminalId, profile: &Profile) -> Result<StartedShell> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let default_shell =
        ShellResolver::new(PathVariableLocator::from_path_variable(&path), std::env::var("COMSPEC").ok())
            .resolve_default_shell();
    let (program, arguments) = match &profile.program {
        Some(program) => (program.clone(), profile.arguments.clone()),
        None => (default_shell.program, default_shell.arguments),
    };
    let title =
        std::path::Path::new(&program).file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let working_directory = profile
        .working_directory
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(Into::into));
    let request = PtySpawnRequest {
        program,
        arguments,
        working_directory,
        environment: vec![EnvironmentVariable { name: "COLORTERM".to_owned(), value: "truecolor".to_owned() }],
        size: TerminalSize::new(80, 24),
    };
    let options = SessionOptions {
        scrollback_lines: profile.scrollback_lines as usize,
        cursor: match profile.cursor_shape {
            CursorShapeSetting::Block => CursorStyle::Block,
            CursorShapeSetting::Underline => CursorStyle::Underline,
            CursorShapeSetting::Bar => CursorStyle::Beam,
        },
    };
    let session = TerminalSession::start_with_options(id, &ConPtyBackend, &request, &options).map_err(|error| {
        tracing::error!(terminal_id = %id, %error, "could not start the shell");
        windows_core::Error::new(windows_core::HRESULT(0x8000_4005_u32 as i32), error.to_string())
    })?;
    Ok(StartedShell { session, title })
}

/// Built-in color schemes plus any `.json` (Windows Terminal) or `.colorscheme` (Konsole) files in `%APPDATA%\Winuake\schemes`.
fn load_schemes() -> Vec<NamedScheme> {
    let mut imported = Vec::new();
    let directory =
        std::env::var_os("APPDATA").map(std::path::PathBuf::from).map(|base| base.join("Winuake").join("schemes"));
    if let Some(entries) = directory.and_then(|directory| std::fs::read_dir(directory).ok()) {
        for path in entries.flatten().map(|entry| entry.path()) {
            let extension = path.extension().map(|extension| extension.to_string_lossy().to_ascii_lowercase());
            let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let result = match extension.as_deref() {
                Some("json") => scheme_catalog::from_windows_terminal_json(&text),
                Some("colorscheme") => {
                    scheme_catalog::from_konsole_colorscheme(&text, &stem).map(|scheme| vec![scheme])
                }
                _ => continue,
            };
            match result {
                Ok(mut schemes) => imported.append(&mut schemes),
                Err(error) => tracing::warn!(file = %path.display(), %error, "could not import a color scheme"),
            }
        }
    }
    scheme_catalog::catalog(imported)
}
