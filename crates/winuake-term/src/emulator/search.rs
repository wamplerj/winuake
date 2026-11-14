//! Find in the terminal's screen and scrollback. The current match is shown as the selection, so the renderer needs nothing new and
//! copying copies the match.

use super::TerminalEmulator;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::search::RegexSearch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub case_sensitive: bool,
    /// The text is a regular expression; otherwise it is searched for literally.
    pub regex: bool,
}

/// Where a search starts relative to the match shown now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStep {
    /// The query changed while typing: keep the current match if it still fits, else the next one from there.
    Refine,
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOutcome {
    /// A match is selected and scrolled into view.
    Found,
    NotFound,
    /// The query is not a valid regular expression.
    InvalidPattern,
}

impl TerminalEmulator {
    /// Finds a match of `query` and selects it. Searching wraps around the ends of the scrollback.
    pub fn find(&mut self, query: &SearchQuery, step: SearchStep) -> SearchOutcome {
        if query.text.is_empty() {
            self.clear_search();
            return SearchOutcome::NotFound;
        }
        let Ok(mut regex) = RegexSearch::new(&pattern_for(query)) else { return SearchOutcome::InvalidPattern };
        let (origin, direction, side) = self.search_origin(step);
        match self.term.search_next(&mut regex, origin, direction, side, None) {
            Some(found) => {
                let mut selection = Selection::new(SelectionType::Simple, *found.start(), Side::Left);
                selection.update(*found.end(), Side::Right);
                self.term.selection = Some(selection);
                self.term.scroll_to_point(*found.start());
                self.search_match = Some(found);
                SearchOutcome::Found
            }
            None => {
                self.clear_search();
                SearchOutcome::NotFound
            }
        }
    }

    /// Forgets the current match and drops its highlight.
    pub fn clear_search(&mut self) {
        if self.search_match.take().is_some() {
            self.term.selection = None;
        }
    }

    fn search_origin(&self, step: SearchStep) -> (Point, Direction, Side) {
        let offset = self.term.grid().display_offset() as i32;
        match (&self.search_match, step) {
            (Some(current), SearchStep::Refine) => (*current.start(), Direction::Right, Side::Left),
            (Some(current), SearchStep::Next) => {
                (current.end().add(&self.term, Boundary::None, 1), Direction::Right, Side::Left)
            }
            (Some(current), SearchStep::Previous) => {
                (current.start().sub(&self.term, Boundary::None, 1), Direction::Left, Side::Right)
            }
            (None, SearchStep::Previous) => (
                Point::new(Line(self.term.screen_lines() as i32 - 1 - offset), self.term.last_column()),
                Direction::Left,
                Side::Right,
            ),
            (None, _) => (Point::new(Line(-offset), Column(0)), Direction::Right, Side::Left),
        }
    }
}

/// The regular expression to run: the query as written (or escaped), with case sensitivity forced either way.
fn pattern_for(query: &SearchQuery) -> String {
    let body = if query.regex { query.text.clone() } else { escape_regex(&query.text) };
    let case = if query.case_sensitive { "(?-i)" } else { "(?i)" };
    format!("{case}{body}")
}

fn escape_regex(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::TerminalSize;

    fn terminal_with(text: &[u8]) -> TerminalEmulator {
        let mut terminal = TerminalEmulator::new(TerminalSize::new(40, 4), 1000);
        terminal.advance(text);
        terminal
    }

    fn literal(text: &str) -> SearchQuery {
        SearchQuery { text: text.to_owned(), case_sensitive: false, regex: false }
    }

    fn selected_column(terminal: &TerminalEmulator) -> usize {
        terminal
            .term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&terminal.term))
            .expect("a match")
            .start
            .column
            .0
    }

    #[test]
    fn given_repeated_text_when_stepping_next_then_each_match_is_selected_in_order_and_it_wraps() {
        let mut terminal = terminal_with(b"alpha beta alpha");
        let query = literal("alpha");
        assert_eq!(terminal.find(&query, SearchStep::Next), SearchOutcome::Found);
        assert_eq!((terminal.selection_text().as_deref(), selected_column(&terminal)), (Some("alpha"), 0));
        terminal.find(&query, SearchStep::Next);
        assert_eq!(selected_column(&terminal), 11);
        terminal.find(&query, SearchStep::Next);
        assert_eq!(selected_column(&terminal), 0);
    }

    #[test]
    fn given_a_match_when_stepping_previous_then_the_earlier_one_is_selected_and_it_wraps() {
        let mut terminal = terminal_with(b"alpha beta alpha");
        let query = literal("alpha");
        terminal.find(&query, SearchStep::Previous);
        assert_eq!(selected_column(&terminal), 11);
        terminal.find(&query, SearchStep::Previous);
        assert_eq!(selected_column(&terminal), 0);
        terminal.find(&query, SearchStep::Previous);
        assert_eq!(selected_column(&terminal), 11);
    }

    #[test]
    fn given_mixed_case_text_when_searching_then_the_case_option_decides_what_matches() {
        let mut terminal = terminal_with(b"Alpha alpha");
        let sensitive = SearchQuery { case_sensitive: true, ..literal("alpha") };
        terminal.find(&sensitive, SearchStep::Next);
        assert_eq!(selected_column(&terminal), 6);
        terminal.clear_search();
        terminal.find(&literal("alpha"), SearchStep::Next);
        assert_eq!(selected_column(&terminal), 0);
    }

    #[test]
    fn given_an_uppercase_query_when_case_is_insensitive_then_lowercase_text_still_matches() {
        let mut terminal = terminal_with(b"say alpha");
        assert_eq!(terminal.find(&literal("ALPHA"), SearchStep::Next), SearchOutcome::Found);
    }

    #[test]
    fn given_regex_characters_when_searching_literally_then_they_match_only_themselves() {
        let mut terminal = terminal_with(b"abc a.c");
        terminal.find(&literal("a.c"), SearchStep::Next);
        assert_eq!(selected_column(&terminal), 4);
    }

    #[test]
    fn given_the_regex_option_when_searching_then_the_pattern_is_interpreted() {
        let mut terminal = terminal_with(b"abc a.c");
        terminal.find(&SearchQuery { regex: true, ..literal("a.c") }, SearchStep::Next);
        assert_eq!(selected_column(&terminal), 0);
    }

    #[test]
    fn given_a_broken_regex_when_searching_then_the_outcome_says_so_and_nothing_changes() {
        let mut terminal = terminal_with(b"(x)");
        assert_eq!(
            terminal.find(&SearchQuery { regex: true, ..literal("(") }, SearchStep::Next),
            SearchOutcome::InvalidPattern
        );
        assert!(!terminal.has_selection());
    }

    #[test]
    fn given_no_match_when_searching_then_nothing_is_selected() {
        let mut terminal = terminal_with(b"alpha");
        terminal.find(&literal("alpha"), SearchStep::Next);
        assert_eq!(terminal.find(&literal("zeta"), SearchStep::Next), SearchOutcome::NotFound);
        assert!(!terminal.has_selection());
    }

    #[test]
    fn given_an_empty_query_when_searching_then_the_highlight_is_dropped() {
        let mut terminal = terminal_with(b"alpha");
        terminal.find(&literal("alpha"), SearchStep::Next);
        assert_eq!(terminal.find(&literal(""), SearchStep::Next), SearchOutcome::NotFound);
        assert!(!terminal.has_selection());
    }

    #[test]
    fn given_a_match_when_the_query_grows_then_refining_keeps_the_same_place() {
        let mut terminal = terminal_with(b"alpha beta alphabet");
        terminal.find(&literal("alp"), SearchStep::Refine);
        terminal.find(&literal("alp"), SearchStep::Next);
        assert_eq!(selected_column(&terminal), 11);
        terminal.find(&literal("alpha"), SearchStep::Refine);
        assert_eq!(selected_column(&terminal), 11);
    }

    #[test]
    fn given_a_match_in_the_scrollback_when_found_then_the_view_scrolls_to_it() {
        let mut terminal = TerminalEmulator::new(TerminalSize::new(20, 2), 100);
        terminal.advance(b"needle\r\n1\r\n2\r\n3\r\n4\r\n5\r\n");
        assert_eq!(terminal.display_offset(), 0);
        assert_eq!(terminal.find(&literal("needle"), SearchStep::Next), SearchOutcome::Found);
        assert!(terminal.display_offset() > 0);
        assert_eq!(terminal.selection_text().as_deref(), Some("needle"));
    }

    #[test]
    fn given_a_match_when_clearing_the_search_then_the_selection_goes_away() {
        let mut terminal = terminal_with(b"alpha");
        terminal.find(&literal("alpha"), SearchStep::Next);
        terminal.clear_search();
        assert!(!terminal.has_selection());
    }
}
