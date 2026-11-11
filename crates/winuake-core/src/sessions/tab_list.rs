// Ported from yakuake app/tabbar.cpp @21c6210 (the non-painting logic)
//! Order, selection and titles of the tab bar.

use crate::ids::SessionId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSource {
    /// Typed by the user: pins the title against dynamic updates.
    Interactive,
    /// Derived from the terminal title.
    Automatic,
}

#[derive(Debug, Clone, Default)]
pub struct TabList {
    order: Vec<SessionId>,
    titles: HashMap<SessionId, String>,
    pinned: HashMap<SessionId, bool>,
    selected: Option<SessionId>,
}

impl TabList {
    pub fn order(&self) -> &[SessionId] {
        &self.order
    }

    pub fn selected(&self) -> Option<SessionId> {
        self.selected
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn title(&self, session: SessionId) -> Option<&str> {
        self.titles.get(&session).map(String::as_str)
    }

    pub fn index_of(&self, session: SessionId) -> Option<usize> {
        self.order.iter().position(|candidate| *candidate == session)
    }

    pub fn session_at(&self, index: usize) -> Option<SessionId> {
        self.order.get(index).copied()
    }

    /// Appends a tab. An empty title gets the first free standard title ("Shell", "Shell No. 2", ...).
    pub fn add(&mut self, session: SessionId, title: &str) {
        let title = if title.is_empty() { self.standard_title() } else { title.to_owned() };
        self.order.push(session);
        self.titles.insert(session, title);
    }

    /// Removes a tab. Returns the session that should now be selected, or `None` when no tabs remain. Removing the selected
    /// tab selects the last one.
    pub fn remove(&mut self, session: SessionId) -> Option<SessionId> {
        let index = self.index_of(session)?;
        self.order.remove(index);
        self.titles.remove(&session);
        self.pinned.remove(&session);
        let next = if self.selected == Some(session) { self.order.last().copied() } else { self.selected };
        self.selected = next;
        next
    }

    pub fn select(&mut self, session: SessionId) -> bool {
        if self.index_of(session).is_none() {
            return false;
        }
        self.selected = Some(session);
        true
    }

    pub fn next(&self) -> Option<SessionId> {
        self.step(1)
    }

    pub fn previous(&self) -> Option<SessionId> {
        self.step(-1)
    }

    fn step(&self, delta: isize) -> Option<SessionId> {
        let index = self.index_of(self.selected?)?;
        let count = self.order.len() as isize;
        self.order.get((index as isize + delta).rem_euclid(count) as usize).copied()
    }

    pub fn can_move_left(&self, session: SessionId) -> bool {
        self.index_of(session).is_some_and(|index| index > 0)
    }

    pub fn can_move_right(&self, session: SessionId) -> bool {
        self.index_of(session).is_some_and(|index| index + 1 < self.order.len())
    }

    pub fn move_left(&mut self, session: SessionId) -> bool {
        let Some(index) = self.index_of(session).filter(|index| *index > 0) else { return false };
        self.order.swap(index, index - 1);
        true
    }

    pub fn move_right(&mut self, session: SessionId) -> bool {
        let Some(index) = self.index_of(session).filter(|index| index + 1 < self.order.len()) else { return false };
        self.order.swap(index, index + 1);
        true
    }

    /// Moves a tab to `index` (clamped), for drag and drop.
    pub fn move_to(&mut self, session: SessionId, index: usize) -> bool {
        let Some(from) = self.index_of(session) else { return false };
        let tab = self.order.remove(from);
        self.order.insert(index.min(self.order.len()), tab);
        true
    }

    pub fn is_pinned(&self, session: SessionId) -> bool {
        self.pinned.get(&session).copied().unwrap_or(false)
    }

    /// Ported from `TabBar::setTabTitle`. Returns true when the shown title changed.
    /// An automatic update is ignored while pinned. An empty interactive title unpins and keeps the old text.
    pub fn set_title(&mut self, session: SessionId, title: &str, source: TitleSource) -> bool {
        if !self.titles.contains_key(&session) {
            return false;
        }
        match source {
            TitleSource::Automatic if self.is_pinned(session) => return false,
            TitleSource::Interactive => {
                self.pinned.insert(session, true);
            }
            TitleSource::Automatic => {}
        }
        if title.is_empty() {
            self.pinned.remove(&session);
            return false;
        }
        let changed = self.titles.get(&session).map(String::as_str) != Some(title);
        self.titles.insert(session, title.to_owned());
        changed
    }

    fn standard_title(&self) -> String {
        (0..)
            .map(|count| if count == 0 { "Shell".to_owned() } else { format!("Shell No. {}", count + 1) })
            .find(|candidate| !self.titles.values().any(|title| title == candidate))
            .expect("an unused title exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: SessionId = SessionId(0);
    const B: SessionId = SessionId(1);
    const C: SessionId = SessionId(2);

    fn three_tabs() -> TabList {
        let mut tabs = TabList::default();
        for session in [A, B, C] {
            tabs.add(session, "");
        }
        tabs.select(C);
        tabs
    }

    #[test]
    fn given_new_tabs_without_titles_when_added_then_standard_titles_count_up() {
        let tabs = three_tabs();
        assert_eq!(tabs.title(A), Some("Shell"));
        assert_eq!(tabs.title(B), Some("Shell No. 2"));
        assert_eq!(tabs.title(C), Some("Shell No. 3"));
    }

    #[test]
    fn given_a_freed_standard_title_when_adding_then_it_is_reused() {
        let mut tabs = three_tabs();
        tabs.remove(A);
        tabs.add(SessionId(9), "");
        assert_eq!(tabs.title(SessionId(9)), Some("Shell"));
    }

    #[test]
    fn given_a_title_when_adding_then_it_is_kept() {
        let mut tabs = TabList::default();
        tabs.add(A, "build");
        assert_eq!(tabs.title(A), Some("build"));
    }

    #[test]
    fn given_the_selected_tab_when_removed_then_the_last_tab_is_selected() {
        let mut tabs = three_tabs();
        tabs.select(A);
        assert_eq!(tabs.remove(A), Some(C));
        assert_eq!(tabs.selected(), Some(C));
    }

    #[test]
    fn given_another_tab_when_removed_then_the_selection_is_kept() {
        let mut tabs = three_tabs();
        tabs.select(B);
        assert_eq!(tabs.remove(C), Some(B));
    }

    #[test]
    fn given_the_last_tab_when_removed_then_nothing_is_selected() {
        let mut tabs = TabList::default();
        tabs.add(A, "");
        tabs.select(A);
        assert_eq!(tabs.remove(A), None);
        assert!(tabs.is_empty());
        assert_eq!(tabs.remove(A), None);
    }

    #[test]
    fn given_the_ends_when_stepping_then_selection_wraps() {
        let mut tabs = three_tabs();
        assert_eq!(tabs.next(), Some(A));
        tabs.select(A);
        assert_eq!(tabs.previous(), Some(C));
        assert_eq!(TabList::default().next(), None);
    }

    #[test]
    fn given_an_unknown_session_when_selecting_then_false() {
        let mut tabs = three_tabs();
        assert!(!tabs.select(SessionId(50)));
        assert_eq!(tabs.selected(), Some(C));
    }

    #[test]
    fn given_tabs_when_moving_left_and_right_then_neighbors_swap_until_the_edge() {
        let mut tabs = three_tabs();
        assert!(tabs.move_left(C));
        assert_eq!(tabs.order(), &[A, C, B]);
        assert!(tabs.move_right(A));
        assert_eq!(tabs.order(), &[C, A, B]);
        assert!(!tabs.move_left(C));
        assert!(!tabs.move_right(B));
        assert!(!tabs.can_move_left(C));
        assert!(tabs.can_move_right(C));
        assert!(!tabs.can_move_left(SessionId(70)));
    }

    #[test]
    fn given_a_tab_when_dragged_to_an_index_then_it_lands_there() {
        let mut tabs = three_tabs();
        assert!(tabs.move_to(A, 2));
        assert_eq!(tabs.order(), &[B, C, A]);
        assert!(tabs.move_to(A, 99));
        assert_eq!(tabs.order(), &[B, C, A]);
        assert!(!tabs.move_to(SessionId(70), 0));
    }

    #[test]
    fn given_the_tab_order_when_asking_by_index_then_the_session_is_found() {
        let tabs = three_tabs();
        assert_eq!(tabs.session_at(1), Some(B));
        assert_eq!(tabs.session_at(3), None);
    }

    #[test]
    fn given_an_automatic_title_when_set_then_the_tab_shows_it() {
        let mut tabs = three_tabs();
        assert!(tabs.set_title(A, "vim", TitleSource::Automatic));
        assert!(!tabs.set_title(A, "vim", TitleSource::Automatic));
        assert_eq!(tabs.title(A), Some("vim"));
    }

    #[test]
    fn given_a_renamed_tab_when_an_automatic_title_arrives_then_the_rename_wins() {
        let mut tabs = three_tabs();
        tabs.set_title(A, "mine", TitleSource::Interactive);
        assert!(!tabs.set_title(A, "vim", TitleSource::Automatic));
        assert_eq!(tabs.title(A), Some("mine"));
        assert!(tabs.is_pinned(A));
    }

    #[test]
    fn given_a_renamed_tab_when_renamed_to_nothing_then_it_unpins_and_keeps_the_text() {
        let mut tabs = three_tabs();
        tabs.set_title(A, "mine", TitleSource::Interactive);
        assert!(!tabs.set_title(A, "", TitleSource::Interactive));
        assert!(!tabs.is_pinned(A));
        assert_eq!(tabs.title(A), Some("mine"));
        assert!(tabs.set_title(A, "vim", TitleSource::Automatic));
    }

    #[test]
    fn given_an_unknown_tab_when_setting_a_title_then_false() {
        let mut tabs = three_tabs();
        assert!(!tabs.set_title(SessionId(70), "x", TitleSource::Interactive));
    }
}
