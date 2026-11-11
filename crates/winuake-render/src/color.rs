//! Turns the emulator's symbolic cell colors into concrete RGB using a color scheme.

use winuake_term::snapshot::{CellColor, SnapshotCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// `self` weighted `self_share`/255, `other` the rest.
    fn blend(self, other: Self, self_share: u8) -> Self {
        let mix = |own: u8, theirs: u8| {
            ((u16::from(own) * u16::from(self_share) + u16::from(theirs) * u16::from(255 - self_share)) / 255) as u8
        };
        Self { red: mix(self.red, other.red), green: mix(self.green, other.green), blue: mix(self.blue, other.blue) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorScheme {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub selection_background: Rgb,
    /// `None` keeps each selected cell's own foreground.
    pub selection_foreground: Option<Rgb>,
    /// The 16 ANSI colors: 8 normal, then 8 bright.
    pub ansi: [Rgb; 16],
    /// Bold text in colors 0-7 is drawn with the bright variant.
    pub bold_uses_bright_colors: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCellColors {
    pub foreground: Rgb,
    pub background: Rgb,
}

impl ColorScheme {
    /// Windows Terminal's "Campbell".
    pub fn campbell() -> Self {
        Self {
            foreground: Rgb::new(0xcc, 0xcc, 0xcc),
            background: Rgb::new(0x0c, 0x0c, 0x0c),
            cursor: Rgb::new(0xff, 0xff, 0xff),
            selection_background: Rgb::new(0x26, 0x4f, 0x78),
            selection_foreground: None,
            ansi: [
                Rgb::new(0x0c, 0x0c, 0x0c),
                Rgb::new(0xc5, 0x0f, 0x1f),
                Rgb::new(0x13, 0xa1, 0x0e),
                Rgb::new(0xc1, 0x9c, 0x00),
                Rgb::new(0x00, 0x37, 0xda),
                Rgb::new(0x88, 0x17, 0x98),
                Rgb::new(0x3a, 0x96, 0xdd),
                Rgb::new(0xcc, 0xcc, 0xcc),
                Rgb::new(0x76, 0x76, 0x76),
                Rgb::new(0xe7, 0x48, 0x56),
                Rgb::new(0x16, 0xc6, 0x0c),
                Rgb::new(0xf9, 0xf1, 0xa5),
                Rgb::new(0x3b, 0x78, 0xff),
                Rgb::new(0xb4, 0x00, 0x9e),
                Rgb::new(0x61, 0xd6, 0xd6),
                Rgb::new(0xf2, 0xf2, 0xf2),
            ],
            bold_uses_bright_colors: true,
        }
    }

    pub fn resolve(&self, cell: &SnapshotCell) -> ResolvedCellColors {
        let attributes = cell.attributes;
        let mut foreground = self.resolve_color(cell.foreground, attributes.bold);
        let mut background = self.resolve_color(cell.background, false);

        if attributes.inverse {
            std::mem::swap(&mut foreground, &mut background);
        }
        if attributes.dim {
            // Two thirds foreground, one third background.
            foreground = foreground.blend(background, 170);
        }
        if attributes.hidden {
            foreground = background;
        }
        if cell.selected {
            background = self.selection_background;
            foreground = self.selection_foreground.unwrap_or(foreground);
        }
        ResolvedCellColors { foreground, background }
    }

    fn resolve_color(&self, color: CellColor, bold: bool) -> Rgb {
        match color {
            CellColor::DefaultForeground => self.foreground,
            CellColor::DefaultBackground => self.background,
            CellColor::Rgb { red, green, blue } => Rgb::new(red, green, blue),
            CellColor::Indexed(index) if index < 8 && bold && self.bold_uses_bright_colors => {
                self.ansi[usize::from(index) + 8]
            }
            CellColor::Indexed(index) if index < 16 => self.ansi[usize::from(index)],
            CellColor::Indexed(index) => extended_palette_color(index),
        }
    }
}

/// xterm's 6x6x6 color cube (16..=231) and 24-step grayscale ramp (232..=255).
fn extended_palette_color(index: u8) -> Rgb {
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if index >= 232 {
        let level = 8 + 10 * (index - 232);
        return Rgb::new(level, level, level);
    }
    let cube_index = index.saturating_sub(16);
    Rgb::new(
        CUBE_LEVELS[usize::from(cube_index / 36)],
        CUBE_LEVELS[usize::from(cube_index / 6 % 6)],
        CUBE_LEVELS[usize::from(cube_index % 6)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use winuake_term::snapshot::CellAttributes;

    fn scheme() -> ColorScheme {
        ColorScheme::campbell()
    }

    fn cell(foreground: CellColor, background: CellColor, attributes: CellAttributes) -> SnapshotCell {
        SnapshotCell { character: 'x', foreground, background, attributes, selected: false }
    }

    fn plain(foreground: CellColor) -> SnapshotCell {
        cell(foreground, CellColor::DefaultBackground, CellAttributes::default())
    }

    #[test]
    fn given_default_colors_when_resolving_then_scheme_foreground_and_background_are_used() {
        let resolved = scheme().resolve(&SnapshotCell::BLANK);
        assert_eq!(resolved, ResolvedCellColors { foreground: scheme().foreground, background: scheme().background });
    }

    #[test]
    fn given_ansi_index_when_resolving_then_the_palette_entry_is_used() {
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(1))).foreground, scheme().ansi[1]);
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(12))).foreground, scheme().ansi[12]);
    }

    #[test]
    fn given_bold_with_a_normal_ansi_color_when_resolving_then_the_bright_variant_is_used() {
        let bold = CellAttributes { bold: true, ..CellAttributes::default() };
        let resolved = scheme().resolve(&cell(CellColor::Indexed(1), CellColor::DefaultBackground, bold));
        assert_eq!(resolved.foreground, scheme().ansi[9]);
    }

    #[test]
    fn given_bold_but_bright_colors_disabled_when_resolving_then_the_normal_color_is_kept() {
        let mut scheme = scheme();
        scheme.bold_uses_bright_colors = false;
        let bold = CellAttributes { bold: true, ..CellAttributes::default() };
        assert_eq!(
            scheme.resolve(&cell(CellColor::Indexed(1), CellColor::DefaultBackground, bold)).foreground,
            scheme.ansi[1]
        );
    }

    #[test]
    fn given_color_cube_indexes_when_resolving_then_xterm_values_are_used() {
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(16))).foreground, Rgb::new(0, 0, 0));
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(196))).foreground, Rgb::new(255, 0, 0));
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(21))).foreground, Rgb::new(0, 0, 255));
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(231))).foreground, Rgb::new(255, 255, 255));
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(67))).foreground, Rgb::new(95, 135, 175));
    }

    #[test]
    fn given_grayscale_indexes_when_resolving_then_the_ramp_is_used() {
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(232))).foreground, Rgb::new(8, 8, 8));
        assert_eq!(scheme().resolve(&plain(CellColor::Indexed(255))).foreground, Rgb::new(238, 238, 238));
    }

    #[test]
    fn given_true_color_when_resolving_then_it_is_used_as_is() {
        assert_eq!(
            scheme().resolve(&plain(CellColor::Rgb { red: 1, green: 2, blue: 3 })).foreground,
            Rgb::new(1, 2, 3)
        );
    }

    #[test]
    fn given_inverse_when_resolving_then_foreground_and_background_swap() {
        let inverse = CellAttributes { inverse: true, ..CellAttributes::default() };
        let resolved = scheme().resolve(&cell(CellColor::Indexed(2), CellColor::Indexed(4), inverse));
        assert_eq!(resolved, ResolvedCellColors { foreground: scheme().ansi[4], background: scheme().ansi[2] });
    }

    #[test]
    fn given_dim_when_resolving_then_foreground_moves_toward_the_background() {
        let dim = CellAttributes { dim: true, ..CellAttributes::default() };
        let resolved = scheme().resolve(&cell(
            CellColor::Rgb { red: 255, green: 255, blue: 255 },
            CellColor::Rgb { red: 0, green: 0, blue: 0 },
            dim,
        ));
        assert_eq!(resolved.foreground, Rgb::new(170, 170, 170));
    }

    #[test]
    fn given_hidden_when_resolving_then_text_is_drawn_in_the_background_color() {
        let hidden = CellAttributes { hidden: true, ..CellAttributes::default() };
        let resolved = scheme().resolve(&cell(CellColor::Indexed(1), CellColor::DefaultBackground, hidden));
        assert_eq!(resolved.foreground, resolved.background);
    }

    #[test]
    fn given_selected_cell_when_resolving_then_selection_background_applies_and_foreground_is_kept_by_default() {
        let mut selected = plain(CellColor::Indexed(2));
        selected.selected = true;
        let resolved = scheme().resolve(&selected);
        assert_eq!(
            resolved,
            ResolvedCellColors { foreground: scheme().ansi[2], background: scheme().selection_background }
        );
    }

    #[test]
    fn given_selection_foreground_configured_when_resolving_a_selected_cell_then_it_overrides() {
        let mut scheme = scheme();
        scheme.selection_foreground = Some(Rgb::new(9, 9, 9));
        let mut selected = plain(CellColor::Indexed(2));
        selected.selected = true;
        assert_eq!(scheme.resolve(&selected).foreground, Rgb::new(9, 9, 9));
    }
}
