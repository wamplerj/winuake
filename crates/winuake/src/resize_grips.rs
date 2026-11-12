//! Resize grips on the window edges: invisible strips on the left and right edges and squares in the bottom corners. Dragging one resizes
//! the window (width and position, plus height from the corners) with a size cursor; the height can also be dragged on the title bar.

use crate::bindings::{FrameworkElement, InputCursor, InputSystemCursor, InputSystemCursorShape, UIElement};
use std::cell::Cell;
use windows_core::{HSTRING, Interface, Result};
use winuake_core::drop_down::{ResizeEdge, ResizedWindow};

#[derive(Clone, Copy)]
struct Drag {
    edge: ResizeEdge,
    start: ResizedWindow,
}

thread_local! {
    /// The edge being dragged and the placement when the drag began.
    static DRAGGED: Cell<Option<Drag>> = const { Cell::new(None) };
}

/// Grip markup to put at the end of the window's root grid (so it sits above the content). The root has three rows.
pub const GRIP_MARKUP: &str = r##"
  <Border x:Name="LeftGrip" Grid.RowSpan="3" Width="6" HorizontalAlignment="Left" VerticalAlignment="Stretch" Background="#01000000"/>
  <Border x:Name="RightGrip" Grid.RowSpan="3" Width="6" HorizontalAlignment="Right" VerticalAlignment="Stretch" Background="#01000000"/>
  <Border x:Name="BottomLeftGrip" Grid.RowSpan="3" Width="10" Height="10" HorizontalAlignment="Left" VerticalAlignment="Bottom" Background="#01000000"/>
  <Border x:Name="BottomRightGrip" Grid.RowSpan="3" Width="10" Height="10" HorizontalAlignment="Right" VerticalAlignment="Bottom" Background="#01000000"/>"##;

pub fn connect(root: &FrameworkElement) -> Result<()> {
    for (name, edge, shape) in [
        ("LeftGrip", ResizeEdge::Left, InputSystemCursorShape::SizeWestEast),
        ("RightGrip", ResizeEdge::Right, InputSystemCursorShape::SizeWestEast),
        ("BottomLeftGrip", ResizeEdge::BottomLeft, InputSystemCursorShape::SizeNortheastSouthwest),
        ("BottomRightGrip", ResizeEdge::BottomRight, InputSystemCursorShape::SizeNorthwestSoutheast),
    ] {
        let grip: UIElement = root.FindName(&HSTRING::from(name))?.cast()?;
        let cursor: InputCursor = InputSystemCursor::Create(shape)?.cast()?;
        grip.SetProtectedCursor(&cursor)?;
        connect_drag(&grip, edge)?;
    }
    Ok(())
}

fn connect_drag(grip: &UIElement, edge: ResizeEdge) -> Result<()> {
    let capture = grip.clone();
    std::mem::forget(grip.PointerPressed(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        if let Ok(pointer) = args.Pointer() {
            let _ = capture.CapturePointer(&pointer);
        }
        DRAGGED.with(|slot| slot.set(Some(Drag { edge, start: crate::drop_down::current_placement() })));
        let _ = args.SetHandled(true);
    })?);
    std::mem::forget(grip.PointerMoved(move |_, _| {
        if let Some(drag) = DRAGGED.with(Cell::get)
            && drag.edge == edge
        {
            crate::drop_down::resize_from_pointer(edge, drag.start);
        }
    })?);
    let release = grip.clone();
    std::mem::forget(grip.PointerReleased(move |_, args| {
        if DRAGGED.with(|slot| slot.replace(None)).is_some() {
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
