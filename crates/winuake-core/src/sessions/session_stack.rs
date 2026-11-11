// Ported from yakuake app/sessionstack.cpp @21c6210
//! All sessions and which one is active.

use super::session::{Session, SessionKind};
use crate::ids::{IdAllocator, SessionId, TerminalId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct SessionStack {
    ids: IdAllocator,
    sessions: BTreeMap<SessionId, Session>,
    active: Option<SessionId>,
}

impl SessionStack {
    pub fn ids_mut(&mut self) -> &mut IdAllocator {
        &mut self.ids
    }

    /// Creates a session. It does not become active until raised.
    pub fn add_session(&mut self, kind: SessionKind) -> SessionId {
        let id = self.ids.next_session();
        let session = Session::new(id, kind, &mut self.ids);
        self.sessions.insert(id, session);
        id
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    /// Splits the borrow so a session can allocate terminal ids.
    pub fn session_with_ids(&mut self, id: SessionId) -> Option<(&mut Session, &mut IdAllocator)> {
        let session = self.sessions.get_mut(&id)?;
        Some((session, &mut self.ids))
    }

    pub fn active_session_id(&self) -> Option<SessionId> {
        self.active
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.active.and_then(|id| self.sessions.get(&id))
    }

    pub fn active_terminal_id(&self) -> Option<TerminalId> {
        self.active_session().and_then(Session::active_terminal)
    }

    pub fn raise_session(&mut self, id: SessionId) -> bool {
        if !self.sessions.contains_key(&id) {
            return false;
        }
        self.active = Some(id);
        true
    }

    /// Drops the session. The active one is forgotten: the caller raises another.
    pub fn remove_session(&mut self, id: SessionId) -> Option<Session> {
        if self.active == Some(id) {
            self.active = None;
        }
        self.sessions.remove(&id)
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    pub fn terminal_ids(&self) -> Vec<TerminalId> {
        self.sessions.values().flat_map(Session::terminal_ids).collect()
    }

    pub fn session_for_terminal(&self, terminal: TerminalId) -> Option<SessionId> {
        self.sessions.values().find(|session| session.has_terminal(terminal)).map(Session::id)
    }

    pub fn has_unclosable_sessions(&self) -> bool {
        self.sessions.values().any(|session| !session.closable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_new_sessions_when_added_then_ids_count_up_and_none_is_active() {
        let mut stack = SessionStack::default();
        assert_eq!(stack.add_session(SessionKind::Single), SessionId(0));
        assert_eq!(stack.add_session(SessionKind::Quad), SessionId(1));
        assert_eq!(stack.count(), 2);
        assert_eq!(stack.active_session_id(), None);
        assert_eq!(stack.terminal_ids().len(), 5);
    }

    #[test]
    fn given_a_session_when_raised_then_it_is_active_with_its_terminal() {
        let mut stack = SessionStack::default();
        let id = stack.add_session(SessionKind::Single);
        assert!(stack.raise_session(id));
        assert_eq!(stack.active_terminal_id(), Some(TerminalId(0)));
        assert!(!stack.raise_session(SessionId(9)));
    }

    #[test]
    fn given_the_active_session_when_removed_then_nothing_is_active() {
        let mut stack = SessionStack::default();
        let id = stack.add_session(SessionKind::Single);
        stack.raise_session(id);
        assert!(stack.remove_session(id).is_some());
        assert_eq!(stack.active_session_id(), None);
        assert_eq!(stack.active_terminal_id(), None);
        assert!(stack.remove_session(id).is_none());
    }

    #[test]
    fn given_two_sessions_when_looking_up_a_terminal_then_its_session_is_found() {
        let mut stack = SessionStack::default();
        stack.add_session(SessionKind::Single);
        let second = stack.add_session(SessionKind::TwoHorizontal);
        assert_eq!(stack.session_for_terminal(TerminalId(2)), Some(second));
        assert_eq!(stack.session_for_terminal(TerminalId(40)), None);
        assert_eq!(stack.session_ids(), vec![SessionId(0), SessionId(1)]);
    }

    #[test]
    fn given_a_locked_session_when_asking_then_unclosable_sessions_exist() {
        let mut stack = SessionStack::default();
        let id = stack.add_session(SessionKind::Single);
        assert!(!stack.has_unclosable_sessions());
        stack.session_mut(id).expect("session").set_closable(false);
        assert!(stack.has_unclosable_sessions());
    }

    #[test]
    fn given_a_session_when_splitting_through_the_stack_ids_stay_unique() {
        let mut stack = SessionStack::default();
        let id = stack.add_session(SessionKind::Single);
        let other = stack.add_session(SessionKind::Single);
        let (session, ids) = stack.session_with_ids(id).expect("session");
        let added = session.split_left_right(None, ids);
        assert_eq!(added, Some(TerminalId(2)));
        assert!(stack.session_with_ids(SessionId(99)).is_none());
        assert_eq!(stack.session(other).expect("other").terminal_ids(), vec![TerminalId(1)]);
    }
}
