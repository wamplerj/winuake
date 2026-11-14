//! Click versus press-and-hold on a button (the new-session button: click starts a session, holding offers the profile menu).
//! Pure state; the caller owns the pointer events and the timer.

/// How a gesture on the button ended or advanced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldOutcome {
    /// The button was pressed and released before the hold time passed.
    Click,
    /// The hold time passed with the button still down: show the menu.
    Hold,
    /// Nothing to do (the gesture already ended, or the press moved away).
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Idle,
    Pressed,
    Held,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PressAndHold {
    phase: Phase,
}

impl PressAndHold {
    /// The button went down; starts a new gesture, dropping any unfinished one.
    pub fn pressed(&mut self) {
        self.phase = Phase::Pressed;
    }

    /// The hold timer fired. Only a gesture still waiting becomes a hold.
    pub fn hold_elapsed(&mut self) -> HoldOutcome {
        if self.phase == Phase::Pressed {
            self.phase = Phase::Held;
            HoldOutcome::Hold
        } else {
            HoldOutcome::Nothing
        }
    }

    /// The button came up. A release after a hold is not a click.
    pub fn released(&mut self) -> HoldOutcome {
        let outcome = if self.phase == Phase::Pressed { HoldOutcome::Click } else { HoldOutcome::Nothing };
        self.phase = Phase::Idle;
        outcome
    }

    /// The pointer left the button or lost capture.
    pub fn cancelled(&mut self) {
        self.phase = Phase::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_press_when_released_before_the_hold_time_then_it_is_a_click() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        assert_eq!(gesture.released(), HoldOutcome::Click);
    }

    #[test]
    fn given_a_press_when_the_hold_time_passes_then_the_menu_is_requested() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        assert_eq!(gesture.hold_elapsed(), HoldOutcome::Hold);
    }

    #[test]
    fn given_a_hold_when_the_button_is_released_then_it_is_not_a_click() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        gesture.hold_elapsed();
        assert_eq!(gesture.released(), HoldOutcome::Nothing);
    }

    #[test]
    fn given_a_hold_when_the_timer_fires_again_then_the_menu_is_not_requested_twice() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        gesture.hold_elapsed();
        assert_eq!(gesture.hold_elapsed(), HoldOutcome::Nothing);
    }

    #[test]
    fn given_a_press_when_the_pointer_leaves_then_neither_a_click_nor_a_hold_happens() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        gesture.cancelled();
        assert_eq!(gesture.hold_elapsed(), HoldOutcome::Nothing);
        assert_eq!(gesture.released(), HoldOutcome::Nothing);
    }

    #[test]
    fn given_no_press_when_released_then_nothing_happens() {
        let mut gesture = PressAndHold::default();
        assert_eq!(gesture.released(), HoldOutcome::Nothing);
        assert_eq!(gesture.hold_elapsed(), HoldOutcome::Nothing);
    }

    #[test]
    fn given_a_hold_whose_release_was_swallowed_when_pressed_again_then_the_new_press_clicks() {
        let mut gesture = PressAndHold::default();
        gesture.pressed();
        gesture.hold_elapsed();
        gesture.pressed();
        assert_eq!(gesture.released(), HoldOutcome::Click);
    }
}
