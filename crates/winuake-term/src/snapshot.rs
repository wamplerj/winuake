//! A copy of what is on screen right now, in a form a renderer can consume without touching the emulator.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    DefaultForeground,
    DefaultBackground,
    /// 0..=15 are the ANSI colors, 16..=255 the extended palette.
    Indexed(u8),
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellAttributes {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub hidden: bool,
    /// First cell of a double-width character.
    pub wide: bool,
    /// Second cell of a double-width character; draws nothing of its own.
    pub wide_spacer: bool,
    pub underline: UnderlineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCell {
    pub character: char,
    pub foreground: CellColor,
    pub background: CellColor,
    pub attributes: CellAttributes,
    pub selected: bool,
}

impl SnapshotCell {
    pub const BLANK: Self = Self {
        character: ' ',
        foreground: CellColor::DefaultForeground,
        background: CellColor::DefaultBackground,
        attributes: CellAttributes {
            bold: false,
            dim: false,
            italic: false,
            strikethrough: false,
            inverse: false,
            hidden: false,
            wide: false,
            wide_spacer: false,
            underline: UnderlineStyle::None,
        },
        selected: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
    HollowBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCursor {
    pub row: usize,
    pub column: usize,
    pub style: CursorStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSnapshot {
    pub columns: usize,
    pub rows: usize,
    /// Row-major, `rows * columns` cells.
    pub cells: Vec<SnapshotCell>,
    /// `None` when the application hid the cursor or it is scrolled out of view.
    pub cursor: Option<SnapshotCursor>,
    /// How many lines the view is scrolled back from the live screen (0 = live).
    pub display_offset: usize,
    pub history_lines: usize,
}

impl ScreenSnapshot {
    pub fn cell(&self, row: usize, column: usize) -> &SnapshotCell {
        &self.cells[row * self.columns + column]
    }

    pub fn row(&self, row: usize) -> &[SnapshotCell] {
        &self.cells[row * self.columns..(row + 1) * self.columns]
    }
}

pub(crate) fn capture<T: EventListener>(
    term: &Term<T>,
    columns: usize,
    rows: usize,
    history_lines: usize,
) -> ScreenSnapshot {
    let content = term.renderable_content();
    let display_offset = content.display_offset;
    let selection = content.selection;
    let mut cells = vec![SnapshotCell::BLANK; columns * rows];

    for indexed in content.display_iter {
        let visible_row = indexed.point.line.0 + display_offset as i32;
        let column = indexed.point.column.0;
        if visible_row < 0 || visible_row as usize >= rows || column >= columns {
            continue;
        }
        let cell = &*indexed;
        cells[visible_row as usize * columns + column] = SnapshotCell {
            character: cell.c,
            foreground: cell_color(cell.fg),
            background: cell_color(cell.bg),
            attributes: attributes(cell.flags),
            selected: selection.is_some_and(|range| range.contains(indexed.point)),
        };
    }

    let cursor_row = content.cursor.point.line.0 + display_offset as i32;
    let cursor = (cursor_row >= 0 && (cursor_row as usize) < rows)
        .then(|| cursor_style(content.cursor.shape))
        .flatten()
        .map(|style| SnapshotCursor { row: cursor_row as usize, column: content.cursor.point.column.0, style });

    ScreenSnapshot { columns, rows, cells, cursor, display_offset, history_lines }
}

fn cursor_style(shape: CursorShape) -> Option<CursorStyle> {
    match shape {
        CursorShape::Block => Some(CursorStyle::Block),
        CursorShape::Underline => Some(CursorStyle::Underline),
        CursorShape::Beam => Some(CursorStyle::Beam),
        CursorShape::HollowBlock => Some(CursorStyle::HollowBlock),
        CursorShape::Hidden => None,
    }
}

fn cell_color(color: Color) -> CellColor {
    match color {
        Color::Spec(rgb) => CellColor::Rgb { red: rgb.r, green: rgb.g, blue: rgb.b },
        Color::Indexed(index) => CellColor::Indexed(index),
        Color::Named(NamedColor::Background) => CellColor::DefaultBackground,
        Color::Named(
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground | NamedColor::Cursor,
        ) => CellColor::DefaultForeground,
        Color::Named(named) => {
            let index = named as usize;
            if index < 16 {
                CellColor::Indexed(index as u8)
            } else {
                // Dim variants of the eight base colors.
                CellColor::Indexed(index.saturating_sub(NamedColor::DimBlack as usize).min(7) as u8)
            }
        }
    }
}

fn attributes(flags: Flags) -> CellAttributes {
    let underline = if flags.contains(Flags::UNDERCURL) {
        UnderlineStyle::Curly
    } else if flags.contains(Flags::DOUBLE_UNDERLINE) {
        UnderlineStyle::Double
    } else if flags.contains(Flags::DOTTED_UNDERLINE) {
        UnderlineStyle::Dotted
    } else if flags.contains(Flags::DASHED_UNDERLINE) {
        UnderlineStyle::Dashed
    } else if flags.contains(Flags::UNDERLINE) {
        UnderlineStyle::Single
    } else {
        UnderlineStyle::None
    };
    CellAttributes {
        bold: flags.contains(Flags::BOLD),
        dim: flags.contains(Flags::DIM),
        italic: flags.contains(Flags::ITALIC),
        strikethrough: flags.contains(Flags::STRIKEOUT),
        inverse: flags.contains(Flags::INVERSE),
        hidden: flags.contains(Flags::HIDDEN),
        wide: flags.contains(Flags::WIDE_CHAR),
        wide_spacer: flags.contains(Flags::WIDE_CHAR_SPACER),
        underline,
    }
}
