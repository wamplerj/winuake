//! Mouse helpers that need no windowing system: pixel -> cell mapping, click counting, wheel accumulation.

use crate::emulator::{CellSide, SelectionKind, ViewportCell};
use std::time::{Duration, Instant};

pub const DEFAULT_MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);
/// Lines scrolled per wheel notch when not reporting to the application.
pub const LINES_PER_WHEEL_NOTCH: i32 = 3;
/// One wheel notch as reported by Windows (`WHEEL_DELTA`).
pub const WHEEL_DELTA: i32 = 120;

/// The cell under a pixel position (relative to the terminal area) and which half of it the pointer is in.
/// Positions outside the grid are clamped to the nearest edge cell.
pub fn cell_at_pixel(
    x: f32,
    y: f32,
    cell_width: f32,
    cell_height: f32,
    columns: usize,
    rows: usize,
) -> (ViewportCell, CellSide) {
    let column_position = (x / cell_width).max(0.0);
    let row_position = (y / cell_height).max(0.0);
    let column = (column_position.floor() as usize).min(columns.saturating_sub(1));
    let row = (row_position.floor() as usize).min(rows.saturating_sub(1));
    let side = if x < 0.0 || (column_position.fract() < 0.5 && column_position < columns as f32) {
        CellSide::Left
    } else {
        CellSide::Right
    };
    (ViewportCell { row, column }, side)
}

/// Counts consecutive clicks on the same cell: single, double, triple, then wraps back to single.
#[derive(Debug)]
pub struct ClickCounter {
    interval: Duration,
    last: Option<(Instant, ViewportCell)>,
    count: u8,
}

impl ClickCounter {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None, count: 0 }
    }

    pub fn register_click(&mut self, cell: ViewportCell, now: Instant) -> SelectionKind {
        let continues = self
            .last
            .is_some_and(|(time, last_cell)| last_cell == cell && now.saturating_duration_since(time) <= self.interval);
        self.count = if continues && self.count < 3 { self.count + 1 } else { 1 };
        self.last = Some((now, cell));
        match self.count {
            2 => SelectionKind::Word,
            3 => SelectionKind::Line,
            _ => SelectionKind::Simple,
        }
    }
}

impl Default for ClickCounter {
    fn default() -> Self {
        Self::new(DEFAULT_MULTI_CLICK_INTERVAL)
    }
}

/// Collects fine-grained wheel deltas (precision touchpads) into whole notches.
#[derive(Debug, Default)]
pub struct WheelAccumulator {
    remainder: i32,
}

impl WheelAccumulator {
    /// Adds a raw delta (positive = away from the user = scroll up) and returns whole notches to act on.
    pub fn add(&mut self, delta: i32) -> i32 {
        self.remainder += delta;
        let notches = self.remainder / WHEEL_DELTA;
        self.remainder -= notches * WHEEL_DELTA;
        notches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: usize, column: usize) -> ViewportCell {
        ViewportCell { row, column }
    }

    #[test]
    fn given_pixel_in_the_left_half_of_a_cell_when_mapping_then_the_cell_and_left_side_are_returned() {
        assert_eq!(cell_at_pixel(23.0, 41.0, 10.0, 20.0, 80, 24), (cell(2, 2), CellSide::Left));
    }

    #[test]
    fn given_pixel_in_the_right_half_of_a_cell_when_mapping_then_the_right_side_is_returned() {
        assert_eq!(cell_at_pixel(27.0, 41.0, 10.0, 20.0, 80, 24), (cell(2, 2), CellSide::Right));
    }

    #[test]
    fn given_pixel_beyond_the_grid_when_mapping_then_it_is_clamped_to_the_last_cell() {
        let (cell_result, side) = cell_at_pixel(5000.0, 5000.0, 10.0, 20.0, 80, 24);
        assert_eq!(cell_result, cell(23, 79));
        assert_eq!(side, CellSide::Right);
    }

    #[test]
    fn given_pixel_before_the_origin_when_mapping_then_it_is_clamped_to_the_first_cell() {
        assert_eq!(cell_at_pixel(-30.0, -5.0, 10.0, 20.0, 80, 24), (cell(0, 0), CellSide::Left));
    }

    #[test]
    fn given_quick_repeated_clicks_on_one_cell_when_counting_then_single_double_triple_then_single_again() {
        let mut counter = ClickCounter::default();
        let start = Instant::now();
        let step = Duration::from_millis(100);
        assert_eq!(counter.register_click(cell(1, 1), start), SelectionKind::Simple);
        assert_eq!(counter.register_click(cell(1, 1), start + step), SelectionKind::Word);
        assert_eq!(counter.register_click(cell(1, 1), start + 2 * step), SelectionKind::Line);
        assert_eq!(counter.register_click(cell(1, 1), start + 3 * step), SelectionKind::Simple);
    }

    #[test]
    fn given_a_slow_second_click_when_counting_then_it_starts_over() {
        let mut counter = ClickCounter::default();
        let start = Instant::now();
        counter.register_click(cell(1, 1), start);
        assert_eq!(counter.register_click(cell(1, 1), start + Duration::from_millis(900)), SelectionKind::Simple);
    }

    #[test]
    fn given_a_second_click_on_a_different_cell_when_counting_then_it_starts_over() {
        let mut counter = ClickCounter::default();
        let start = Instant::now();
        counter.register_click(cell(1, 1), start);
        assert_eq!(counter.register_click(cell(1, 2), start + Duration::from_millis(50)), SelectionKind::Simple);
    }

    #[test]
    fn given_small_wheel_deltas_when_accumulating_then_notches_appear_only_when_a_full_step_is_reached() {
        let mut wheel = WheelAccumulator::default();
        assert_eq!(wheel.add(40), 0);
        assert_eq!(wheel.add(40), 0);
        assert_eq!(wheel.add(40), 1);
        assert_eq!(wheel.add(-120), -1);
    }

    #[test]
    fn given_a_large_delta_when_accumulating_then_multiple_notches_are_returned_and_the_remainder_kept() {
        let mut wheel = WheelAccumulator::default();
        assert_eq!(wheel.add(300), 2);
        assert_eq!(wheel.add(60), 1);
    }
}
