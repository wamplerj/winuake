//! VT emulation: bytes from the shell in, screen state and terminal events out. Wraps `alacritty_terminal`.

use crate::pty::TerminalSize;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use std::sync::mpsc::{self, Receiver, Sender};

pub const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

/// Things the terminal wants the host to know about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    TitleChanged(String),
    TitleReset,
    BellRung,
    /// Bytes the terminal wants written back to the shell (device attribute replies, cursor position reports, ...).
    ResponseForShell(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub column: usize,
}

struct EventForwarder {
    sender: Sender<Event>,
}

impl EventListener for EventForwarder {
    fn send_event(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

struct GridDimensions {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for GridDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

impl From<TerminalSize> for GridDimensions {
    fn from(size: TerminalSize) -> Self {
        Self { columns: usize::from(size.columns).max(1), screen_lines: usize::from(size.rows).max(1) }
    }
}

pub struct TerminalEmulator {
    term: Term<EventForwarder>,
    processor: Processor,
    events: Receiver<Event>,
}

impl TerminalEmulator {
    pub fn new(size: TerminalSize, scrollback_lines: usize) -> Self {
        let (sender, events) = mpsc::channel();
        let config = Config { scrolling_history: scrollback_lines, ..Config::default() };
        let term = Term::new(config, &GridDimensions::from(size), EventForwarder { sender });
        Self { term, processor: Processor::new(), events }
    }

    /// Feeds shell output through the VT parser and returns the host-visible events it produced.
    pub fn advance(&mut self, bytes: &[u8]) -> Vec<TerminalEvent> {
        self.processor.advance(&mut self.term, bytes);
        self.events.try_iter().filter_map(Self::translate_event).collect()
    }

    pub fn resize(&mut self, size: TerminalSize) {
        self.term.resize(GridDimensions::from(size));
    }

    pub fn columns(&self) -> usize {
        self.term.columns()
    }

    pub fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    pub fn scrollback_line_count(&self) -> usize {
        self.term.history_size()
    }

    pub fn cursor_position(&self) -> CursorPosition {
        let point = self.term.grid().cursor.point;
        CursorPosition { row: point.line.0.max(0) as usize, column: point.column.0 }
    }

    /// Text of one visible row with trailing blanks removed. Rows past the bottom are empty.
    pub fn row_text(&self, row: usize) -> String {
        if row >= self.rows() {
            return String::new();
        }
        let line = &self.term.grid()[Line(row as i32)];
        let mut text = String::with_capacity(self.columns());
        for column in 0..self.columns() {
            let cell = &line[Column(column)];
            if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                text.push(cell.c);
            }
        }
        text.truncate(text.trim_end().len());
        text
    }

    /// All visible rows joined with `\n`, trailing empty rows removed.
    pub fn screen_text(&self) -> String {
        let mut rows: Vec<String> = (0..self.rows()).map(|row| self.row_text(row)).collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows.join("\n")
    }

    fn translate_event(event: Event) -> Option<TerminalEvent> {
        match event {
            Event::Title(title) => Some(TerminalEvent::TitleChanged(title)),
            Event::ResetTitle => Some(TerminalEvent::TitleReset),
            Event::Bell => Some(TerminalEvent::BellRung),
            Event::PtyWrite(text) => Some(TerminalEvent::ResponseForShell(text.into_bytes())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emulator(columns: u16, rows: u16) -> TerminalEmulator {
        TerminalEmulator::new(TerminalSize::new(columns, rows), 100)
    }

    #[test]
    fn given_plain_text_when_advancing_then_it_appears_on_the_first_row() {
        let mut terminal = emulator(20, 5);
        terminal.advance(b"hello");
        assert_eq!(terminal.row_text(0), "hello");
        assert_eq!(terminal.cursor_position(), CursorPosition { row: 0, column: 5 });
    }

    #[test]
    fn given_crlf_when_advancing_then_text_moves_to_the_next_row() {
        let mut terminal = emulator(20, 5);
        terminal.advance(b"one\r\ntwo");
        assert_eq!(terminal.screen_text(), "one\ntwo");
        assert_eq!(terminal.cursor_position(), CursorPosition { row: 1, column: 3 });
    }

    #[test]
    fn given_text_longer_than_a_row_when_advancing_then_it_wraps() {
        let mut terminal = emulator(5, 3);
        terminal.advance(b"abcdefgh");
        assert_eq!(terminal.row_text(0), "abcde");
        assert_eq!(terminal.row_text(1), "fgh");
    }

    #[test]
    fn given_more_lines_than_rows_when_advancing_then_old_lines_go_to_scrollback() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"a\r\nb\r\nc\r\nd");
        assert_eq!(terminal.screen_text(), "c\nd");
        assert_eq!(terminal.scrollback_line_count(), 2);
    }

    #[test]
    fn given_sgr_color_sequences_when_advancing_then_only_the_text_remains() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"\x1b[31mred\x1b[0m plain");
        assert_eq!(terminal.row_text(0), "red plain");
    }

    #[test]
    fn given_erase_line_and_cursor_moves_when_advancing_then_screen_is_edited() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"hello\x1b[3D\x1b[K!");
        assert_eq!(terminal.row_text(0), "he!");
    }

    #[test]
    fn given_wide_characters_when_advancing_then_each_glyph_appears_once() {
        let mut terminal = emulator(10, 2);
        terminal.advance("a世b".as_bytes());
        assert_eq!(terminal.row_text(0), "a世b");
    }

    #[test]
    fn given_osc_title_sequence_when_advancing_then_title_changed_event_is_returned() {
        let mut terminal = emulator(20, 3);
        let events = terminal.advance(b"\x1b]0;my shell\x07");
        assert_eq!(events, vec![TerminalEvent::TitleChanged("my shell".to_owned())]);
    }

    #[test]
    fn given_bell_character_when_advancing_then_bell_event_is_returned() {
        let mut terminal = emulator(20, 3);
        assert_eq!(terminal.advance(b"\x07"), vec![TerminalEvent::BellRung]);
    }

    #[test]
    fn given_device_status_request_when_advancing_then_a_cursor_report_is_returned_for_the_shell() {
        let mut terminal = emulator(20, 5);
        terminal.advance(b"ab");
        let events = terminal.advance(b"\x1b[6n");
        assert_eq!(events, vec![TerminalEvent::ResponseForShell(b"\x1b[1;3R".to_vec())]);
    }

    #[test]
    fn given_resize_when_growing_then_dimensions_update_and_content_is_kept() {
        let mut terminal = emulator(10, 3);
        terminal.advance(b"keep");
        terminal.resize(TerminalSize::new(40, 10));
        assert_eq!((terminal.columns(), terminal.rows()), (40, 10));
        assert_eq!(terminal.row_text(0), "keep");
    }

    #[test]
    fn given_row_beyond_screen_when_reading_then_text_is_empty() {
        assert_eq!(emulator(10, 3).row_text(99), "");
    }

    #[test]
    fn given_zero_sized_terminal_when_created_then_it_is_clamped_to_one_cell() {
        let terminal = emulator(0, 0);
        assert_eq!((terminal.columns(), terminal.rows()), (1, 1));
    }
}
