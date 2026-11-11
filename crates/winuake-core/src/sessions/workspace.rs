// Ported from yakuake app/mainwindow.cpp (session/tab handlers) and app/sessionstack.cpp @21c6210
//! Sessions and tabs together. Every operation changes state and returns the effects the UI must carry out, which stand in
//! for the Qt signals Yakuake connects between `SessionStack`, `TabBar`, `TitleBar` and `MainWindow`.

use super::session::{SessionKind, TerminalSwitch};
use super::session_stack::SessionStack;
use super::split_tree::{GrowthDirection, PaneRect};
use super::tab_list::{TabList, TitleSource};
use crate::ids::{SessionId, TerminalId};

/// Settings that change how the workspace behaves. Phase 5 will load these from the config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceSettings {
    pub dynamic_tab_titles: bool,
    pub keep_open_after_last_session_closes: bool,
    pub confirm_quit: bool,
}

impl Default for WorkspaceSettings {
    /// Defaults of `yakuake.kcfg`.
    fn default() -> Self {
        Self { dynamic_tab_titles: false, keep_open_after_last_session_closes: false, confirm_quit: true }
    }
}

/// What the UI has to do after an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEffect {
    /// Start a shell for each terminal. `working_directory_from` is the terminal whose directory the new ones start in.
    SessionAdded {
        session: SessionId,
        terminals: Vec<TerminalId>,
        working_directory_from: Option<TerminalId>,
    },
    TerminalAdded {
        session: SessionId,
        terminal: TerminalId,
        working_directory_from: TerminalId,
    },
    /// Stop the shell and drop the view.
    TerminalRemoved {
        session: SessionId,
        terminal: TerminalId,
    },
    SessionRemoved {
        session: SessionId,
    },
    /// Show this session's panes.
    SessionRaised {
        session: SessionId,
    },
    /// Give this terminal keyboard focus.
    TerminalFocused {
        terminal: TerminalId,
    },
    /// The pane layout of a session changed.
    LayoutChanged {
        session: SessionId,
    },
    /// Tab order, selection or a title changed; redraw the tab bar.
    TabsChanged,
    /// Title shown in the title bar and window title.
    ActiveTitleChanged {
        title: String,
    },
    /// The last session closed and the window should not stay open.
    RetractWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// Ask the user when the session is locked.
    Ask,
    /// The user already agreed.
    Granted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseQuestion {
    Session,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseOutcome {
    Done(Vec<WorkspaceEffect>),
    /// The session is locked against closing; ask "Really Close?" and call again with `Confirmation::Granted`.
    ConfirmationRequired(CloseQuestion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitCheck {
    Proceed,
    /// Show "Really Quit?". The flags pick the warning text.
    Confirm {
        multiple_sessions: bool,
        locked_sessions: bool,
    },
}

/// Which terminal a scripted split applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTarget {
    /// The active terminal of this session.
    SessionActive(SessionId),
    Terminal(TerminalId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitHow {
    LeftRight,
    TopBottom,
    Auto,
}

/// A split's effects and the id of the new terminal, if one was made.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitOutcome {
    pub effects: Vec<WorkspaceEffect>,
    pub terminal: Option<TerminalId>,
}

/// Pixel geometry of the area holding the panes, needed for decisions that depend on size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneGeometry {
    pub area: PaneRect,
    pub divider: i32,
    pub minimum_extent: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    stack: SessionStack,
    tabs: TabList,
    settings: WorkspaceSettings,
}

impl Workspace {
    pub fn new(settings: WorkspaceSettings) -> Self {
        Self { settings, ..Self::default() }
    }

    pub fn stack(&self) -> &SessionStack {
        &self.stack
    }

    pub fn tabs(&self) -> &TabList {
        &self.tabs
    }

    pub fn settings(&self) -> WorkspaceSettings {
        self.settings
    }

    /// Ported from `MainWindow::applySettings`: turning dynamic titles on pushes the current titles to the tabs.
    pub fn apply_settings(&mut self, settings: WorkspaceSettings) -> Vec<WorkspaceEffect> {
        let enabling = settings.dynamic_tab_titles && !self.settings.dynamic_tab_titles;
        self.settings = settings;
        if !enabling {
            return Vec::new();
        }
        let mut changed = false;
        for id in self.stack.session_ids() {
            let title = self.stack.session(id).map(|session| session.title().to_owned()).unwrap_or_default();
            if !title.is_empty() {
                changed |= self.tabs.set_title(id, &title, TitleSource::Automatic);
            }
        }
        if changed { vec![WorkspaceEffect::TabsChanged] } else { Vec::new() }
    }

    // ---- sessions ----

    /// Adds a session, selects its tab and raises it.
    pub fn add_session(&mut self, kind: SessionKind) -> Vec<WorkspaceEffect> {
        let working_directory_from = self.stack.active_terminal_id();
        let session = self.stack.add_session(kind);
        let terminals = self.stack.session(session).map(|created| created.terminal_ids()).unwrap_or_default();
        self.tabs.add(session, "");
        let mut effects = vec![WorkspaceEffect::SessionAdded { session, terminals, working_directory_from }];
        effects.extend(self.raise_session(session));
        effects
    }

    pub fn raise_session(&mut self, session: SessionId) -> Vec<WorkspaceEffect> {
        if !self.stack.raise_session(session) {
            return Vec::new();
        }
        self.tabs.select(session);
        let mut effects = vec![WorkspaceEffect::SessionRaised { session }, WorkspaceEffect::TabsChanged];
        if let Some(active) = self.stack.session(session) {
            effects.push(WorkspaceEffect::ActiveTitleChanged { title: active.title().to_owned() });
            if let Some(terminal) = active.active_terminal() {
                effects.push(WorkspaceEffect::TerminalFocused { terminal });
            }
        }
        effects
    }

    /// `switch-to-session-N` (zero-based tab position).
    pub fn raise_session_at_tab(&mut self, index: usize) -> Vec<WorkspaceEffect> {
        match self.tabs.session_at(index) {
            Some(session) => self.raise_session(session),
            None => Vec::new(),
        }
    }

    pub fn next_session(&mut self) -> Vec<WorkspaceEffect> {
        match self.tabs.next() {
            Some(session) => self.raise_session(session),
            None => Vec::new(),
        }
    }

    pub fn previous_session(&mut self) -> Vec<WorkspaceEffect> {
        match self.tabs.previous() {
            Some(session) => self.raise_session(session),
            None => Vec::new(),
        }
    }

    pub fn move_session_left(&mut self, session: Option<SessionId>) -> Vec<WorkspaceEffect> {
        self.move_tab(session, |tabs, id| tabs.move_left(id))
    }

    pub fn move_session_right(&mut self, session: Option<SessionId>) -> Vec<WorkspaceEffect> {
        self.move_tab(session, |tabs, id| tabs.move_right(id))
    }

    /// Drag and drop reorder.
    pub fn move_session_to(&mut self, session: SessionId, index: usize) -> Vec<WorkspaceEffect> {
        if self.tabs.move_to(session, index) { vec![WorkspaceEffect::TabsChanged] } else { Vec::new() }
    }

    fn move_tab(
        &mut self,
        session: Option<SessionId>,
        action: impl FnOnce(&mut TabList, SessionId) -> bool,
    ) -> Vec<WorkspaceEffect> {
        let Some(id) = session.or(self.tabs.selected()) else { return Vec::new() };
        if action(&mut self.tabs, id) { vec![WorkspaceEffect::TabsChanged] } else { Vec::new() }
    }

    /// Interactive rename. An empty title unpins the tab so dynamic titles resume.
    pub fn rename_session(&mut self, session: SessionId, title: &str) -> Vec<WorkspaceEffect> {
        let title = title.trim();
        self.tabs.set_title(session, title, TitleSource::Interactive);
        vec![WorkspaceEffect::TabsChanged]
    }

    pub fn close_session(&mut self, session: Option<SessionId>, confirmation: Confirmation) -> CloseOutcome {
        let Some(id) = session.or(self.stack.active_session_id()) else { return CloseOutcome::Done(Vec::new()) };
        let Some(target) = self.stack.session(id) else { return CloseOutcome::Done(Vec::new()) };
        if !target.closable() && confirmation == Confirmation::Ask {
            return CloseOutcome::ConfirmationRequired(CloseQuestion::Session);
        }
        CloseOutcome::Done(self.remove_session(id))
    }

    fn remove_session(&mut self, id: SessionId) -> Vec<WorkspaceEffect> {
        let Some(removed) = self.stack.remove_session(id) else { return Vec::new() };
        let mut effects: Vec<WorkspaceEffect> = removed
            .terminal_ids()
            .into_iter()
            .map(|terminal| WorkspaceEffect::TerminalRemoved { session: id, terminal })
            .collect();
        effects.push(WorkspaceEffect::SessionRemoved { session: id });
        let was_selected = self.tabs.selected() == Some(id);
        match self.tabs.remove(id) {
            Some(next) if was_selected => effects.extend(self.raise_session(next)),
            Some(_) => effects.push(WorkspaceEffect::TabsChanged),
            None => {
                effects.push(WorkspaceEffect::TabsChanged);
                if !self.settings.keep_open_after_last_session_closes {
                    effects.push(WorkspaceEffect::RetractWindow);
                }
            }
        }
        effects
    }

    // ---- terminals ----

    /// Splits the active terminal of the active session.
    pub fn split_left_right(&mut self) -> Vec<WorkspaceEffect> {
        self.split_active(|session, ids| session.split_left_right(None, ids))
    }

    pub fn split_top_bottom(&mut self) -> Vec<WorkspaceEffect> {
        self.split_active(|session, ids| session.split_top_bottom(None, ids))
    }

    pub fn split_auto(&mut self, geometry: PaneGeometry) -> Vec<WorkspaceEffect> {
        self.split_active(|session, ids| session.split_auto(None, geometry.area, geometry.divider, ids))
    }

    fn split_active(
        &mut self,
        split: impl FnOnce(&mut super::session::Session, &mut crate::ids::IdAllocator) -> Option<TerminalId>,
    ) -> Vec<WorkspaceEffect> {
        let Some(session_id) = self.stack.active_session_id() else { return Vec::new() };
        let Some(source) = self.stack.active_terminal_id() else { return Vec::new() };
        let Some((session, ids)) = self.stack.session_with_ids(session_id) else { return Vec::new() };
        let Some(terminal) = split(session, ids) else { return Vec::new() };
        let mut effects = vec![
            WorkspaceEffect::TerminalAdded { session: session_id, terminal, working_directory_from: source },
            WorkspaceEffect::LayoutChanged { session: session_id },
            WorkspaceEffect::TerminalFocused { terminal },
        ];
        effects.extend(self.title_effects(session_id));
        effects
    }

    pub fn focus_next_terminal(&mut self) -> Vec<WorkspaceEffect> {
        self.focus_neighbor(true)
    }

    pub fn focus_previous_terminal(&mut self) -> Vec<WorkspaceEffect> {
        self.focus_neighbor(false)
    }

    fn focus_neighbor(&mut self, forward: bool) -> Vec<WorkspaceEffect> {
        let Some(session_id) = self.stack.active_session_id() else { return Vec::new() };
        let Some(session) = self.stack.session_mut(session_id) else { return Vec::new() };
        let target = if forward { session.focus_next_terminal() } else { session.focus_previous_terminal() };
        let Some(terminal) = target else { return Vec::new() };
        let mut effects = vec![WorkspaceEffect::TerminalFocused { terminal }];
        effects.extend(self.title_effects(session_id));
        effects
    }

    /// The user clicked into a terminal.
    pub fn activate_terminal(&mut self, terminal: TerminalId) -> Vec<WorkspaceEffect> {
        let Some(session_id) = self.stack.session_for_terminal(terminal) else { return Vec::new() };
        if let Some(session) = self.stack.session_mut(session_id) {
            session.set_active_terminal(terminal);
        }
        self.title_effects(session_id)
    }

    /// Closes the active terminal of `session` (default: active). A locked session asks first.
    pub fn close_active_terminal(&mut self, session: Option<SessionId>, confirmation: Confirmation) -> CloseOutcome {
        let Some(id) = session.or(self.stack.active_session_id()) else { return CloseOutcome::Done(Vec::new()) };
        let Some(target) = self.stack.session(id) else { return CloseOutcome::Done(Vec::new()) };
        if !target.closable() && confirmation == Confirmation::Ask {
            return CloseOutcome::ConfirmationRequired(CloseQuestion::Terminal);
        }
        CloseOutcome::Done(self.close_terminal_in(id, None))
    }

    /// The shell of a terminal ended (or the terminal asked to close): no confirmation.
    pub fn terminal_exited(&mut self, terminal: TerminalId) -> Vec<WorkspaceEffect> {
        match self.stack.session_for_terminal(terminal) {
            Some(session) => self.close_terminal_in(session, Some(terminal)),
            None => Vec::new(),
        }
    }

    fn close_terminal_in(&mut self, session_id: SessionId, terminal: Option<TerminalId>) -> Vec<WorkspaceEffect> {
        let Some(session) = self.stack.session_mut(session_id) else { return Vec::new() };
        let Some(closure) = session.close_terminal(terminal) else { return Vec::new() };
        if closure.session_empty {
            // The terminal is already gone from the session, so report it here before the session goes.
            let mut effects =
                vec![WorkspaceEffect::TerminalRemoved { session: session_id, terminal: closure.terminal }];
            effects.extend(self.remove_session(session_id));
            return effects;
        }
        let mut effects = vec![
            WorkspaceEffect::TerminalRemoved { session: session_id, terminal: closure.terminal },
            WorkspaceEffect::LayoutChanged { session: session_id },
        ];
        if let Some(focus) = closure.focus_moved_to {
            effects.push(WorkspaceEffect::TerminalFocused { terminal: focus });
        }
        effects.extend(self.title_effects(session_id));
        effects
    }

    /// `grow-terminal-*`: moves a divider next to the active terminal. Returns the pixels gained, if any.
    pub fn grow_active_terminal(
        &mut self,
        direction: GrowthDirection,
        pixels: i32,
        geometry: PaneGeometry,
    ) -> Option<i32> {
        let session_id = self.stack.active_session_id()?;
        let terminal = self.stack.active_terminal_id()?;
        let session = self.stack.session_mut(session_id)?;
        session.try_grow_terminal(terminal, direction, pixels, geometry.area, geometry.divider, geometry.minimum_extent)
    }

    /// A divider was dragged.
    pub fn set_divider_ratio(
        &mut self,
        session: SessionId,
        path: &super::split_tree::SplitPath,
        ratio: f32,
    ) -> Vec<WorkspaceEffect> {
        let Some(target) = self.stack.session_mut(session) else { return Vec::new() };
        if target.set_divider_ratio(path, ratio) {
            vec![WorkspaceEffect::LayoutChanged { session }]
        } else {
            Vec::new()
        }
    }

    /// A terminal reported a new title (OSC 0/2).
    pub fn set_terminal_title(&mut self, terminal: TerminalId, title: &str) -> Vec<WorkspaceEffect> {
        let Some(session_id) = self.stack.session_for_terminal(terminal) else { return Vec::new() };
        let Some(session) = self.stack.session_mut(session_id) else { return Vec::new() };
        if session.set_terminal_title(terminal, title).is_none() {
            return Vec::new();
        }
        self.title_effects(session_id)
    }

    /// After a session's title may have changed: update the title bar and (dynamic titles) the tab.
    fn title_effects(&mut self, session_id: SessionId) -> Vec<WorkspaceEffect> {
        let Some(session) = self.stack.session(session_id) else { return Vec::new() };
        let title = session.title().to_owned();
        let mut effects = Vec::new();
        if self.settings.dynamic_tab_titles
            && !title.is_empty()
            && self.tabs.set_title(session_id, &title, TitleSource::Automatic)
        {
            effects.push(WorkspaceEffect::TabsChanged);
        }
        if self.stack.active_session_id() == Some(session_id) {
            effects.push(WorkspaceEffect::ActiveTitleChanged { title });
        }
        effects
    }

    // ---- operations addressed by id (scripting) ----

    /// Splits a terminal chosen by session (its active terminal) or by terminal id. `how` picks the direction.
    pub fn split_by_id(&mut self, target: SplitTarget, how: SplitHow, geometry: PaneGeometry) -> SplitOutcome {
        let (session_id, terminal) = match target {
            SplitTarget::SessionActive(session) => {
                (Some(session), self.stack.session(session).and_then(|found| found.active_terminal()))
            }
            SplitTarget::Terminal(terminal) => (self.stack.session_for_terminal(terminal), Some(terminal)),
        };
        let (Some(session_id), Some(terminal)) = (session_id, terminal) else { return SplitOutcome::default() };
        let is_active_session = self.stack.active_session_id() == Some(session_id);
        let Some((session, ids)) = self.stack.session_with_ids(session_id) else { return SplitOutcome::default() };
        let added = match how {
            SplitHow::LeftRight => session.split_left_right(Some(terminal), ids),
            SplitHow::TopBottom => session.split_top_bottom(Some(terminal), ids),
            SplitHow::Auto => session.split_auto(Some(terminal), geometry.area, geometry.divider, ids),
        };
        let Some(added) = added else { return SplitOutcome::default() };
        let mut effects = vec![
            WorkspaceEffect::TerminalAdded { session: session_id, terminal: added, working_directory_from: terminal },
            WorkspaceEffect::LayoutChanged { session: session_id },
        ];
        if is_active_session {
            effects.push(WorkspaceEffect::TerminalFocused { terminal: added });
        }
        effects.extend(self.title_effects(session_id));
        SplitOutcome { effects, terminal: Some(added) }
    }

    /// `removeTerminal`: closes the terminal unless its session is locked.
    pub fn remove_terminal(&mut self, terminal: TerminalId) -> Vec<WorkspaceEffect> {
        let Some(session_id) = self.stack.session_for_terminal(terminal) else { return Vec::new() };
        if !self.stack.session(session_id).is_some_and(|session| session.closable()) {
            return Vec::new();
        }
        self.close_terminal_in(session_id, Some(terminal))
    }

    /// `tryGrowTerminal*` for any terminal. `None` when it cannot grow that way.
    pub fn grow_terminal(
        &mut self,
        terminal: TerminalId,
        direction: GrowthDirection,
        pixels: i32,
        geometry: PaneGeometry,
    ) -> Option<i32> {
        let session_id = self.stack.session_for_terminal(terminal)?;
        let session = self.stack.session_mut(session_id)?;
        session.try_grow_terminal(terminal, direction, pixels, geometry.area, geometry.divider, geometry.minimum_extent)
    }

    /// Sets a switch on one terminal, or on every terminal of the session when `terminal` is `None`.
    pub fn set_switch(
        &mut self,
        session: SessionId,
        terminal: Option<TerminalId>,
        switch: TerminalSwitch,
        enabled: bool,
    ) {
        if let Some(target) = self.stack.session_mut(session) {
            target.set_switch(switch, terminal, enabled);
        }
    }

    /// `setTabTitle`. A script title is interactive by default, which pins it against dynamic titles.
    pub fn set_tab_title(&mut self, session: SessionId, title: &str, interactive: bool) -> Vec<WorkspaceEffect> {
        let source = if interactive { TitleSource::Interactive } else { TitleSource::Automatic };
        self.tabs.set_title(session, title, source);
        vec![WorkspaceEffect::TabsChanged]
    }

    // ---- session switches ----

    pub fn set_session_closable(&mut self, session: Option<SessionId>, closable: bool) {
        let Some(id) = session.or(self.stack.active_session_id()) else { return };
        if let Some(target) = self.stack.session_mut(id) {
            target.set_closable(closable);
        }
    }

    /// Ported from `handleToggleSessionKeyboardInput` and friends: a switch counts as on only when every terminal has it on;
    /// toggling turns all terminals off, or all on if any was off.
    pub fn toggle_session_switch(&mut self, session: Option<SessionId>, switch: TerminalSwitch) -> bool {
        let Some(id) = session.or(self.stack.active_session_id()) else { return false };
        let Some(target) = self.stack.session_mut(id) else { return false };
        let enable = !target.all_enabled(switch);
        target.set_switch(switch, None, enable);
        enable
    }

    // ---- quit ----

    /// Ported from `MainWindow::queryClose`.
    pub fn check_quit(&self) -> QuitCheck {
        let multiple_sessions = self.settings.confirm_quit && self.stack.count() > 1;
        let locked_sessions = self.stack.has_unclosable_sessions();
        if multiple_sessions || locked_sessions {
            QuitCheck::Confirm { multiple_sessions, locked_sessions }
        } else {
            QuitCheck::Proceed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEOMETRY: PaneGeometry =
        PaneGeometry { area: PaneRect::new(0, 0, 1000, 400), divider: 0, minimum_extent: 10 };

    fn workspace_with(sessions: usize) -> Workspace {
        let mut workspace = Workspace::default();
        for _ in 0..sessions {
            workspace.add_session(SessionKind::Single);
        }
        workspace
    }

    fn selected(workspace: &Workspace) -> Option<SessionId> {
        workspace.tabs().selected()
    }

    #[test]
    fn given_an_empty_workspace_when_adding_a_session_then_it_is_raised_and_its_shell_requested() {
        let mut workspace = Workspace::default();
        let effects = workspace.add_session(SessionKind::Single);
        assert_eq!(
            effects[0],
            WorkspaceEffect::SessionAdded {
                session: SessionId(0),
                terminals: vec![TerminalId(0)],
                working_directory_from: None
            }
        );
        assert!(effects.contains(&WorkspaceEffect::SessionRaised { session: SessionId(0) }));
        assert!(effects.contains(&WorkspaceEffect::TerminalFocused { terminal: TerminalId(0) }));
        assert_eq!(selected(&workspace), Some(SessionId(0)));
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("Shell"));
    }

    #[test]
    fn given_an_active_terminal_when_adding_a_session_then_it_starts_in_that_terminals_directory() {
        let mut workspace = workspace_with(1);
        let effects = workspace.add_session(SessionKind::Single);
        assert_eq!(
            effects[0],
            WorkspaceEffect::SessionAdded {
                session: SessionId(1),
                terminals: vec![TerminalId(1)],
                working_directory_from: Some(TerminalId(0)),
            }
        );
    }

    #[test]
    fn given_a_quad_session_when_added_then_four_terminals_are_requested() {
        let mut workspace = Workspace::default();
        let effects = workspace.add_session(SessionKind::Quad);
        let WorkspaceEffect::SessionAdded { terminals, .. } = &effects[0] else { panic!("expected SessionAdded") };
        assert_eq!(terminals.len(), 4);
    }

    #[test]
    fn given_three_sessions_when_switching_by_tab_position_then_that_session_is_raised() {
        let mut workspace = workspace_with(3);
        workspace.raise_session_at_tab(0);
        assert_eq!(selected(&workspace), Some(SessionId(0)));
        assert!(workspace.raise_session_at_tab(7).is_empty());
        assert_eq!(selected(&workspace), Some(SessionId(0)));
    }

    #[test]
    fn given_the_last_tab_when_going_next_then_the_first_is_raised() {
        let mut workspace = workspace_with(3);
        workspace.next_session();
        assert_eq!(selected(&workspace), Some(SessionId(0)));
        workspace.previous_session();
        assert_eq!(selected(&workspace), Some(SessionId(2)));
        assert_eq!(workspace.stack().active_session_id(), Some(SessionId(2)));
    }

    #[test]
    fn given_moved_tabs_when_switching_by_position_then_tab_order_not_creation_order_counts() {
        let mut workspace = workspace_with(3);
        workspace.move_session_left(None);
        workspace.move_session_left(None);
        workspace.raise_session_at_tab(0);
        assert_eq!(selected(&workspace), Some(SessionId(2)));
        assert!(workspace.move_session_left(None).is_empty());
        workspace.move_session_right(Some(SessionId(2)));
        assert_eq!(workspace.tabs().order()[1], SessionId(2));
        assert_eq!(workspace.move_session_to(SessionId(2), 0), vec![WorkspaceEffect::TabsChanged]);
    }

    #[test]
    fn given_two_sessions_when_the_selected_one_closes_then_the_last_tab_is_raised() {
        let mut workspace = workspace_with(3);
        workspace.raise_session(SessionId(0));
        let CloseOutcome::Done(effects) = workspace.close_session(None, Confirmation::Ask) else { panic!("closed") };
        assert!(effects.contains(&WorkspaceEffect::TerminalRemoved { session: SessionId(0), terminal: TerminalId(0) }));
        assert!(effects.contains(&WorkspaceEffect::SessionRemoved { session: SessionId(0) }));
        assert!(effects.contains(&WorkspaceEffect::SessionRaised { session: SessionId(2) }));
        assert_eq!(workspace.stack().count(), 2);
    }

    #[test]
    fn given_a_background_session_when_it_closes_then_the_selection_stays() {
        let mut workspace = workspace_with(3);
        let CloseOutcome::Done(effects) = workspace.close_session(Some(SessionId(0)), Confirmation::Ask) else {
            panic!("closed")
        };
        assert_eq!(selected(&workspace), Some(SessionId(2)));
        assert!(!effects.iter().any(|effect| matches!(effect, WorkspaceEffect::SessionRaised { .. })));
        assert!(effects.contains(&WorkspaceEffect::TabsChanged));
    }

    #[test]
    fn given_the_last_session_when_it_closes_then_the_window_retracts() {
        let mut workspace = workspace_with(1);
        let CloseOutcome::Done(effects) = workspace.close_session(None, Confirmation::Ask) else { panic!("closed") };
        assert!(effects.contains(&WorkspaceEffect::RetractWindow));
        assert!(workspace.tabs().is_empty());
    }

    #[test]
    fn given_keep_open_after_last_session_when_the_last_closes_then_the_window_stays() {
        let mut workspace =
            Workspace::new(WorkspaceSettings { keep_open_after_last_session_closes: true, ..Default::default() });
        workspace.add_session(SessionKind::Single);
        let CloseOutcome::Done(effects) = workspace.close_session(None, Confirmation::Ask) else { panic!("closed") };
        assert!(!effects.contains(&WorkspaceEffect::RetractWindow));
    }

    #[test]
    fn given_a_locked_session_when_closing_then_confirmation_is_required_until_granted() {
        let mut workspace = workspace_with(2);
        workspace.set_session_closable(None, false);
        assert_eq!(
            workspace.close_session(None, Confirmation::Ask),
            CloseOutcome::ConfirmationRequired(CloseQuestion::Session)
        );
        assert_eq!(workspace.stack().count(), 2);
        assert!(matches!(workspace.close_session(None, Confirmation::Granted), CloseOutcome::Done(_)));
        assert_eq!(workspace.stack().count(), 1);
    }

    #[test]
    fn given_no_such_session_when_closing_then_nothing_happens() {
        let mut workspace = workspace_with(1);
        assert_eq!(workspace.close_session(Some(SessionId(9)), Confirmation::Ask), CloseOutcome::Done(Vec::new()));
        assert_eq!(Workspace::default().close_session(None, Confirmation::Ask), CloseOutcome::Done(Vec::new()));
        assert_eq!(Workspace::default().close_active_terminal(None, Confirmation::Ask), CloseOutcome::Done(Vec::new()));
    }

    #[test]
    fn given_one_terminal_when_splitting_left_right_then_a_second_one_starts_and_takes_focus() {
        let mut workspace = workspace_with(1);
        let effects = workspace.split_left_right();
        assert!(effects.contains(&WorkspaceEffect::TerminalAdded {
            session: SessionId(0),
            terminal: TerminalId(1),
            working_directory_from: TerminalId(0),
        }));
        assert!(effects.contains(&WorkspaceEffect::LayoutChanged { session: SessionId(0) }));
        assert!(effects.contains(&WorkspaceEffect::TerminalFocused { terminal: TerminalId(1) }));
        assert_eq!(workspace.stack().active_terminal_id(), Some(TerminalId(1)));
    }

    #[test]
    fn given_no_session_when_splitting_then_nothing_happens() {
        let mut workspace = Workspace::default();
        assert!(workspace.split_left_right().is_empty());
        assert!(workspace.split_top_bottom().is_empty());
        assert!(workspace.split_auto(GEOMETRY).is_empty());
        assert!(workspace.focus_next_terminal().is_empty());
    }

    #[test]
    fn given_a_wide_area_when_splitting_automatically_then_panes_sit_side_by_side() {
        let mut workspace = workspace_with(1);
        workspace.split_auto(GEOMETRY);
        let panes = workspace.stack().active_session().expect("session").tree().layout(GEOMETRY.area, 0);
        assert_eq!(panes[0].rect.width, 500);
        workspace.split_top_bottom();
        assert_eq!(workspace.stack().active_session().expect("session").terminal_count(), 3);
    }

    #[test]
    fn given_two_terminals_when_the_active_one_closes_then_the_other_is_focused() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        let CloseOutcome::Done(effects) = workspace.close_active_terminal(None, Confirmation::Ask) else {
            panic!("closed")
        };
        assert!(effects.contains(&WorkspaceEffect::TerminalRemoved { session: SessionId(0), terminal: TerminalId(1) }));
        assert!(effects.contains(&WorkspaceEffect::TerminalFocused { terminal: TerminalId(0) }));
        assert_eq!(workspace.stack().count(), 1);
    }

    #[test]
    fn given_one_terminal_when_it_closes_then_the_session_and_tab_go_too() {
        let mut workspace = workspace_with(2);
        let CloseOutcome::Done(effects) = workspace.close_active_terminal(None, Confirmation::Ask) else {
            panic!("closed")
        };
        assert!(effects.contains(&WorkspaceEffect::SessionRemoved { session: SessionId(1) }));
        assert_eq!(selected(&workspace), Some(SessionId(0)));
    }

    #[test]
    fn given_a_locked_session_when_closing_a_terminal_then_confirmation_is_required() {
        let mut workspace = workspace_with(1);
        workspace.set_session_closable(None, false);
        assert_eq!(
            workspace.close_active_terminal(None, Confirmation::Ask),
            CloseOutcome::ConfirmationRequired(CloseQuestion::Terminal)
        );
    }

    #[test]
    fn given_a_shell_that_exits_when_reported_then_it_closes_without_asking_even_if_locked() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        workspace.set_session_closable(None, false);
        let effects = workspace.terminal_exited(TerminalId(0));
        assert!(effects.contains(&WorkspaceEffect::TerminalRemoved { session: SessionId(0), terminal: TerminalId(0) }));
        assert!(workspace.terminal_exited(TerminalId(0)).is_empty());
        let last = workspace.terminal_exited(TerminalId(1));
        assert!(last.contains(&WorkspaceEffect::RetractWindow));
    }

    #[test]
    fn given_several_terminals_when_focusing_next_and_previous_then_focus_wraps() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        assert_eq!(workspace.focus_next_terminal()[0], WorkspaceEffect::TerminalFocused { terminal: TerminalId(0) });
        assert_eq!(
            workspace.focus_previous_terminal()[0],
            WorkspaceEffect::TerminalFocused { terminal: TerminalId(1) }
        );
    }

    #[test]
    fn given_a_click_in_another_terminal_when_activating_then_it_becomes_active() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        workspace.activate_terminal(TerminalId(0));
        assert_eq!(workspace.stack().active_terminal_id(), Some(TerminalId(0)));
        assert!(workspace.activate_terminal(TerminalId(99)).is_empty());
    }

    #[test]
    fn given_split_terminals_when_growing_then_pixels_gained_are_reported() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        workspace.focus_previous_terminal();
        assert_eq!(workspace.grow_active_terminal(GrowthDirection::Right, 50, GEOMETRY), Some(50));
        assert_eq!(workspace.grow_active_terminal(GrowthDirection::Left, 50, GEOMETRY), None);
        assert_eq!(Workspace::default().grow_active_terminal(GrowthDirection::Left, 5, GEOMETRY), None);
    }

    #[test]
    fn given_a_divider_drag_when_released_then_the_layout_changes() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        let path =
            workspace.stack().active_session().expect("session").tree().dividers(GEOMETRY.area, 0)[0].path.clone();
        assert_eq!(
            workspace.set_divider_ratio(SessionId(0), &path, 0.3),
            vec![WorkspaceEffect::LayoutChanged { session: SessionId(0) }]
        );
        assert!(workspace.set_divider_ratio(SessionId(8), &path, 0.3).is_empty());
    }

    #[test]
    fn given_the_active_terminal_title_when_it_changes_then_the_title_bar_updates_but_the_tab_keeps_its_name() {
        let mut workspace = workspace_with(1);
        let effects = workspace.set_terminal_title(TerminalId(0), "vim");
        assert_eq!(effects, vec![WorkspaceEffect::ActiveTitleChanged { title: "vim".to_owned() }]);
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("Shell"));
        assert!(workspace.set_terminal_title(TerminalId(0), "vim").is_empty());
        assert!(workspace.set_terminal_title(TerminalId(5), "x").is_empty());
    }

    #[test]
    fn given_dynamic_tab_titles_when_the_terminal_title_changes_then_the_tab_follows() {
        let mut workspace = Workspace::new(WorkspaceSettings { dynamic_tab_titles: true, ..Default::default() });
        workspace.add_session(SessionKind::Single);
        let effects = workspace.set_terminal_title(TerminalId(0), "vim");
        assert!(effects.contains(&WorkspaceEffect::TabsChanged));
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("vim"));
    }

    #[test]
    fn given_a_renamed_tab_when_dynamic_titles_arrive_then_the_name_is_kept() {
        let mut workspace = Workspace::new(WorkspaceSettings { dynamic_tab_titles: true, ..Default::default() });
        workspace.add_session(SessionKind::Single);
        workspace.rename_session(SessionId(0), "  build  ");
        workspace.set_terminal_title(TerminalId(0), "vim");
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("build"));
        workspace.rename_session(SessionId(0), "");
        workspace.set_terminal_title(TerminalId(0), "less");
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("less"));
    }

    #[test]
    fn given_titles_already_set_when_dynamic_titles_turn_on_then_tabs_pick_them_up() {
        let mut workspace = workspace_with(1);
        workspace.set_terminal_title(TerminalId(0), "vim");
        let effects = workspace.apply_settings(WorkspaceSettings { dynamic_tab_titles: true, ..Default::default() });
        assert_eq!(effects, vec![WorkspaceEffect::TabsChanged]);
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("vim"));
        assert!(
            workspace.apply_settings(WorkspaceSettings { dynamic_tab_titles: true, ..Default::default() }).is_empty()
        );
    }

    #[test]
    fn given_another_session_when_raised_then_the_title_bar_shows_its_title() {
        let mut workspace = workspace_with(2);
        workspace.set_terminal_title(TerminalId(0), "first");
        let effects = workspace.raise_session(SessionId(0));
        assert!(effects.contains(&WorkspaceEffect::ActiveTitleChanged { title: "first".to_owned() }));
        assert!(workspace.raise_session(SessionId(9)).is_empty());
    }

    #[test]
    fn given_a_session_when_toggling_keyboard_input_then_all_terminals_flip_together() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        assert!(!workspace.toggle_session_switch(None, TerminalSwitch::KeyboardInput));
        let session = workspace.stack().active_session().expect("session");
        assert!(
            session.any_disabled(TerminalSwitch::KeyboardInput) && !session.any_enabled(TerminalSwitch::KeyboardInput)
        );
        assert!(workspace.toggle_session_switch(None, TerminalSwitch::KeyboardInput));
        assert!(!Workspace::default().toggle_session_switch(None, TerminalSwitch::MonitorActivity));
    }

    #[test]
    fn given_one_session_when_checking_quit_then_it_proceeds() {
        assert_eq!(workspace_with(1).check_quit(), QuitCheck::Proceed);
        assert_eq!(Workspace::default().check_quit(), QuitCheck::Proceed);
    }

    #[test]
    fn given_several_sessions_when_checking_quit_then_confirmation_is_asked_unless_disabled() {
        assert_eq!(
            workspace_with(2).check_quit(),
            QuitCheck::Confirm { multiple_sessions: true, locked_sessions: false }
        );
        let mut quiet = Workspace::new(WorkspaceSettings { confirm_quit: false, ..Default::default() });
        quiet.add_session(SessionKind::Single);
        quiet.add_session(SessionKind::Single);
        assert_eq!(quiet.check_quit(), QuitCheck::Proceed);
    }

    #[test]
    fn given_a_locked_session_when_checking_quit_then_confirmation_is_asked_even_without_confirm_quit() {
        let mut workspace = Workspace::new(WorkspaceSettings { confirm_quit: false, ..Default::default() });
        workspace.add_session(SessionKind::Single);
        workspace.set_session_closable(None, false);
        assert_eq!(workspace.check_quit(), QuitCheck::Confirm { multiple_sessions: false, locked_sessions: true });
    }

    #[test]
    fn given_a_background_session_when_splitting_it_by_id_then_the_new_terminal_does_not_steal_focus() {
        let mut workspace = workspace_with(2);
        let outcome = workspace.split_by_id(SplitTarget::SessionActive(SessionId(0)), SplitHow::LeftRight, GEOMETRY);
        assert_eq!(outcome.terminal, Some(TerminalId(2)));
        assert!(!outcome.effects.iter().any(|effect| matches!(effect, WorkspaceEffect::TerminalFocused { .. })));
        assert_eq!(workspace.stack().active_session_id(), Some(SessionId(1)));
        assert_eq!(workspace.stack().session(SessionId(0)).expect("session").terminal_count(), 2);
    }

    #[test]
    fn given_the_active_session_when_splitting_a_terminal_by_id_then_the_new_one_is_focused() {
        let mut workspace = workspace_with(1);
        let outcome = workspace.split_by_id(SplitTarget::Terminal(TerminalId(0)), SplitHow::TopBottom, GEOMETRY);
        assert!(outcome.effects.contains(&WorkspaceEffect::TerminalFocused { terminal: TerminalId(1) }));
        let auto = workspace.split_by_id(SplitTarget::Terminal(TerminalId(1)), SplitHow::Auto, GEOMETRY);
        assert_eq!(auto.terminal, Some(TerminalId(2)));
    }

    #[test]
    fn given_unknown_ids_when_splitting_by_id_then_nothing_happens() {
        let mut workspace = workspace_with(1);
        assert_eq!(
            workspace.split_by_id(SplitTarget::Terminal(TerminalId(9)), SplitHow::LeftRight, GEOMETRY),
            SplitOutcome::default()
        );
        assert_eq!(
            workspace.split_by_id(SplitTarget::SessionActive(SessionId(9)), SplitHow::LeftRight, GEOMETRY),
            SplitOutcome::default()
        );
    }

    #[test]
    fn given_a_locked_session_when_removing_a_terminal_by_id_then_it_stays() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        workspace.set_session_closable(None, false);
        assert!(workspace.remove_terminal(TerminalId(0)).is_empty());
        workspace.set_session_closable(None, true);
        assert!(!workspace.remove_terminal(TerminalId(0)).is_empty());
        assert!(workspace.remove_terminal(TerminalId(0)).is_empty());
    }

    #[test]
    fn given_split_terminals_when_growing_one_by_id_then_pixels_are_reported() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        assert_eq!(workspace.grow_terminal(TerminalId(0), GrowthDirection::Right, 30, GEOMETRY), Some(30));
        assert_eq!(workspace.grow_terminal(TerminalId(9), GrowthDirection::Right, 30, GEOMETRY), None);
    }

    #[test]
    fn given_switches_when_set_by_id_then_one_terminal_or_the_whole_session_changes() {
        let mut workspace = workspace_with(1);
        workspace.split_left_right();
        workspace.set_switch(SessionId(0), Some(TerminalId(0)), TerminalSwitch::MonitorSilence, true);
        let session = workspace.stack().session(SessionId(0)).expect("session");
        assert!(session.switch_state(TerminalSwitch::MonitorSilence, TerminalId(0)));
        assert!(!session.switch_state(TerminalSwitch::MonitorSilence, TerminalId(1)));
        workspace.set_switch(SessionId(0), None, TerminalSwitch::MonitorSilence, true);
        assert!(workspace.stack().session(SessionId(0)).expect("session").all_enabled(TerminalSwitch::MonitorSilence));
    }

    #[test]
    fn given_a_scripted_title_when_set_interactively_then_dynamic_titles_no_longer_replace_it() {
        let mut workspace = Workspace::new(WorkspaceSettings { dynamic_tab_titles: true, ..Default::default() });
        workspace.add_session(SessionKind::Single);
        workspace.set_tab_title(SessionId(0), "deploy", true);
        workspace.set_terminal_title(TerminalId(0), "vim");
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("deploy"));
        workspace.set_tab_title(SessionId(0), "", true);
        workspace.set_terminal_title(TerminalId(0), "less");
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("less"));
        workspace.set_tab_title(SessionId(0), "auto", false);
        assert_eq!(workspace.tabs().title(SessionId(0)), Some("auto"));
    }
}
