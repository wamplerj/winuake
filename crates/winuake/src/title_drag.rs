//! Dragging the title bar changes the window height (Yakuake `TitleBar::mouseMoveEvent`).

use crate::bindings::UIElement;
use std::cell::Cell;
use windows_core::Result;

thread_local! {
    static DRAGGING: Cell<bool> = const { Cell::new(false) };
}

pub fn connect(title_bar: &UIElement) -> Result<()> {
    let capture = title_bar.clone();
    std::mem::forget(title_bar.PointerPressed(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        if let Ok(pointer) = args.Pointer() {
            let _ = capture.CapturePointer(&pointer);
        }
        DRAGGING.with(|flag| flag.set(true));
    })?);
    std::mem::forget(title_bar.PointerMoved(move |_, _| {
        if DRAGGING.with(Cell::get) {
            crate::drop_down::set_height_from_pointer();
        }
    })?);
    let release = title_bar.clone();
    std::mem::forget(title_bar.PointerReleased(move |_, args| {
        if DRAGGING.with(|flag| flag.replace(false)) {
            crate::settings_store::save_current();
        }
        if let Some(args) = args.as_ref()
            && let Ok(pointer) = args.Pointer()
        {
            let _ = release.ReleasePointerCapture(&pointer);
        }
    })?);
    Ok(())
}
