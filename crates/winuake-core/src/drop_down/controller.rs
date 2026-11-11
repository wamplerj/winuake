//! The drop-down window as a state machine. Ports Yakuake `MainWindow` open/retract logic: events in, commands out, no windowing calls,
//! so every decision is unit-testable. The Windows layer executes the commands.

use super::geometry::{
    Monitor, PercentStep, PlacementRequest, Point, Rect, select_screen, stepped_percent, window_rect,
};
use super::slide::{SlideDirection, SlideFrame, frame_at, slide_frame};
use super::toggle::{ToggleAction, ToggleInputs, decide_toggle};
use std::time::Duration;
use winuake_config::window::WindowSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropDownState {
    Hidden,
    Opening,
    Open,
    Closing,
}

/// What the world looks like right now; supplied with every event.
#[derive(Debug, Clone, Copy)]
pub struct WindowContext<'a> {
    /// Ordered as `order_monitors` returns them.
    pub monitors: &'a [Monitor],
    pub pointer: Point,
    pub window_is_active: bool,
    pub window_on_other_desktop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropDownEvent {
    /// The global hotkey, tray click, or a second instance.
    Toggle,
    /// Time since the animation started; sent while an animation timer runs.
    AnimationTick(Duration),
    /// Foreground window changed. `belongs_to_window` is true when focus moved to one of the app's own windows (a dialog).
    FocusChanged {
        active: bool,
        belongs_to_window: bool,
    },
    AdjustWidth(PercentStep),
    AdjustHeight(PercentStep),
    ToggleFullscreen,
    SettingsChanged(WindowSettings),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropDownCommand {
    /// Move/resize the window and clip its top `clip_top` pixels.
    PositionWindow(SlideFrame),
    ShowWindow,
    HideWindow,
    ActivateWindow,
    MoveToCurrentDesktop,
    SetTopmost(bool),
    StartAnimationTimer,
    StopAnimationTimer,
    StartFocusWatch,
    StopFocusWatch,
    /// Settings the user changed through an action; the caller saves them.
    PersistSettings(WindowSettings),
}

#[derive(Debug)]
pub struct DropDownController {
    settings: WindowSettings,
    state: DropDownState,
    fullscreen: bool,
    final_rect: Option<Rect>,
    last_frame: Option<u32>,
    /// Becomes true once the window has actually been seen with focus; focus loss before that is the open itself settling.
    seen_active: bool,
    /// Focus was lost while the opening animation was still running; retract as soon as it finishes.
    focus_lost_while_opening: bool,
}

impl DropDownController {
    pub fn new(settings: WindowSettings) -> Self {
        Self {
            settings: settings.clamped(),
            state: DropDownState::Hidden,
            fullscreen: false,
            final_rect: None,
            last_frame: None,
            seen_active: false,
            focus_lost_while_opening: false,
        }
    }

    pub fn state(&self) -> DropDownState {
        self.state
    }

    pub fn settings(&self) -> &WindowSettings {
        &self.settings
    }

    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn handle(&mut self, event: DropDownEvent, context: &WindowContext<'_>) -> Vec<DropDownCommand> {
        match event {
            DropDownEvent::Toggle => self.toggle(context),
            DropDownEvent::AnimationTick(elapsed) => self.animation_tick(elapsed),
            DropDownEvent::FocusChanged { active, belongs_to_window } => {
                self.focus_changed(active || belongs_to_window, belongs_to_window)
            }
            DropDownEvent::AdjustWidth(step) => {
                let width = stepped_percent(self.settings.width_percent, step);
                self.change_settings(WindowSettings { width_percent: width, ..self.settings.clone() }, context, true)
            }
            DropDownEvent::AdjustHeight(step) => {
                let height = stepped_percent(self.settings.height_percent, step);
                self.change_settings(WindowSettings { height_percent: height, ..self.settings.clone() }, context, true)
            }
            DropDownEvent::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                self.reposition_if_visible(context)
            }
            DropDownEvent::SettingsChanged(settings) => self.change_settings(settings.clamped(), context, false),
        }
    }

    fn toggle(&mut self, context: &WindowContext<'_>) -> Vec<DropDownCommand> {
        if matches!(self.state, DropDownState::Opening | DropDownState::Closing) {
            return Vec::new();
        }
        let action = decide_toggle(ToggleInputs {
            visible: self.state == DropDownState::Open,
            window_active: context.window_is_active,
            keep_open: self.settings.keep_open,
            toggle_to_focus: self.settings.toggle_to_focus,
            show_on_all_desktops: self.settings.show_on_all_desktops,
            on_other_desktop: context.window_on_other_desktop,
        });
        match action {
            ToggleAction::FocusExisting => vec![DropDownCommand::ActivateWindow],
            ToggleAction::MoveToCurrentDesktopAndFocus => {
                vec![DropDownCommand::MoveToCurrentDesktop, DropDownCommand::ActivateWindow]
            }
            ToggleAction::Open => self.open(context),
            ToggleAction::Retract => self.retract(),
        }
    }

    fn open(&mut self, context: &WindowContext<'_>) -> Vec<DropDownCommand> {
        let Some(rect) = self.target_rect(context) else { return Vec::new() };
        self.final_rect = Some(rect);
        self.seen_active = false;
        self.focus_lost_while_opening = false;
        self.last_frame = None;

        let mut commands = vec![DropDownCommand::SetTopmost(self.settings.wants_topmost())];
        match slide_frame(rect, self.settings.frames, 0, SlideDirection::Opening) {
            Some(first_frame) => {
                self.state = DropDownState::Opening;
                self.last_frame = Some(0);
                commands.extend([
                    DropDownCommand::PositionWindow(first_frame),
                    DropDownCommand::ShowWindow,
                    DropDownCommand::ActivateWindow,
                    DropDownCommand::StartAnimationTimer,
                ]);
            }
            None => {
                self.state = DropDownState::Open;
                commands.extend([
                    DropDownCommand::PositionWindow(SlideFrame::settled(rect)),
                    DropDownCommand::ShowWindow,
                    DropDownCommand::ActivateWindow,
                ]);
            }
        }
        commands.push(DropDownCommand::StartFocusWatch);
        commands
    }

    fn retract(&mut self) -> Vec<DropDownCommand> {
        let mut commands = vec![DropDownCommand::StopFocusWatch];
        self.last_frame = None;
        if self.settings.frames == 0 {
            self.finish_hiding(&mut commands);
        } else {
            self.state = DropDownState::Closing;
            commands.push(DropDownCommand::StartAnimationTimer);
        }
        commands
    }

    fn finish_hiding(&mut self, commands: &mut Vec<DropDownCommand>) {
        self.state = DropDownState::Hidden;
        commands.push(DropDownCommand::HideWindow);
        if !self.settings.remember_fullscreen {
            self.fullscreen = false;
        }
    }

    fn animation_tick(&mut self, elapsed: Duration) -> Vec<DropDownCommand> {
        let direction = match self.state {
            DropDownState::Opening => SlideDirection::Opening,
            DropDownState::Closing => SlideDirection::Closing,
            DropDownState::Hidden | DropDownState::Open => return Vec::new(),
        };
        let Some(final_rect) = self.final_rect else { return Vec::new() };
        let frame = frame_at(elapsed);

        match slide_frame(final_rect, self.settings.frames, frame, direction) {
            Some(slide) => {
                if self.last_frame == Some(frame) {
                    return Vec::new();
                }
                self.last_frame = Some(frame);
                vec![DropDownCommand::PositionWindow(slide)]
            }
            None if direction == SlideDirection::Opening => {
                self.state = DropDownState::Open;
                let mut commands = vec![
                    DropDownCommand::PositionWindow(SlideFrame::settled(final_rect)),
                    DropDownCommand::StopAnimationTimer,
                ];
                if std::mem::take(&mut self.focus_lost_while_opening) {
                    commands.extend(self.retract());
                }
                commands
            }
            None => {
                let mut commands = vec![DropDownCommand::StopAnimationTimer];
                self.finish_hiding(&mut commands);
                commands
            }
        }
    }

    fn focus_changed(&mut self, has_focus: bool, belongs_to_window: bool) -> Vec<DropDownCommand> {
        if !matches!(self.state, DropDownState::Opening | DropDownState::Open) {
            return Vec::new();
        }
        if has_focus {
            self.seen_active = true;
            self.focus_lost_while_opening = false;
            return Vec::new();
        }
        // Focus going to the app's own windows (dialogs) never retracts; before the first focus the window is still settling.
        if belongs_to_window || !self.seen_active || self.settings.keep_open {
            return Vec::new();
        }
        if self.state == DropDownState::Opening {
            self.focus_lost_while_opening = true;
            return Vec::new();
        }
        self.retract()
    }

    fn change_settings(
        &mut self,
        settings: WindowSettings,
        context: &WindowContext<'_>,
        persist: bool,
    ) -> Vec<DropDownCommand> {
        if settings == self.settings {
            return Vec::new();
        }
        self.settings = settings;
        let mut commands = Vec::new();
        if persist {
            commands.push(DropDownCommand::PersistSettings(self.settings.clone()));
        }
        commands.extend(self.reposition_if_visible(context));
        commands
    }

    fn reposition_if_visible(&mut self, context: &WindowContext<'_>) -> Vec<DropDownCommand> {
        if self.state == DropDownState::Hidden {
            return Vec::new();
        }
        let Some(rect) = self.target_rect(context) else { return Vec::new() };
        self.final_rect = Some(rect);
        if self.state == DropDownState::Open {
            vec![DropDownCommand::PositionWindow(SlideFrame::settled(rect))]
        } else {
            Vec::new()
        }
    }

    fn target_rect(&self, context: &WindowContext<'_>) -> Option<Rect> {
        let monitor = context.monitors.get(select_screen(self.settings.screen, context.monitors, context.pointer))?;
        Some(window_rect(
            monitor,
            PlacementRequest {
                width_percent: self.settings.width_percent,
                height_percent: self.settings.height_percent,
                position_percent: self.settings.position_percent,
                fullscreen: self.fullscreen,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Monitor =
        Monitor { bounds: Rect::new(0, 0, 2000, 1000), work_area: Rect::new(0, 0, 2000, 960), is_primary: true };
    const MONITORS: [Monitor; 1] = [SCREEN];

    fn context(active: bool) -> WindowContext<'static> {
        WindowContext {
            monitors: &MONITORS,
            pointer: Point { x: 10, y: 10 },
            window_is_active: active,
            window_on_other_desktop: false,
        }
    }

    fn instant() -> WindowSettings {
        WindowSettings {
            frames: 0,
            width_percent: 50,
            height_percent: 50,
            position_percent: 50,
            ..WindowSettings::default()
        }
    }

    fn animated() -> WindowSettings {
        WindowSettings {
            frames: 10,
            width_percent: 50,
            height_percent: 50,
            position_percent: 50,
            ..WindowSettings::default()
        }
    }

    fn controller(settings: WindowSettings) -> DropDownController {
        DropDownController::new(settings)
    }

    fn final_rect() -> Rect {
        Rect::from_origin_size(500, 0, 1000, 480)
    }

    fn open_instantly(controller: &mut DropDownController) {
        controller.handle(DropDownEvent::Toggle, &context(false));
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_a_hidden_window_when_toggling_without_animation_then_it_is_placed_shown_and_focused_in_one_step() {
        let mut controller = controller(instant());
        let commands = controller.handle(DropDownEvent::Toggle, &context(false));
        assert_eq!(
            commands,
            vec![
                DropDownCommand::SetTopmost(true),
                DropDownCommand::PositionWindow(SlideFrame::settled(final_rect())),
                DropDownCommand::ShowWindow,
                DropDownCommand::ActivateWindow,
                DropDownCommand::StartFocusWatch,
            ]
        );
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_an_open_focused_window_when_toggling_without_animation_then_it_hides_immediately() {
        let mut controller = controller(instant());
        open_instantly(&mut controller);
        let commands = controller.handle(DropDownEvent::Toggle, &context(true));
        assert_eq!(commands, vec![DropDownCommand::StopFocusWatch, DropDownCommand::HideWindow]);
        assert_eq!(controller.state(), DropDownState::Hidden);
    }

    #[test]
    fn given_animation_when_opening_then_the_first_frame_is_clipped_and_a_timer_starts() {
        let mut controller = controller(animated());
        let commands = controller.handle(DropDownEvent::Toggle, &context(false));
        assert_eq!(controller.state(), DropDownState::Opening);
        assert!(commands.contains(&DropDownCommand::StartAnimationTimer));
        let Some(DropDownCommand::PositionWindow(first)) =
            commands.iter().find(|command| matches!(command, DropDownCommand::PositionWindow(_)))
        else {
            panic!("no position command in {commands:?}");
        };
        assert_eq!(first.clip_top, 480);
    }

    #[test]
    fn given_an_opening_animation_when_ticks_arrive_then_frames_progress_and_the_last_tick_settles_the_window() {
        let mut controller = controller(animated());
        controller.handle(DropDownEvent::Toggle, &context(false));

        let middle = controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(50)), &context(true));
        assert!(
            matches!(middle.as_slice(), [DropDownCommand::PositionWindow(frame)] if frame.clip_top == 480 - 48 * 5)
        );
        assert_eq!(controller.state(), DropDownState::Opening);

        let done = controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(100)), &context(true));
        assert_eq!(
            done,
            vec![
                DropDownCommand::PositionWindow(SlideFrame::settled(final_rect())),
                DropDownCommand::StopAnimationTimer
            ]
        );
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_two_ticks_within_the_same_frame_when_animating_then_the_second_is_ignored() {
        let mut controller = controller(animated());
        controller.handle(DropDownEvent::Toggle, &context(false));
        assert_eq!(controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(30)), &context(true)).len(), 1);
        assert!(controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(33)), &context(true)).is_empty());
    }

    #[test]
    fn given_an_open_window_with_animation_when_retracting_then_it_closes_frame_by_frame_and_hides_at_the_end() {
        let mut controller = controller(animated());
        controller.handle(DropDownEvent::Toggle, &context(false));
        controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(200)), &context(true));

        let start = controller.handle(DropDownEvent::Toggle, &context(true));
        assert_eq!(start, vec![DropDownCommand::StopFocusWatch, DropDownCommand::StartAnimationTimer]);
        assert_eq!(controller.state(), DropDownState::Closing);

        let middle = controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(30)), &context(true));
        assert!(
            matches!(middle.as_slice(), [DropDownCommand::PositionWindow(frame)] if frame.clip_top == 480 - 48 * 7)
        );

        let end = controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(100)), &context(true));
        assert_eq!(end, vec![DropDownCommand::StopAnimationTimer, DropDownCommand::HideWindow]);
        assert_eq!(controller.state(), DropDownState::Hidden);
    }

    #[test]
    fn given_an_animation_in_progress_when_toggling_then_the_toggle_is_ignored() {
        let mut controller = controller(animated());
        controller.handle(DropDownEvent::Toggle, &context(false));
        assert!(controller.handle(DropDownEvent::Toggle, &context(true)).is_empty());
        assert_eq!(controller.state(), DropDownState::Opening);
    }

    #[test]
    fn given_no_monitors_when_toggling_then_nothing_happens() {
        let mut controller = controller(instant());
        let empty = WindowContext { monitors: &[], ..context(false) };
        assert!(controller.handle(DropDownEvent::Toggle, &empty).is_empty());
        assert_eq!(controller.state(), DropDownState::Hidden);
    }

    #[test]
    fn given_keep_above_off_and_keep_open_on_when_opening_then_the_window_is_not_topmost() {
        let mut controller = controller(WindowSettings { keep_above: false, ..instant() });
        let commands = controller.handle(DropDownEvent::Toggle, &context(false));
        assert_eq!(commands[0], DropDownCommand::SetTopmost(false));
    }

    #[test]
    fn given_toggle_to_focus_when_the_open_window_lacks_focus_then_toggling_focuses_it_instead_of_closing() {
        let mut controller = controller(WindowSettings { toggle_to_focus: true, ..instant() });
        open_instantly(&mut controller);
        assert_eq!(controller.handle(DropDownEvent::Toggle, &context(false)), vec![DropDownCommand::ActivateWindow]);
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_the_window_is_on_another_desktop_when_toggling_then_it_moves_to_the_current_desktop_and_gets_focus() {
        let mut controller = controller(WindowSettings { show_on_all_desktops: false, ..instant() });
        open_instantly(&mut controller);
        let elsewhere = WindowContext { window_on_other_desktop: true, ..context(false) };
        assert_eq!(
            controller.handle(DropDownEvent::Toggle, &elsewhere),
            vec![DropDownCommand::MoveToCurrentDesktop, DropDownCommand::ActivateWindow]
        );
    }

    #[test]
    fn given_retract_on_focus_loss_when_focus_is_lost_after_being_seen_then_the_window_retracts() {
        let mut controller = controller(WindowSettings { keep_open: false, ..instant() });
        open_instantly(&mut controller);
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        let commands =
            controller.handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false));
        assert_eq!(commands, vec![DropDownCommand::StopFocusWatch, DropDownCommand::HideWindow]);
        assert_eq!(controller.state(), DropDownState::Hidden);
    }

    #[test]
    fn given_retract_on_focus_loss_when_focus_was_never_seen_then_the_opening_itself_does_not_retract() {
        let mut controller = controller(WindowSettings { keep_open: false, ..instant() });
        open_instantly(&mut controller);
        assert!(
            controller
                .handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false))
                .is_empty()
        );
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_focus_arrives_during_the_opening_animation_when_it_is_lost_after_opening_then_the_window_retracts() {
        let mut controller = controller(WindowSettings { keep_open: false, ..animated() });
        controller.handle(DropDownEvent::Toggle, &context(false));
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(200)), &context(true));
        assert_eq!(controller.state(), DropDownState::Open);

        let commands =
            controller.handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false));
        assert_eq!(commands, vec![DropDownCommand::StopFocusWatch, DropDownCommand::StartAnimationTimer]);
        assert_eq!(controller.state(), DropDownState::Closing);
    }

    #[test]
    fn given_focus_is_lost_during_the_opening_animation_then_the_window_retracts_as_soon_as_it_has_opened() {
        let mut controller = controller(WindowSettings { keep_open: false, ..animated() });
        controller.handle(DropDownEvent::Toggle, &context(false));
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        assert!(
            controller
                .handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false))
                .is_empty()
        );

        let settled = controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(200)), &context(false));
        assert_eq!(
            settled,
            vec![
                DropDownCommand::PositionWindow(SlideFrame::settled(final_rect())),
                DropDownCommand::StopAnimationTimer,
                DropDownCommand::StopFocusWatch,
                DropDownCommand::StartAnimationTimer,
            ]
        );
        assert_eq!(controller.state(), DropDownState::Closing);
    }

    #[test]
    fn given_focus_returns_during_the_opening_animation_then_the_pending_retract_is_cancelled() {
        let mut controller = controller(WindowSettings { keep_open: false, ..animated() });
        controller.handle(DropDownEvent::Toggle, &context(false));
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        controller.handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false));
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        controller.handle(DropDownEvent::AnimationTick(Duration::from_millis(200)), &context(true));
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_keep_open_when_focus_is_lost_then_the_window_stays() {
        let mut controller = controller(instant());
        open_instantly(&mut controller);
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        assert!(
            controller
                .handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false))
                .is_empty()
        );
    }

    #[test]
    fn given_focus_moves_to_one_of_the_apps_own_windows_when_retract_on_focus_loss_then_the_window_stays() {
        let mut controller = controller(WindowSettings { keep_open: false, ..instant() });
        open_instantly(&mut controller);
        controller.handle(DropDownEvent::FocusChanged { active: true, belongs_to_window: false }, &context(true));
        assert!(
            controller
                .handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: true }, &context(false))
                .is_empty()
        );
        assert_eq!(controller.state(), DropDownState::Open);
    }

    #[test]
    fn given_a_hidden_window_when_focus_changes_then_nothing_happens() {
        let mut controller = controller(WindowSettings { keep_open: false, ..instant() });
        assert!(
            controller
                .handle(DropDownEvent::FocusChanged { active: false, belongs_to_window: false }, &context(false))
                .is_empty()
        );
    }

    #[test]
    fn given_an_open_window_when_widening_then_the_setting_is_persisted_and_the_window_is_repositioned() {
        let mut controller = controller(instant());
        open_instantly(&mut controller);
        let commands = controller.handle(DropDownEvent::AdjustWidth(PercentStep::Increase), &context(true));
        let wider = Rect::from_origin_size(400, 0, 1200, 480);
        assert_eq!(commands.len(), 2);
        assert!(matches!(&commands[0], DropDownCommand::PersistSettings(settings) if settings.width_percent == 60));
        assert_eq!(commands[1], DropDownCommand::PositionWindow(SlideFrame::settled(wider)));
    }

    #[test]
    fn given_a_hidden_window_when_changing_height_then_the_setting_is_persisted_without_moving_anything() {
        let mut controller = controller(instant());
        let commands = controller.handle(DropDownEvent::AdjustHeight(PercentStep::Decrease), &context(false));
        assert!(
            matches!(commands.as_slice(), [DropDownCommand::PersistSettings(settings)] if settings.height_percent == 40)
        );
    }

    #[test]
    fn given_the_width_limit_when_adjusting_then_nothing_happens() {
        let mut controller = controller(WindowSettings { width_percent: 100, ..instant() });
        assert!(controller.handle(DropDownEvent::AdjustWidth(PercentStep::Increase), &context(false)).is_empty());
    }

    #[test]
    fn given_an_open_window_when_toggling_fullscreen_then_it_covers_the_whole_monitor_and_back() {
        let mut controller = controller(instant());
        open_instantly(&mut controller);
        let full = controller.handle(DropDownEvent::ToggleFullscreen, &context(true));
        assert_eq!(full, vec![DropDownCommand::PositionWindow(SlideFrame::settled(SCREEN.bounds))]);
        assert!(controller.is_fullscreen());
        let normal = controller.handle(DropDownEvent::ToggleFullscreen, &context(true));
        assert_eq!(normal, vec![DropDownCommand::PositionWindow(SlideFrame::settled(final_rect()))]);
    }

    #[test]
    fn given_fullscreen_when_the_window_hides_then_fullscreen_is_forgotten_unless_remembered() {
        let mut forgetting = controller(instant());
        open_instantly(&mut forgetting);
        forgetting.handle(DropDownEvent::ToggleFullscreen, &context(true));
        forgetting.handle(DropDownEvent::Toggle, &context(true));
        assert!(!forgetting.is_fullscreen());

        let mut remembering = controller(WindowSettings { remember_fullscreen: true, ..instant() });
        open_instantly(&mut remembering);
        remembering.handle(DropDownEvent::ToggleFullscreen, &context(true));
        remembering.handle(DropDownEvent::Toggle, &context(true));
        assert!(remembering.is_fullscreen());
    }

    #[test]
    fn given_changed_settings_when_the_window_is_open_then_it_is_repositioned_without_persisting() {
        let mut controller = controller(instant());
        open_instantly(&mut controller);
        let commands = controller.handle(
            DropDownEvent::SettingsChanged(WindowSettings { height_percent: 100, ..instant() }),
            &context(true),
        );
        assert_eq!(
            commands,
            vec![DropDownCommand::PositionWindow(SlideFrame::settled(Rect::from_origin_size(500, 0, 1000, 960)))]
        );
    }

    #[test]
    fn given_out_of_range_settings_when_constructing_then_they_are_clamped() {
        let controller = controller(WindowSettings { width_percent: 0, ..instant() });
        assert_eq!(controller.settings().width_percent, 10);
    }

    #[test]
    fn given_screen_two_when_opening_then_the_window_appears_on_the_second_monitor() {
        let second = Monitor {
            bounds: Rect::new(2000, 0, 4000, 1000),
            work_area: Rect::new(2000, 0, 4000, 1000),
            is_primary: false,
        };
        let monitors = [SCREEN, second];
        let both = WindowContext { monitors: &monitors, ..context(false) };
        let mut controller = controller(WindowSettings { screen: 2, ..instant() });
        let commands = controller.handle(DropDownEvent::Toggle, &both);
        assert!(commands.contains(&DropDownCommand::PositionWindow(SlideFrame::settled(Rect::from_origin_size(
            2500, 0, 1000, 500
        )))));
    }
}
