//! Click, press-and-hold and right-click on a button: a click runs `on_click` when the button comes up, holding it (or a right click)
//! runs `on_menu`. The gesture rules live in `winuake_core::press_and_hold`; this wires them to pointer events and one timer.

use crate::bindings::UIElement;
use crate::window_bindings::{HWND, KillTimer, SetTimer};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows_core::Result;
use winuake_core::press_and_hold::{HoldOutcome, PressAndHold};

/// How long the button must stay down before the menu opens (about the Windows default for press and hold).
const HOLD_MILLISECONDS: u32 = 450;

struct Gesture {
    machine: RefCell<PressAndHold>,
    on_click: Rc<dyn Fn()>,
    on_menu: Rc<dyn Fn()>,
}

thread_local! {
    /// Handlers must be Send; the gestures live on the UI thread, so each handler reaches its gesture through an index into this list.
    static GESTURES: RefCell<Vec<Rc<Gesture>>> = const { RefCell::new(Vec::new()) };
    /// The gesture whose hold timer is running, if any.
    static TIMER_GESTURE: Cell<Option<usize>> = const { Cell::new(None) };
    static TIMER: Cell<usize> = const { Cell::new(0) };
}

fn gesture(index: usize) -> Option<Rc<Gesture>> {
    GESTURES.with(|gestures| gestures.borrow().get(index).cloned())
}

/// Forgets the gestures of a bar that was replaced. (Handlers on the old elements die with them.)
pub fn clear_registry() {
    cancel_timer();
    GESTURES.with(|gestures| gestures.borrow_mut().clear());
}

/// Wires `element` for click, hold and right click. `on_click` and `on_menu` run on the UI thread.
pub fn attach(element: &UIElement, on_click: impl Fn() + 'static, on_menu: impl Fn() + 'static) -> Result<()> {
    let index = GESTURES.with(|gestures| {
        let mut gestures = gestures.borrow_mut();
        gestures.push(Rc::new(Gesture {
            machine: RefCell::new(PressAndHold::default()),
            on_click: Rc::new(on_click),
            on_menu: Rc::new(on_menu),
        }));
        gestures.len() - 1
    });

    let pressed_target = element.clone();
    std::mem::forget(element.PointerPressed(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let _ = args.SetHandled(true);
        let Some(gesture) = gesture(index) else { return };
        if crate::skin_chrome::is_right_button(args, &pressed_target) {
            (gesture.on_menu)();
            return;
        }
        gesture.machine.borrow_mut().pressed();
        start_timer(index);
    })?);
    std::mem::forget(element.PointerReleased(move |_, args| {
        if let Some(args) = args.as_ref() {
            let _ = args.SetHandled(true);
        }
        cancel_timer();
        let Some(gesture) = gesture(index) else { return };
        let outcome = gesture.machine.borrow_mut().released();
        if outcome == HoldOutcome::Click {
            (gesture.on_click)();
        }
    })?);
    std::mem::forget(element.PointerExited(move |_, _| {
        cancel_timer();
        if let Some(gesture) = gesture(index) {
            gesture.machine.borrow_mut().cancelled();
        }
    })?);
    Ok(())
}

fn start_timer(index: usize) {
    cancel_timer();
    TIMER_GESTURE.with(|slot| slot.set(Some(index)));
    let timer = unsafe { SetTimer(None, 0, HOLD_MILLISECONDS, Some(timer_procedure)) };
    TIMER.with(|slot| slot.set(timer));
}

fn cancel_timer() {
    TIMER_GESTURE.with(Cell::take);
    let timer = TIMER.with(Cell::take);
    if timer != 0 {
        let _ = unsafe { KillTimer(None, timer) };
    }
}

unsafe extern "system" fn timer_procedure(_window: HWND, _message: u32, _timer: usize, _tick: u32) {
    let index = TIMER_GESTURE.with(Cell::get);
    cancel_timer();
    let Some(gesture) = index.and_then(gesture) else { return };
    // Release the borrow before the menu runs: it blocks until the menu closes.
    let outcome = gesture.machine.borrow_mut().hold_elapsed();
    if outcome == HoldOutcome::Hold {
        (gesture.on_menu)();
    }
}
