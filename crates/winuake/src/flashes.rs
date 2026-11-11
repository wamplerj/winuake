//! Short-lived overlays (blocked-input indicator, activation highlight): added to a session's grid, removed by one shared timer.

use crate::bindings::{Grid, UIElement};
use crate::window_bindings::{KillTimer, SetTimer};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

const CHECK_INTERVAL_MS: u32 = 40;

struct Flash {
    grid: Grid,
    element: UIElement,
    until: Instant,
}

thread_local! {
    static ACTIVE: RefCell<Vec<Flash>> = const { RefCell::new(Vec::new()) };
    static TIMER: Cell<usize> = const { Cell::new(0) };
}

/// Shows `element` (already in `grid`) for `duration_ms`, then removes it.
pub fn track(grid: Grid, element: UIElement, duration_ms: u32) {
    ACTIVE.with(|active| {
        active.borrow_mut().push(Flash {
            grid,
            element,
            until: Instant::now() + Duration::from_millis(u64::from(duration_ms)),
        })
    });
    if TIMER.with(Cell::get) == 0 {
        let timer = unsafe { SetTimer(None, 0, CHECK_INTERVAL_MS, Some(timer_procedure)) };
        TIMER.with(|slot| slot.set(timer));
    }
}

unsafe extern "system" fn timer_procedure(
    _window: crate::window_bindings::HWND,
    _message: u32,
    _timer: usize,
    _tick: u32,
) {
    let now = Instant::now();
    let finished: Vec<Flash> = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let (done, keep): (Vec<Flash>, Vec<Flash>) = active.drain(..).partition(|flash| flash.until <= now);
        *active = keep;
        done
    });
    for flash in finished {
        if let Ok(children) = flash.grid.Children() {
            let mut index = 0;
            if children.IndexOf(&flash.element, &mut index).unwrap_or(false) {
                let _ = children.RemoveAt(index);
            }
        }
    }
    if ACTIVE.with(|active| active.borrow().is_empty()) {
        let timer = TIMER.with(Cell::take);
        if timer != 0 {
            let _ = unsafe { KillTimer(None, timer) };
        }
    }
}
