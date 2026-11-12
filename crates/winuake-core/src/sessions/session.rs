// Ported from yakuake app/session.cpp @21c6210
//! One tab: a split tree of terminals, which one is active, its title, and whether it may be closed casually.

use super::split_tree::{GrowthDirection, Orientation, PaneRect, SplitPath, SplitTree};
use crate::ids::{IdAllocator, SessionId, TerminalId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Single,
    TwoHorizontal,
    TwoVertical,
    Quad,
    /// No terminals: a page of the app itself (settings) shown in a tab.
    Page,
}

/// Per-terminal switches that Yakuake exposes in the tab context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSwitch {
    KeyboardInput,
    MonitorActivity,
    MonitorSilence,
}

#[derive(Debug, Clone)]
struct TerminalState {
    title: String,
    keyboard_input_enabled: bool,
    monitor_activity: bool,
    monitor_silence: bool,
}

impl TerminalState {
    fn new() -> Self {
        Self { title: String::new(), keyboard_input_enabled: true, monitor_activity: false, monitor_silence: false }
    }

    fn switch(&self, switch: TerminalSwitch) -> bool {
        match switch {
            TerminalSwitch::KeyboardInput => self.keyboard_input_enabled,
            TerminalSwitch::MonitorActivity => self.monitor_activity,
            TerminalSwitch::MonitorSilence => self.monitor_silence,
        }
    }

    fn set_switch(&mut self, switch: TerminalSwitch, enabled: bool) {
        match switch {
            TerminalSwitch::KeyboardInput => self.keyboard_input_enabled = enabled,
            TerminalSwitch::MonitorActivity => self.monitor_activity = enabled,
            TerminalSwitch::MonitorSilence => self.monitor_silence = enabled,
        }
    }
}

/// What closing a terminal did to its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalClosure {
    pub terminal: TerminalId,
    /// The terminal that became active because the closed one was.
    pub focus_moved_to: Option<TerminalId>,
    /// The last terminal went away, so the session itself is finished.
    pub session_empty: bool,
}

#[derive(Debug, Clone)]
pub struct Session {
    id: SessionId,
    title: String,
    closable: bool,
    tree: SplitTree,
    /// Ordered by id: next/previous terminal walks this order, not the visual one (as in Yakuake).
    terminals: BTreeMap<TerminalId, TerminalState>,
    active: Option<TerminalId>,
    page: bool,
}

impl Session {
    pub fn new(id: SessionId, kind: SessionKind, ids: &mut IdAllocator) -> Self {
        let mut created = Vec::new();
        let mut new_terminal = |ids: &mut IdAllocator| {
            let terminal = ids.next_terminal();
            created.push(terminal);
            terminal
        };
        let tree = match kind {
            SessionKind::Single => SplitTree::single(new_terminal(ids)),
            SessionKind::TwoHorizontal => {
                SplitTree::pair(Orientation::Horizontal, new_terminal(ids), new_terminal(ids))
            }
            SessionKind::TwoVertical => SplitTree::pair(Orientation::Vertical, new_terminal(ids), new_terminal(ids)),
            SessionKind::Page => SplitTree::empty(),
            SessionKind::Quad => {
                let top = [new_terminal(ids), new_terminal(ids)];
                let bottom = [new_terminal(ids), new_terminal(ids)];
                SplitTree::grid(top, bottom)
            }
        };
        let active = created.first().copied();
        let terminals = created.into_iter().map(|terminal| (terminal, TerminalState::new())).collect();
        Self { id, title: String::new(), closable: true, tree, terminals, active, page: kind == SessionKind::Page }
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    /// A page of the app (settings) rather than a set of terminals.
    pub fn is_page(&self) -> bool {
        self.page
    }

    /// Gives a page session its title (terminal sessions take theirs from the active terminal).
    pub fn set_page_title(&mut self, title: &str) {
        self.title = title.to_owned();
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn closable(&self) -> bool {
        self.closable
    }

    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }

    pub fn tree(&self) -> &SplitTree {
        &self.tree
    }

    pub fn active_terminal(&self) -> Option<TerminalId> {
        self.active
    }

    pub fn set_divider_ratio(&mut self, path: &SplitPath, ratio: f32) -> bool {
        self.tree.set_ratio(path, ratio)
    }

    pub fn terminal_ids(&self) -> Vec<TerminalId> {
        self.terminals.keys().copied().collect()
    }

    pub fn terminal_count(&self) -> usize {
        self.terminals.len()
    }

    pub fn has_terminal(&self, terminal: TerminalId) -> bool {
        self.terminals.contains_key(&terminal)
    }

    pub fn terminal_title(&self, terminal: TerminalId) -> Option<&str> {
        self.terminals.get(&terminal).map(|state| state.title.as_str())
    }

    /// `None` means the active terminal, as the `-1` default does upstream.
    fn resolve(&self, terminal: Option<TerminalId>) -> Option<TerminalId> {
        terminal.or(self.active).filter(|candidate| self.terminals.contains_key(candidate))
    }

    /// Marks `terminal` active and adopts its title for the session. Returns the new session title when it changed.
    pub fn set_active_terminal(&mut self, terminal: TerminalId) -> Option<String> {
        let state = self.terminals.get(&terminal)?;
        self.active = Some(terminal);
        self.adopt_title(state.title.clone())
    }

    fn adopt_title(&mut self, title: String) -> Option<String> {
        if self.title == title {
            return None;
        }
        self.title = title.clone();
        Some(title)
    }

    /// Records a terminal's title; the session shows it when that terminal is the active one. Returns the new session title.
    pub fn set_terminal_title(&mut self, terminal: TerminalId, title: &str) -> Option<String> {
        self.terminals.get_mut(&terminal)?.title = title.to_owned();
        if self.active == Some(terminal) { self.adopt_title(title.to_owned()) } else { None }
    }

    pub fn split_left_right(&mut self, target: Option<TerminalId>, ids: &mut IdAllocator) -> Option<TerminalId> {
        self.split(target, Orientation::Horizontal, ids)
    }

    pub fn split_top_bottom(&mut self, target: Option<TerminalId>, ids: &mut IdAllocator) -> Option<TerminalId> {
        self.split(target, Orientation::Vertical, ids)
    }

    /// Splits along the longer side of the pane as laid out in `area`: left/right when wider than tall, else top/bottom.
    pub fn split_auto(
        &mut self,
        target: Option<TerminalId>,
        area: PaneRect,
        divider: i32,
        ids: &mut IdAllocator,
    ) -> Option<TerminalId> {
        let terminal = self.resolve(target)?;
        let pane = self.tree.layout(area, divider).into_iter().find(|pane| pane.terminal == terminal)?;
        let orientation =
            if pane.rect.width > pane.rect.height { Orientation::Horizontal } else { Orientation::Vertical };
        self.split(Some(terminal), orientation, ids)
    }

    fn split(
        &mut self,
        target: Option<TerminalId>,
        orientation: Orientation,
        ids: &mut IdAllocator,
    ) -> Option<TerminalId> {
        let terminal = self.resolve(target)?;
        let added = ids.next_terminal();
        if !self.tree.split(terminal, orientation, added) {
            return None;
        }
        self.terminals.insert(added, TerminalState::new());
        self.active = Some(added);
        let title = self.terminals[&added].title.clone();
        self.adopt_title(title);
        Some(added)
    }

    /// Ported from `Session::cleanup(int)`: when the active terminal closes and others remain, focus moves to the previous one.
    pub fn close_terminal(&mut self, target: Option<TerminalId>) -> Option<TerminalClosure> {
        let terminal = self.resolve(target)?;
        let mut focus_moved_to = None;
        if self.active == Some(terminal) && self.terminals.len() > 1 {
            focus_moved_to = self.neighbor(terminal, false);
            if let Some(previous) = focus_moved_to {
                self.set_active_terminal(previous);
            }
        }
        self.terminals.remove(&terminal);
        self.tree.remove(terminal);
        let session_empty = self.terminals.is_empty();
        if session_empty {
            self.active = None;
        }
        Some(TerminalClosure { terminal, focus_moved_to, session_empty })
    }

    /// Terminal after (or before) `from` in id order, wrapping.
    fn neighbor(&self, from: TerminalId, forward: bool) -> Option<TerminalId> {
        let ids: Vec<TerminalId> = self.terminals.keys().copied().collect();
        let index = ids.iter().position(|id| *id == from)?;
        let count = ids.len();
        let next = if forward { (index + 1) % count } else { (index + count - 1) % count };
        Some(ids[next])
    }

    pub fn focus_next_terminal(&mut self) -> Option<TerminalId> {
        self.focus_neighbor(true)
    }

    pub fn focus_previous_terminal(&mut self) -> Option<TerminalId> {
        self.focus_neighbor(false)
    }

    fn focus_neighbor(&mut self, forward: bool) -> Option<TerminalId> {
        let current = self.resolve(None)?;
        let target = self.neighbor(current, forward)?;
        self.set_active_terminal(target);
        Some(target)
    }

    /// See `SplitTree::try_grow`.
    pub fn try_grow_terminal(
        &mut self,
        terminal: TerminalId,
        direction: GrowthDirection,
        pixels: i32,
        area: PaneRect,
        divider: i32,
        minimum_extent: i32,
    ) -> Option<i32> {
        self.tree.try_grow(terminal, direction, pixels, area, divider, minimum_extent)
    }

    pub fn switch_state(&self, switch: TerminalSwitch, terminal: TerminalId) -> bool {
        self.terminals.get(&terminal).is_some_and(|state| state.switch(switch))
    }

    /// `None` applies to every terminal of the session.
    pub fn set_switch(&mut self, switch: TerminalSwitch, terminal: Option<TerminalId>, enabled: bool) {
        match terminal {
            Some(id) => {
                if let Some(state) = self.terminals.get_mut(&id) {
                    state.set_switch(switch, enabled);
                }
            }
            None => self.terminals.values_mut().for_each(|state| state.set_switch(switch, enabled)),
        }
    }

    pub fn all_enabled(&self, switch: TerminalSwitch) -> bool {
        self.terminals.values().all(|state| state.switch(switch))
    }

    pub fn any_enabled(&self, switch: TerminalSwitch) -> bool {
        self.terminals.values().any(|state| state.switch(switch))
    }

    pub fn any_disabled(&self, switch: TerminalSwitch) -> bool {
        self.terminals.values().any(|state| !state.switch(switch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: PaneRect = PaneRect::new(0, 0, 1000, 400);
    const TALL: PaneRect = PaneRect::new(0, 0, 400, 1000);

    fn session(kind: SessionKind) -> (Session, IdAllocator) {
        let mut ids = IdAllocator::default();
        let id = ids.next_session();
        (Session::new(id, kind, &mut ids), ids)
    }

    #[test]
    fn given_a_single_session_when_created_then_it_has_one_active_terminal() {
        let (session, _) = session(SessionKind::Single);
        assert_eq!(session.terminal_ids(), vec![TerminalId(0)]);
        assert_eq!(session.active_terminal(), Some(TerminalId(0)));
        assert!(session.closable());
    }

    #[test]
    fn given_two_horizontal_or_vertical_or_quad_when_created_then_terminal_counts_match() {
        assert_eq!(session(SessionKind::TwoHorizontal).0.terminal_count(), 2);
        assert_eq!(session(SessionKind::TwoVertical).0.terminal_count(), 2);
        let (quad, _) = session(SessionKind::Quad);
        assert_eq!(quad.terminal_count(), 4);
        assert_eq!(quad.active_terminal(), Some(TerminalId(0)));
    }

    #[test]
    fn given_a_session_when_splitting_left_right_then_the_new_terminal_is_active() {
        let (mut session, mut ids) = session(SessionKind::Single);
        let added = session.split_left_right(None, &mut ids);
        assert_eq!(added, Some(TerminalId(1)));
        assert_eq!(session.active_terminal(), added);
        assert_eq!(session.tree().terminals_in_layout_order(), vec![TerminalId(0), TerminalId(1)]);
    }

    #[test]
    fn given_a_missing_terminal_when_splitting_then_nothing_is_added() {
        let (mut session, mut ids) = session(SessionKind::Single);
        assert_eq!(session.split_top_bottom(Some(TerminalId(99)), &mut ids), None);
        assert_eq!(session.terminal_count(), 1);
    }

    #[test]
    fn given_a_wide_pane_when_splitting_automatically_then_it_splits_left_right() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_auto(None, WIDE, 0, &mut ids);
        let rects = session.tree().layout(WIDE, 0);
        assert_eq!(rects[0].rect.height, 400);
        assert_eq!(rects[0].rect.width, 500);
    }

    #[test]
    fn given_a_tall_pane_when_splitting_automatically_then_it_splits_top_bottom() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_auto(None, TALL, 0, &mut ids);
        let rects = session.tree().layout(TALL, 0);
        assert_eq!(rects[0].rect.width, 400);
        assert_eq!(rects[0].rect.height, 500);
    }

    #[test]
    fn given_three_terminals_when_focusing_next_then_it_wraps_in_id_order() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_left_right(None, &mut ids);
        session.split_top_bottom(None, &mut ids);
        assert_eq!(session.active_terminal(), Some(TerminalId(2)));
        assert_eq!(session.focus_next_terminal(), Some(TerminalId(0)));
        assert_eq!(session.focus_previous_terminal(), Some(TerminalId(2)));
        assert_eq!(session.focus_previous_terminal(), Some(TerminalId(1)));
    }

    #[test]
    fn given_the_active_terminal_when_closed_then_the_previous_one_gets_focus() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_left_right(None, &mut ids);
        let closure = session.close_terminal(None).expect("closure");
        assert_eq!(
            closure,
            TerminalClosure { terminal: TerminalId(1), focus_moved_to: Some(TerminalId(0)), session_empty: false }
        );
        assert_eq!(session.active_terminal(), Some(TerminalId(0)));
        assert_eq!(session.tree().terminals_in_layout_order(), vec![TerminalId(0)]);
    }

    #[test]
    fn given_an_inactive_terminal_when_closed_then_focus_stays() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_left_right(None, &mut ids);
        let closure = session.close_terminal(Some(TerminalId(0))).expect("closure");
        assert_eq!(closure.focus_moved_to, None);
        assert_eq!(session.active_terminal(), Some(TerminalId(1)));
    }

    #[test]
    fn given_the_last_terminal_when_closed_then_the_session_is_empty() {
        let (mut session, _) = session(SessionKind::Single);
        let closure = session.close_terminal(None).expect("closure");
        assert!(closure.session_empty);
        assert_eq!(session.active_terminal(), None);
        assert_eq!(session.close_terminal(None), None);
    }

    #[test]
    fn given_the_active_terminal_title_when_it_changes_then_the_session_title_follows() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_left_right(None, &mut ids);
        assert_eq!(session.set_terminal_title(TerminalId(1), "vim"), Some("vim".to_owned()));
        assert_eq!(session.set_terminal_title(TerminalId(0), "pwsh"), None);
        assert_eq!(session.title(), "vim");
        assert_eq!(session.set_terminal_title(TerminalId(1), "vim"), None);
    }

    #[test]
    fn given_another_terminal_when_it_becomes_active_then_its_title_becomes_the_session_title() {
        let (mut session, mut ids) = session(SessionKind::Single);
        session.split_left_right(None, &mut ids);
        session.set_terminal_title(TerminalId(0), "pwsh");
        assert_eq!(session.set_active_terminal(TerminalId(0)), Some("pwsh".to_owned()));
        assert_eq!(session.set_active_terminal(TerminalId(42)), None);
    }

    #[test]
    fn given_a_session_when_locking_then_it_is_not_closable() {
        let (mut session, _) = session(SessionKind::Single);
        session.set_closable(false);
        assert!(!session.closable());
    }

    #[test]
    fn given_two_terminals_when_one_has_input_disabled_then_the_aggregates_reflect_it() {
        let (mut session, _) = session(SessionKind::TwoHorizontal);
        session.set_switch(TerminalSwitch::KeyboardInput, Some(TerminalId(1)), false);
        assert!(!session.all_enabled(TerminalSwitch::KeyboardInput));
        assert!(session.any_enabled(TerminalSwitch::KeyboardInput));
        assert!(session.any_disabled(TerminalSwitch::KeyboardInput));
        assert!(!session.switch_state(TerminalSwitch::KeyboardInput, TerminalId(1)));
        session.set_switch(TerminalSwitch::KeyboardInput, None, true);
        assert!(session.all_enabled(TerminalSwitch::KeyboardInput));
    }

    #[test]
    fn given_monitoring_when_set_for_the_whole_session_then_every_terminal_is_monitored() {
        let (mut session, _) = session(SessionKind::Quad);
        assert!(!session.any_enabled(TerminalSwitch::MonitorActivity));
        session.set_switch(TerminalSwitch::MonitorActivity, None, true);
        session.set_switch(TerminalSwitch::MonitorSilence, Some(TerminalId(2)), true);
        assert!(session.all_enabled(TerminalSwitch::MonitorActivity));
        assert!(session.switch_state(TerminalSwitch::MonitorSilence, TerminalId(2)));
        assert!(!session.switch_state(TerminalSwitch::MonitorSilence, TerminalId(0)));
        assert!(!session.switch_state(TerminalSwitch::MonitorSilence, TerminalId(77)));
    }

    #[test]
    fn given_a_session_when_growing_a_terminal_then_the_layout_changes() {
        let (mut session, _) = session(SessionKind::TwoHorizontal);
        let grown = session.try_grow_terminal(TerminalId(0), GrowthDirection::Right, 100, WIDE, 0, 10);
        assert_eq!(grown, Some(100));
        assert_eq!(session.tree().layout(WIDE, 0)[0].rect.width, 600);
    }
}
