// Ported from yakuake app/mainwindow.cpp (pollMouse) @21c6210
//! Opening the window by pushing the pointer against the top edge of the screen.

use super::geometry::{Point, Rect};

/// True when the pointer is on the top row of the screen and horizontally within where the window would appear.
/// The span follows Yakuake: `x = work.x + work.w * position * (100 - width) / 10000`, `w = work.w * width / 100`.
pub fn pointer_at_open_edge(pointer: Point, work_area: Rect, width_percent: u8, position_percent: u8) -> bool {
    let work_width = i64::from(work_area.right - work_area.left);
    let left = i64::from(work_area.left)
        + work_width * i64::from(position_percent) * i64::from(100 - i32::from(width_percent)) / 10_000;
    let width = work_width * i64::from(width_percent) / 100;
    let pointer_x = i64::from(pointer.x);
    pointer.y == work_area.top && pointer_x >= left && pointer_x <= left + width
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect::new(0, 0, 1000, 800);

    #[test]
    fn given_the_pointer_on_the_top_edge_inside_a_centered_window_span_when_checking_then_it_opens() {
        assert!(pointer_at_open_edge(Point { x: 500, y: 0 }, WORK, 50, 50));
        assert!(pointer_at_open_edge(Point { x: 250, y: 0 }, WORK, 50, 50));
        assert!(pointer_at_open_edge(Point { x: 750, y: 0 }, WORK, 50, 50));
    }

    #[test]
    fn given_the_pointer_outside_the_span_or_off_the_edge_when_checking_then_it_does_not_open() {
        assert!(!pointer_at_open_edge(Point { x: 100, y: 0 }, WORK, 50, 50));
        assert!(!pointer_at_open_edge(Point { x: 900, y: 0 }, WORK, 50, 50));
        assert!(!pointer_at_open_edge(Point { x: 500, y: 1 }, WORK, 50, 50));
    }

    #[test]
    fn given_left_and_right_positions_when_checking_then_the_span_moves() {
        assert!(pointer_at_open_edge(Point { x: 10, y: 0 }, WORK, 50, 0));
        assert!(!pointer_at_open_edge(Point { x: 900, y: 0 }, WORK, 50, 0));
        assert!(pointer_at_open_edge(Point { x: 990, y: 0 }, WORK, 50, 100));
    }

    #[test]
    fn given_a_work_area_below_the_screen_top_when_checking_then_its_own_top_row_counts() {
        let work = Rect::new(0, 40, 1000, 840);
        assert!(pointer_at_open_edge(Point { x: 500, y: 40 }, work, 100, 50));
        assert!(!pointer_at_open_edge(Point { x: 500, y: 0 }, work, 100, 50));
    }
}
