//! Mouse, wheel, selection and clipboard behavior of `TerminalView`.

use super::{TerminalView, current_modifiers};
use crate::bindings::{PointerRoutedEventArgs, PointerUpdateKind};
use crate::clipboard;
use std::time::Instant;
use windows_core::Result;
use winuake_term::emulator::{CellSide, ScrollRequest, ViewportCell};
use winuake_term::input::{
    CellPosition, MouseAction, MouseButton, MouseReporting, encode_mouse, encode_paste, encode_wheel_as_arrows,
};
use winuake_term::pointer::{LINES_PER_WHEEL_NOTCH, cell_at_pixel};

struct PointerSample {
    cell: ViewportCell,
    side: CellSide,
    update_kind: PointerUpdateKind,
    wheel_delta: i32,
}

impl TerminalView {
    fn sample_pointer(&self, args: &PointerRoutedEventArgs) -> Result<PointerSample> {
        let point = args.GetCurrentPoint(&self.panel)?;
        let position = point.Position()?;
        let properties = point.Properties()?;
        let (scale, metrics) = {
            let state = self.render.borrow();
            (state.scale, state.renderer.cell_metrics())
        };
        let (columns, rows) = self.session.with_emulator(|emulator| (emulator.columns(), emulator.rows()));
        let (cell, side) =
            cell_at_pixel(position.X * scale, position.Y * scale, metrics.width, metrics.height, columns, rows);
        Ok(PointerSample {
            cell,
            side,
            update_kind: properties.PointerUpdateKind()?,
            wheel_delta: properties.MouseWheelDelta()?,
        })
    }

    fn report_mouse(&self, action: MouseAction, cell: ViewportCell) {
        let modes = self.session.with_emulator(|emulator| emulator.input_modes());
        let position = CellPosition {
            column: cell.column.min(usize::from(u16::MAX)) as u16,
            row: cell.row.min(usize::from(u16::MAX)) as u16,
        };
        if let Some(bytes) = encode_mouse(action, position, current_modifiers(), &modes)
            && let Err(error) = self.session.send_input(&bytes)
        {
            tracing::warn!(terminal_id = %self.id, %error, "could not report mouse event to shell");
        }
    }

    /// Applications that asked for mouse events get them, unless Shift is held (the xterm convention for "select text instead").
    fn application_wants_mouse(&self) -> bool {
        let reporting = self.session.with_emulator(|emulator| emulator.input_modes().mouse_reporting);
        reporting != MouseReporting::Off && !current_modifiers().shift
    }

    pub(super) fn handle_pointer_pressed(&self, args: &PointerRoutedEventArgs) {
        let Ok(sample) = self.sample_pointer(args) else { return };
        self.focus();
        let button = match sample.update_kind {
            PointerUpdateKind::LeftButtonPressed => MouseButton::Left,
            PointerUpdateKind::MiddleButtonPressed => MouseButton::Middle,
            PointerUpdateKind::RightButtonPressed => MouseButton::Right,
            _ => return,
        };
        let _ = args.SetHandled(true);

        if self.application_wants_mouse() {
            self.pressed_button.set(Some(button));
            self.capture_pointer(args);
            self.report_mouse(MouseAction::Press(button), sample.cell);
            return;
        }

        match button {
            MouseButton::Left => {
                let kind = self.clicks.borrow_mut().register_click(sample.cell, Instant::now());
                self.session.update_emulator(|emulator| emulator.start_selection(kind, sample.cell, sample.side));
                self.selecting.set(true);
                self.capture_pointer(args);
                self.redraw();
            }
            MouseButton::Right if self.copy_selection() => {}
            MouseButton::Right | MouseButton::Middle => self.paste_clipboard(),
        }
    }

    pub(super) fn handle_pointer_moved(&self, args: &PointerRoutedEventArgs) {
        let Ok(sample) = self.sample_pointer(args) else { return };
        if self.selecting.get() {
            self.session.update_emulator(|emulator| emulator.update_selection(sample.cell, sample.side));
            self.redraw();
        } else if let Some(button) = self.pressed_button.get() {
            self.report_mouse(MouseAction::Move(Some(button)), sample.cell);
        } else if self.application_wants_mouse() {
            self.report_mouse(MouseAction::Move(None), sample.cell);
        }
    }

    pub(super) fn handle_pointer_released(&self, args: &PointerRoutedEventArgs) {
        let Ok(sample) = self.sample_pointer(args) else { return };
        if self.selecting.replace(false) {
            self.release_pointer(args);
            // A click without a drag selects nothing; make sure no stale highlight remains.
            self.session.update_emulator(|emulator| {
                if !emulator.has_selection() {
                    emulator.clear_selection();
                }
            });
            self.redraw();
        } else if let Some(button) = self.pressed_button.take() {
            self.release_pointer(args);
            self.report_mouse(MouseAction::Release(button), sample.cell);
        }
    }

    pub(super) fn handle_pointer_wheel(&self, args: &PointerRoutedEventArgs) {
        let Ok(sample) = self.sample_pointer(args) else { return };
        let notches = self.wheel.borrow_mut().add(sample.wheel_delta);
        if notches == 0 {
            return;
        }
        let _ = args.SetHandled(true);

        if self.application_wants_mouse() {
            let action = if notches > 0 { MouseAction::WheelUp } else { MouseAction::WheelDown };
            for _ in 0..notches.unsigned_abs() {
                self.report_mouse(action, sample.cell);
            }
            return;
        }

        let modes = self.session.with_emulator(|emulator| emulator.input_modes());
        if let Some(arrows) = encode_wheel_as_arrows(notches, &modes) {
            if let Err(error) = self.session.send_input(&arrows) {
                tracing::warn!(terminal_id = %self.id, %error, "could not send wheel scroll to shell");
            }
            return;
        }
        self.session
            .update_emulator(|emulator| emulator.scroll_view(ScrollRequest::Lines(notches * LINES_PER_WHEEL_NOTCH)));
        self.redraw();
    }

    /// Copies the selection to the clipboard and clears it. Returns whether there was anything to copy.
    pub(super) fn copy_selection(&self) -> bool {
        let Some(text) = self.session.with_emulator(|emulator| emulator.selection_text()) else { return false };
        tracing::info!(terminal_id = %self.id, length = text.len(), "copying selection");
        if let Err(error) = clipboard::set_text(self.window_handle, &text) {
            tracing::warn!(terminal_id = %self.id, %error, "could not copy to clipboard");
        }
        self.session.update_emulator(|emulator| emulator.clear_selection());
        self.redraw();
        true
    }

    pub(super) fn paste_clipboard(&self) {
        let text = match clipboard::get_text(self.window_handle) {
            Ok(Some(text)) => text,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(terminal_id = %self.id, %error, "could not read clipboard");
                return;
            }
        };
        tracing::info!(terminal_id = %self.id, length = text.len(), "pasting into terminal");
        let modes = self.session.with_emulator(|emulator| emulator.input_modes());
        self.session.update_emulator(|emulator| {
            emulator.clear_selection();
            emulator.scroll_view(ScrollRequest::Bottom);
        });
        if let Err(error) = self.session.send_input(&encode_paste(&text, &modes)) {
            tracing::warn!(terminal_id = %self.id, %error, "could not paste into shell");
        }
        self.redraw();
    }

    fn capture_pointer(&self, args: &PointerRoutedEventArgs) {
        if let Ok(pointer) = args.Pointer() {
            let _ = self.control.CapturePointer(&pointer);
        }
    }

    fn release_pointer(&self, args: &PointerRoutedEventArgs) {
        if let Ok(pointer) = args.Pointer() {
            let _ = self.control.ReleasePointerCapture(&pointer);
        }
    }
}
