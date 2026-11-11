//! Drop-down window logic: geometry, slide animation, toggle rules, and the controller state machine. Ports Yakuake `mainwindow.cpp`.

pub mod controller;
pub mod geometry;
pub mod slide;
pub mod toggle;

pub use controller::{DropDownCommand, DropDownController, DropDownEvent, DropDownState, WindowContext};
pub use geometry::{
    Monitor, PercentStep, PlacementRequest, Point, Rect, order_monitors, select_screen, stepped_percent, window_rect,
};
pub use slide::{FRAME_INTERVAL, SlideDirection, SlideFrame};
pub use toggle::{ToggleAction, ToggleInputs, decide_toggle};
