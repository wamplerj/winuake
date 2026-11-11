//! What the toggle action does. Ports the branches of Yakuake `_toggleWindowState`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleInputs {
    pub visible: bool,
    pub window_active: bool,
    pub keep_open: bool,
    pub toggle_to_focus: bool,
    pub show_on_all_desktops: bool,
    pub on_other_desktop: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleAction {
    Open,
    Retract,
    /// Already open but without focus: give it focus instead of closing it.
    FocusExisting,
    /// Open on another virtual desktop: bring it here and focus it, rather than making the user press the key twice.
    MoveToCurrentDesktopAndFocus,
}

pub fn decide_toggle(inputs: ToggleInputs) -> ToggleAction {
    if inputs.visible && !inputs.window_active && inputs.keep_open {
        if inputs.toggle_to_focus {
            return ToggleAction::FocusExisting;
        }
        if !inputs.show_on_all_desktops && inputs.on_other_desktop {
            return ToggleAction::MoveToCurrentDesktopAndFocus;
        }
    }
    if inputs.visible { ToggleAction::Retract } else { ToggleAction::Open }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN_WITHOUT_FOCUS: ToggleInputs = ToggleInputs {
        visible: true,
        window_active: false,
        keep_open: true,
        toggle_to_focus: false,
        show_on_all_desktops: true,
        on_other_desktop: false,
    };

    #[test]
    fn given_a_hidden_window_when_toggling_then_it_opens() {
        assert_eq!(decide_toggle(ToggleInputs { visible: false, ..OPEN_WITHOUT_FOCUS }), ToggleAction::Open);
    }

    #[test]
    fn given_an_open_focused_window_when_toggling_then_it_retracts() {
        assert_eq!(decide_toggle(ToggleInputs { window_active: true, ..OPEN_WITHOUT_FOCUS }), ToggleAction::Retract);
    }

    #[test]
    fn given_an_open_unfocused_window_that_stays_open_when_toggle_to_focus_is_on_then_it_is_focused() {
        assert_eq!(
            decide_toggle(ToggleInputs { toggle_to_focus: true, ..OPEN_WITHOUT_FOCUS }),
            ToggleAction::FocusExisting
        );
    }

    #[test]
    fn given_an_open_unfocused_window_when_toggle_to_focus_is_off_then_it_retracts() {
        assert_eq!(decide_toggle(OPEN_WITHOUT_FOCUS), ToggleAction::Retract);
    }

    #[test]
    fn given_the_window_is_on_another_desktop_and_not_shown_on_all_desktops_then_it_is_brought_here_and_focused() {
        let inputs = ToggleInputs { show_on_all_desktops: false, on_other_desktop: true, ..OPEN_WITHOUT_FOCUS };
        assert_eq!(decide_toggle(inputs), ToggleAction::MoveToCurrentDesktopAndFocus);
    }

    #[test]
    fn given_the_window_is_on_another_desktop_but_shown_on_all_desktops_then_the_normal_rule_applies() {
        let inputs = ToggleInputs { on_other_desktop: true, ..OPEN_WITHOUT_FOCUS };
        assert_eq!(decide_toggle(inputs), ToggleAction::Retract);
    }

    #[test]
    fn given_an_unfocused_window_that_does_not_stay_open_when_toggling_then_it_retracts() {
        let inputs = ToggleInputs { keep_open: false, toggle_to_focus: true, ..OPEN_WITHOUT_FOCUS };
        assert_eq!(decide_toggle(inputs), ToggleAction::Retract);
    }
}
