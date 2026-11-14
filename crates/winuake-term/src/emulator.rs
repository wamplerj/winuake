//! VT emulation: bytes from the shell in, screen state and terminal events out. Wraps `alacritty_terminal`.

use crate::input::{InputModes, MouseReporting};
use crate::pty::TerminalSize;
use crate::snapshot::CursorStyle;
use crate::snapshot::{self, ScreenSnapshot};
use crate::working_directory::{ReportedDirectory, WorkingDirectoryTracker};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
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

/// A cell in the visible area, zero-based from the top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportCell {
    pub row: usize,
    pub column: usize,
}

/// Which half of a cell the pointer is in; decides whether the cell is included at a selection edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// Character by character, wrapping across lines.
    Simple,
    /// Rectangular.
    Block,
    /// Whole words (double-click).
    Word,
    /// Whole lines (triple-click).
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollRequest {
    /// Positive scrolls toward older output.
    Lines(i32),
    PageUp,
    PageDown,
    Top,
    Bottom,
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

fn config_for(scrollback_lines: usize, default_cursor: CursorStyle) -> Config {
    let shape = match default_cursor {
        CursorStyle::Underline => alacritty_terminal::vte::ansi::CursorShape::Underline,
        CursorStyle::Beam => alacritty_terminal::vte::ansi::CursorShape::Beam,
        CursorStyle::HollowBlock => alacritty_terminal::vte::ansi::CursorShape::HollowBlock,
        CursorStyle::Block => alacritty_terminal::vte::ansi::CursorShape::Block,
    };
    Config {
        scrolling_history: scrollback_lines,
        default_cursor_style: alacritty_terminal::vte::ansi::CursorStyle { shape, blinking: false },
        ..Config::default()
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
    directory: WorkingDirectoryTracker,
}

impl TerminalEmulator {
    pub fn new(size: TerminalSize, scrollback_lines: usize) -> Self {
        Self::with_cursor(size, scrollback_lines, CursorStyle::Block)
    }

    /// `default_cursor` is the shape shown until a program asks for another (DECSCUSR); `Block` is the usual default.
    pub fn with_cursor(size: TerminalSize, scrollback_lines: usize, default_cursor: CursorStyle) -> Self {
        let (sender, events) = mpsc::channel();
        let config = config_for(scrollback_lines, default_cursor);
        let term = Term::new(config, &GridDimensions::from(size), EventForwarder { sender });
        Self { term, processor: Processor::new(), events, directory: WorkingDirectoryTracker::default() }
    }

    /// The folder the shell last reported (OSC 7 or OSC 9;9), if it ever did.
    pub fn current_directory(&self) -> Option<ReportedDirectory> {
        self.directory.current().cloned()
    }

    /// Changes the profile-driven defaults of a running terminal (switching profile): how many lines of scrollback are kept (lines beyond a
    /// smaller limit are dropped) and the cursor shape shown until a program asks for another. What is on screen stays.
    pub fn apply_defaults(&mut self, scrollback_lines: usize, default_cursor: CursorStyle) {
        self.term.set_options(config_for(scrollback_lines, default_cursor));
    }

    /// Drops all scrollback lines; the screen and the cursor stay.
    pub fn clear_scrollback(&mut self) {
        self.term.grid_mut().clear_history();
    }

    /// Full reset (RIS): clears the screen and scrollback and puts every terminal mode back to its default. The shell is not told, so its
    /// prompt comes back at its next output.
    pub fn reset_and_clear(&mut self) -> Vec<TerminalEvent> {
        self.advance(b"\x1bc")
    }

    /// Feeds shell output through the VT parser and returns the host-visible events it produced.
    pub fn advance(&mut self, bytes: &[u8]) -> Vec<TerminalEvent> {
        self.directory.feed(bytes);
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

    /// Mode flags that change how input must be encoded.
    pub fn input_modes(&self) -> InputModes {
        let mode = *self.term.mode();
        let mouse_reporting = if mode.contains(TermMode::MOUSE_MOTION) {
            MouseReporting::Motion
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            MouseReporting::Drag
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            MouseReporting::Click
        } else {
            MouseReporting::Off
        };
        InputModes {
            application_cursor_keys: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            mouse_reporting,
            sgr_mouse: mode.contains(TermMode::SGR_MOUSE),
            focus_reporting: mode.contains(TermMode::FOCUS_IN_OUT),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
            alternate_screen: mode.contains(TermMode::ALT_SCREEN),
        }
    }

    pub fn scroll_view(&mut self, request: ScrollRequest) {
        self.term.scroll_display(match request {
            ScrollRequest::Lines(lines) => Scroll::Delta(lines),
            ScrollRequest::PageUp => Scroll::PageUp,
            ScrollRequest::PageDown => Scroll::PageDown,
            ScrollRequest::Top => Scroll::Top,
            ScrollRequest::Bottom => Scroll::Bottom,
        });
    }

    /// Lines the view is scrolled back from the live screen.
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn start_selection(&mut self, kind: SelectionKind, cell: ViewportCell, side: CellSide) {
        let selection_type = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Block => SelectionType::Block,
            SelectionKind::Word => SelectionType::Semantic,
            SelectionKind::Line => SelectionType::Lines,
        };
        self.term.selection = Some(Selection::new(selection_type, self.grid_point(cell), grid_side(side)));
    }

    pub fn update_selection(&mut self, cell: ViewportCell, side: CellSide) {
        let point = self.grid_point(cell);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, grid_side(side));
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn has_selection(&self) -> bool {
        self.term.selection.as_ref().is_some_and(|selection| !selection.is_empty())
    }

    /// Selected text, or `None` when nothing is selected.
    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|text| !text.is_empty())
    }

    /// What is on screen now, including scrollback position, cursor and selection.
    pub fn snapshot(&self) -> ScreenSnapshot {
        snapshot::capture(&self.term, self.columns(), self.rows(), self.scrollback_line_count())
    }

    fn grid_point(&self, cell: ViewportCell) -> Point {
        let row = cell.row.min(self.rows().saturating_sub(1)) as i32;
        let column = cell.column.min(self.columns().saturating_sub(1));
        Point::new(Line(row - self.display_offset() as i32), Column(column))
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

fn grid_side(side: CellSide) -> Side {
    match side {
        CellSide::Left => Side::Left,
        CellSide::Right => Side::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{CellColor, CursorStyle, SnapshotCell, UnderlineStyle};

    fn emulator(columns: u16, rows: u16) -> TerminalEmulator {
        TerminalEmulator::new(TerminalSize::new(columns, rows), 100)
    }

    #[test]
    fn given_a_profile_cursor_shape_when_nothing_overrides_it_then_the_snapshot_uses_it() {
        let terminal = TerminalEmulator::with_cursor(TerminalSize::new(10, 3), 100, CursorStyle::Underline);
        assert_eq!(terminal.snapshot().cursor.map(|cursor| cursor.style), Some(CursorStyle::Underline));
        let beam = TerminalEmulator::with_cursor(TerminalSize::new(10, 3), 100, CursorStyle::Beam);
        assert_eq!(beam.snapshot().cursor.map(|cursor| cursor.style), Some(CursorStyle::Beam));
    }

    #[test]
    fn given_a_program_that_sets_the_cursor_when_it_runs_then_the_profile_default_is_overridden() {
        let mut terminal = TerminalEmulator::with_cursor(TerminalSize::new(10, 3), 100, CursorStyle::Underline);
        terminal.advance(b"\x1b[2 q");
        assert_eq!(terminal.snapshot().cursor.map(|cursor| cursor.style), Some(CursorStyle::Block));
    }

    #[test]
    fn given_a_small_scrollback_when_more_lines_scroll_off_then_only_that_many_are_kept() {
        let mut terminal = TerminalEmulator::new(TerminalSize::new(10, 2), 3);
        terminal.advance(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n");
        assert_eq!(terminal.scrollback_line_count(), 3);
    }

    #[test]
    fn given_scrollback_when_clearing_it_then_the_history_goes_and_the_screen_stays() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"a\r\nb\r\nc\r\nd");
        terminal.scroll_view(ScrollRequest::Top);
        terminal.clear_scrollback();
        assert_eq!(terminal.scrollback_line_count(), 0);
        assert_eq!(terminal.display_offset(), 0);
        assert_eq!(terminal.screen_text(), "c\nd");
    }

    #[test]
    fn given_scrollback_and_screen_text_when_resetting_then_both_are_empty_and_the_cursor_is_home() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"a\r\nb\r\nc\r\nd");
        terminal.reset_and_clear();
        assert_eq!(terminal.scrollback_line_count(), 0);
        assert_eq!(terminal.screen_text(), "");
        assert_eq!(terminal.cursor_position(), CursorPosition { row: 0, column: 0 });
    }

    #[test]
    fn given_a_program_changed_modes_when_resetting_then_modes_return_to_their_defaults() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"\x1b[?1000h");
        assert_ne!(terminal.input_modes().mouse_reporting, MouseReporting::Off);
        terminal.reset_and_clear();
        assert_eq!(terminal.input_modes().mouse_reporting, MouseReporting::Off);
    }

    #[test]
    fn given_a_running_terminal_when_the_scrollback_limit_shrinks_then_older_lines_are_dropped() {
        let mut terminal = TerminalEmulator::new(TerminalSize::new(10, 2), 100);
        terminal.advance(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n");
        assert_eq!(terminal.scrollback_line_count(), 6);
        terminal.apply_defaults(3, CursorStyle::Block);
        assert_eq!(terminal.scrollback_line_count(), 3);
    }

    #[test]
    fn given_a_running_terminal_when_the_default_cursor_changes_then_the_snapshot_uses_it() {
        let mut terminal = emulator(10, 2);
        terminal.apply_defaults(100, CursorStyle::Beam);
        assert_eq!(terminal.snapshot().cursor.map(|cursor| cursor.style), Some(CursorStyle::Beam));
    }

    #[test]
    fn given_a_program_chose_a_cursor_when_defaults_change_then_the_program_choice_stays() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"\x1b[4 q");
        terminal.apply_defaults(100, CursorStyle::Beam);
        assert_eq!(terminal.snapshot().cursor.map(|cursor| cursor.style), Some(CursorStyle::Underline));
    }

    #[test]
    fn given_a_folder_report_when_advancing_then_the_emulator_remembers_it_and_shows_no_text() {
        let mut terminal = emulator(30, 2);
        terminal.advance(b"\x1b]9;9;C:\\work\x07$ ");
        assert_eq!(
            terminal.current_directory(),
            Some(ReportedDirectory::Windows(std::path::PathBuf::from(r"C:\work")))
        );
        assert_eq!(terminal.row_text(0), "$");
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

    fn cell(row: usize, column: usize) -> ViewportCell {
        ViewportCell { row, column }
    }

    #[test]
    fn given_mode_switching_sequences_when_advancing_then_input_modes_follow() {
        let mut terminal = emulator(20, 5);
        assert_eq!(
            terminal.input_modes(),
            InputModes { alternate_scroll: true, ..InputModes::default() },
            "alternate scroll is on by default"
        );

        terminal.advance(b"\x1b[?1h\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1004h");
        let modes = terminal.input_modes();
        assert!(modes.application_cursor_keys && modes.bracketed_paste && modes.sgr_mouse && modes.focus_reporting);
        assert_eq!(modes.mouse_reporting, MouseReporting::Click);

        terminal.advance(b"\x1b[?1002h");
        assert_eq!(terminal.input_modes().mouse_reporting, MouseReporting::Drag);
        terminal.advance(b"\x1b[?1003h");
        assert_eq!(terminal.input_modes().mouse_reporting, MouseReporting::Motion);
        terminal.advance(b"\x1b[?1049h");
        assert!(terminal.input_modes().alternate_screen);
    }

    #[test]
    fn given_scrollback_when_scrolling_the_view_then_display_offset_and_snapshot_move() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"one\r\ntwo\r\nthree\r\nfour");
        assert_eq!(terminal.display_offset(), 0);

        terminal.scroll_view(ScrollRequest::Lines(2));
        let snapshot = terminal.snapshot();

        assert_eq!(terminal.display_offset(), 2);
        assert_eq!(snapshot.display_offset, 2);
        assert_eq!(snapshot.cell(0, 0).character, 'o');
        assert_eq!(snapshot.cell(1, 0).character, 't');
        assert!(snapshot.cursor.is_none(), "cursor is off screen while scrolled back");

        terminal.scroll_view(ScrollRequest::Bottom);
        assert_eq!(terminal.display_offset(), 0);
        terminal.scroll_view(ScrollRequest::Top);
        assert_eq!(terminal.display_offset(), 2);
    }

    #[test]
    fn given_simple_selection_when_dragging_across_text_then_selected_text_is_returned() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"hello world");
        terminal.start_selection(SelectionKind::Simple, cell(0, 0), CellSide::Left);
        terminal.update_selection(cell(0, 4), CellSide::Right);
        assert!(terminal.has_selection());
        assert_eq!(terminal.selection_text(), Some("hello".to_owned()));
    }

    #[test]
    fn given_selection_when_cleared_then_nothing_is_selected() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"hello");
        terminal.start_selection(SelectionKind::Simple, cell(0, 0), CellSide::Left);
        terminal.update_selection(cell(0, 2), CellSide::Right);
        terminal.clear_selection();
        assert!(!terminal.has_selection());
        assert_eq!(terminal.selection_text(), None);
    }

    #[test]
    fn given_word_selection_when_double_clicking_inside_a_word_then_the_whole_word_is_selected() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"hello world");
        terminal.start_selection(SelectionKind::Word, cell(0, 7), CellSide::Left);
        assert_eq!(terminal.selection_text(), Some("world".to_owned()));
    }

    #[test]
    fn given_line_selection_when_triple_clicking_then_the_whole_line_is_selected() {
        let mut terminal = emulator(20, 3);
        terminal.advance(b"first line\r\nsecond");
        terminal.start_selection(SelectionKind::Line, cell(0, 3), CellSide::Left);
        assert_eq!(terminal.selection_text().map(|text| text.trim_end().to_owned()), Some("first line".to_owned()));
    }

    #[test]
    fn given_block_selection_when_dragging_a_rectangle_then_only_the_rectangle_is_returned() {
        let mut terminal = emulator(20, 4);
        terminal.advance(b"abcdef\r\nghijkl\r\nmnopqr");
        terminal.start_selection(SelectionKind::Block, cell(0, 1), CellSide::Left);
        terminal.update_selection(cell(2, 3), CellSide::Right);
        assert_eq!(terminal.selection_text(), Some("bcd\nhij\nnop".to_owned()));
    }

    #[test]
    fn given_scrolled_view_when_selecting_then_viewport_cells_map_to_the_visible_lines() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"one\r\ntwo\r\nthree\r\nfour");
        terminal.scroll_view(ScrollRequest::Lines(2));
        terminal.start_selection(SelectionKind::Simple, cell(0, 0), CellSide::Left);
        terminal.update_selection(cell(0, 2), CellSide::Right);
        assert_eq!(terminal.selection_text(), Some("one".to_owned()));
    }

    #[test]
    fn given_out_of_range_cell_when_selecting_then_it_is_clamped_to_the_screen() {
        let mut terminal = emulator(5, 2);
        terminal.advance(b"abcde");
        terminal.start_selection(SelectionKind::Simple, cell(0, 0), CellSide::Left);
        terminal.update_selection(cell(99, 99), CellSide::Right);
        assert!(terminal.selection_text().is_some());
    }

    #[test]
    fn given_selection_when_snapshotting_then_selected_cells_are_flagged() {
        let mut terminal = emulator(10, 2);
        terminal.advance(b"abcd");
        terminal.start_selection(SelectionKind::Simple, cell(0, 1), CellSide::Left);
        terminal.update_selection(cell(0, 2), CellSide::Right);
        let snapshot = terminal.snapshot();
        let selected: Vec<bool> = (0..4).map(|column| snapshot.cell(0, column).selected).collect();
        assert_eq!(selected, vec![false, true, true, false]);
    }

    #[test]
    fn given_colors_and_attributes_when_snapshotting_then_cells_carry_them() {
        let mut terminal = emulator(30, 3);
        terminal.advance(b"\x1b[31mR\x1b[38;5;200mP\x1b[38;2;1;2;3mT\x1b[0;1;3;4mB\x1b[0;7mI\x1b[0;44mG\x1b[0m.");
        let snapshot = terminal.snapshot();

        assert_eq!(snapshot.cell(0, 0).foreground, CellColor::Indexed(1));
        assert_eq!(snapshot.cell(0, 1).foreground, CellColor::Indexed(200));
        assert_eq!(snapshot.cell(0, 2).foreground, CellColor::Rgb { red: 1, green: 2, blue: 3 });
        let styled = snapshot.cell(0, 3).attributes;
        assert!(styled.bold && styled.italic);
        assert_eq!(styled.underline, UnderlineStyle::Single);
        assert!(snapshot.cell(0, 4).attributes.inverse);
        assert_eq!(snapshot.cell(0, 5).background, CellColor::Indexed(4));
        assert_eq!(snapshot.cell(0, 6).foreground, CellColor::DefaultForeground);
        assert_eq!(snapshot.cell(0, 6).background, CellColor::DefaultBackground);
        assert_eq!(snapshot.cell(1, 0), &SnapshotCell::BLANK);
    }

    #[test]
    fn given_wide_character_when_snapshotting_then_first_cell_is_wide_and_second_is_a_spacer() {
        let mut terminal = emulator(10, 2);
        terminal.advance("世a".as_bytes());
        let snapshot = terminal.snapshot();
        assert_eq!(snapshot.cell(0, 0).character, '世');
        assert!(snapshot.cell(0, 0).attributes.wide);
        assert!(snapshot.cell(0, 1).attributes.wide_spacer);
        assert_eq!(snapshot.cell(0, 2).character, 'a');
    }

    #[test]
    fn given_cursor_visibility_and_style_sequences_when_snapshotting_then_cursor_reflects_them() {
        let mut terminal = emulator(10, 3);
        terminal.advance(b"ab");
        let cursor = terminal.snapshot().cursor.expect("cursor visible");
        assert_eq!((cursor.row, cursor.column, cursor.style), (0, 2, CursorStyle::Block));

        terminal.advance(b"\x1b[6 q");
        assert_eq!(terminal.snapshot().cursor.unwrap().style, CursorStyle::Beam);
        terminal.advance(b"\x1b[4 q");
        assert_eq!(terminal.snapshot().cursor.unwrap().style, CursorStyle::Underline);

        terminal.advance(b"\x1b[?25l");
        assert_eq!(terminal.snapshot().cursor, None);
    }

    #[test]
    fn given_snapshot_when_reading_rows_then_row_slices_match_the_grid_dimensions() {
        let terminal = emulator(7, 3);
        let snapshot = terminal.snapshot();
        assert_eq!((snapshot.columns, snapshot.rows, snapshot.cells.len()), (7, 3, 21));
        assert_eq!(snapshot.row(2).len(), 7);
    }
}
