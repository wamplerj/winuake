//! Where the drop-down window goes. Ports Yakuake `setWindowGeometry`, `getScreen` and the width/height step actions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Screen rectangle in physical pixels; `right` and `bottom` are exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self { left, top, right, bottom }
    }

    pub const fn from_origin_size(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { left: x, top: y, right: x + width, bottom: y + height }
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monitor {
    /// Whole screen.
    pub bounds: Rect,
    /// Screen minus the taskbar and other docked bars.
    pub work_area: Rect,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementRequest {
    pub width_percent: u8,
    pub height_percent: u8,
    pub position_percent: u8,
    pub fullscreen: bool,
}

/// Window rectangle on `monitor`. Integer arithmetic exactly as in Yakuake so sizes match pixel for pixel.
/// Full screen covers the whole monitor (taskbar included); otherwise the window hangs from the top of the work area.
pub fn window_rect(monitor: &Monitor, request: PlacementRequest) -> Rect {
    if request.fullscreen {
        return monitor.bounds;
    }
    let area = monitor.work_area;
    let width_percent = i32::from(request.width_percent);
    let height = area.height() * i32::from(request.height_percent) / 100;
    let width = area.width() * width_percent / 100;
    let x = area.left + area.width() * i32::from(request.position_percent) * (100 - width_percent) / 10_000;
    Rect::from_origin_size(x, area.top, width, height)
}

/// Primary monitor first, then the others left to right (top to bottom on ties). This is what `Screen = N` counts over.
pub fn order_monitors(mut monitors: Vec<Monitor>) -> Vec<Monitor> {
    monitors.sort_by_key(|monitor| (!monitor.is_primary, monitor.bounds.left, monitor.bounds.top));
    monitors
}

/// Index into `monitors` (already ordered). `Screen = 0`, or a number past the last screen, means the screen under the pointer.
/// Window height percent for a title-bar drag: the pointer's distance below the top of the work area as a share of its height, kept
/// within 10..=100 (Ported from `TitleBar::mouseMoveEvent`).
pub fn height_percent_for_pointer(pointer_y: i32, work_area: Rect) -> u8 {
    let height = (work_area.bottom - work_area.top).max(1);
    let percent = i64::from(pointer_y - work_area.top) * 100 / i64::from(height);
    percent.clamp(10, 100) as u8
}

pub fn select_screen(screen_setting: u32, monitors: &[Monitor], pointer: Point) -> usize {
    if screen_setting >= 1 && (screen_setting as usize) <= monitors.len() {
        return screen_setting as usize - 1;
    }
    monitors.iter().position(|monitor| monitor.bounds.contains(pointer)).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentStep {
    Increase,
    Decrease,
}

/// Width/height step actions: +/-10 points, refused at the ends (increase only up to 100, decrease only down to 10).
pub fn stepped_percent(current: u8, step: PercentStep) -> u8 {
    match step {
        PercentStep::Increase if current <= 90 => current + 10,
        PercentStep::Decrease if current >= 20 => current - 10,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(left: i32, top: i32, width: i32, height: i32, taskbar: i32, primary: bool) -> Monitor {
        let bounds = Rect::from_origin_size(left, top, width, height);
        Monitor { bounds, work_area: Rect::new(left, top, left + width, top + height - taskbar), is_primary: primary }
    }

    fn request(width: u8, height: u8, position: u8) -> PlacementRequest {
        PlacementRequest { width_percent: width, height_percent: height, position_percent: position, fullscreen: false }
    }

    #[test]
    fn given_default_percentages_when_placing_on_a_1080p_screen_then_the_window_is_centered_at_the_top_of_the_work_area()
     {
        let screen = monitor(0, 0, 1920, 1080, 48, true);
        let rect = window_rect(&screen, request(90, 50, 50));
        assert_eq!(rect, Rect::from_origin_size(96, 0, 1728, 516));
    }

    #[test]
    fn given_position_zero_and_hundred_when_placing_then_the_window_hugs_the_left_and_right_edges() {
        let screen = monitor(0, 0, 2000, 1000, 0, true);
        assert_eq!(window_rect(&screen, request(50, 50, 0)).left, 0);
        assert_eq!(window_rect(&screen, request(50, 50, 100)).right, 2000);
    }

    #[test]
    fn given_a_full_width_window_when_placing_then_position_has_no_effect() {
        let screen = monitor(0, 0, 1920, 1080, 0, true);
        assert_eq!(window_rect(&screen, request(100, 50, 0)), window_rect(&screen, request(100, 50, 100)));
    }

    #[test]
    fn given_a_secondary_monitor_with_an_offset_when_placing_then_coordinates_are_in_virtual_desktop_space() {
        let screen = monitor(1920, -200, 2560, 1440, 40, false);
        let rect = window_rect(&screen, request(50, 50, 50));
        assert_eq!(rect.left, 1920 + 2560 * 50 * 50 / 10_000);
        assert_eq!(rect.top, -200);
        assert_eq!((rect.width(), rect.height()), (1280, 700));
    }

    #[test]
    fn given_a_top_taskbar_when_placing_then_the_window_starts_below_it() {
        let mut screen = monitor(0, 0, 1920, 1080, 0, true);
        screen.work_area = Rect::new(0, 40, 1920, 1080);
        assert_eq!(window_rect(&screen, request(90, 50, 50)).top, 40);
    }

    #[test]
    fn given_fullscreen_when_placing_then_the_whole_monitor_including_the_taskbar_is_covered() {
        let screen = monitor(0, 0, 1920, 1080, 48, true);
        let rect = window_rect(&screen, PlacementRequest { fullscreen: true, ..request(50, 50, 50) });
        assert_eq!(rect, screen.bounds);
    }

    #[test]
    fn given_several_monitors_when_ordering_then_primary_comes_first_then_left_to_right() {
        let left = monitor(-1920, 0, 1920, 1080, 0, false);
        let primary = monitor(0, 0, 2560, 1440, 0, true);
        let right = monitor(2560, 0, 1920, 1080, 0, false);
        let ordered = order_monitors(vec![right, left, primary]);
        assert_eq!(ordered, vec![primary, left, right]);
    }

    #[test]
    fn given_screen_zero_when_selecting_then_the_monitor_under_the_pointer_is_used() {
        let monitors = [monitor(0, 0, 1920, 1080, 0, true), monitor(1920, 0, 1920, 1080, 0, false)];
        assert_eq!(select_screen(0, &monitors, Point { x: 2500, y: 100 }), 1);
        assert_eq!(select_screen(0, &monitors, Point { x: 10, y: 10 }), 0);
    }

    #[test]
    fn given_a_screen_number_when_selecting_then_that_monitor_is_used_regardless_of_the_pointer() {
        let monitors = [monitor(0, 0, 1920, 1080, 0, true), monitor(1920, 0, 1920, 1080, 0, false)];
        assert_eq!(select_screen(2, &monitors, Point { x: 10, y: 10 }), 1);
        assert_eq!(select_screen(1, &monitors, Point { x: 2500, y: 10 }), 0);
    }

    #[test]
    fn given_a_screen_number_past_the_last_monitor_when_selecting_then_it_falls_back_to_the_pointer() {
        let monitors = [monitor(0, 0, 1920, 1080, 0, true), monitor(1920, 0, 1920, 1080, 0, false)];
        assert_eq!(select_screen(5, &monitors, Point { x: 2000, y: 10 }), 1);
    }

    #[test]
    fn given_a_pointer_outside_every_monitor_when_selecting_then_the_first_monitor_is_used() {
        let monitors = [monitor(0, 0, 1920, 1080, 0, true)];
        assert_eq!(select_screen(0, &monitors, Point { x: -5000, y: 0 }), 0);
    }

    #[test]
    fn given_width_steps_when_adjusting_then_they_move_by_ten_and_stop_at_the_limits() {
        assert_eq!(stepped_percent(50, PercentStep::Increase), 60);
        assert_eq!(stepped_percent(50, PercentStep::Decrease), 40);
        assert_eq!(stepped_percent(90, PercentStep::Increase), 100);
        assert_eq!(stepped_percent(100, PercentStep::Increase), 100);
        assert_eq!(stepped_percent(20, PercentStep::Decrease), 10);
        assert_eq!(stepped_percent(10, PercentStep::Decrease), 10);
    }

    #[test]
    fn given_a_pointer_in_the_work_area_when_dragging_the_title_bar_then_the_height_is_its_share() {
        let work = Rect::new(0, 40, 1000, 1040);
        assert_eq!(height_percent_for_pointer(540, work), 50);
        assert_eq!(height_percent_for_pointer(40 + 333, work), 33);
    }

    #[test]
    fn given_a_pointer_outside_the_work_area_when_dragging_then_the_height_is_clamped() {
        let work = Rect::new(0, 0, 1000, 1000);
        assert_eq!(height_percent_for_pointer(-50, work), 10);
        assert_eq!(height_percent_for_pointer(5000, work), 100);
    }
}
