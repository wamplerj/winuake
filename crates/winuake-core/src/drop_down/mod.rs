//! Drop-down window logic: geometry, slide animation, toggle rules, and the controller state machine. Ports Yakuake `mainwindow.cpp`.

pub mod auto_open;
pub mod controller;
pub mod geometry;
pub mod slide;
pub mod toggle;

pub use auto_open::pointer_at_open_edge;
pub use controller::{DropDownCommand, DropDownController, DropDownEvent, DropDownState, WindowContext};
pub use geometry::{
    Monitor, PercentStep, PlacementRequest, Point, Rect, height_percent_for_pointer, order_monitors, select_screen,
    stepped_percent, window_rect,
};
pub use slide::{FRAME_INTERVAL, SlideDirection, SlideFrame};
pub use toggle::{ToggleAction, ToggleInputs, decide_toggle};
