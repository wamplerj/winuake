//! The slide animation. Ports Yakuake's `xshapeOpenWindow` / `xshapeRetractWindow`: linear, one step per 10 ms frame.

use super::geometry::Rect;
use std::time::Duration;

pub const FRAME_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideDirection {
    Opening,
    Closing,
}

/// Where to put the full-size window for one animation frame. The part above the work area (`clip_top` pixels from the window's top)
/// must not be visible, which on Windows is done with a window region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideFrame {
    pub rect: Rect,
    pub clip_top: i32,
}

impl SlideFrame {
    pub fn settled(rect: Rect) -> Self {
        Self { rect, clip_top: 0 }
    }
}

/// Frame number for the time since the animation began.
pub fn frame_at(elapsed: Duration) -> u32 {
    (elapsed.as_millis() / FRAME_INTERVAL.as_millis()) as u32
}

/// The frame to show, or `None` once the animation has run its course (`frame >= frames`).
pub fn slide_frame(final_rect: Rect, frames: u32, frame: u32, direction: SlideDirection) -> Option<SlideFrame> {
    if frames == 0 || frame >= frames {
        return None;
    }
    let height = final_rect.height();
    let step = height / frames as i32;
    let visible = match direction {
        SlideDirection::Opening => step * frame as i32,
        SlideDirection::Closing => step * (frames - frame) as i32,
    };
    let hidden = height - visible;
    Some(SlideFrame {
        rect: Rect::from_origin_size(final_rect.left, final_rect.top - hidden, final_rect.width(), height),
        clip_top: hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rect = Rect::from_origin_size(100, 40, 800, 510);

    #[test]
    fn given_elapsed_time_when_computing_the_frame_then_it_advances_every_ten_milliseconds() {
        assert_eq!(frame_at(Duration::ZERO), 0);
        assert_eq!(frame_at(Duration::from_millis(9)), 0);
        assert_eq!(frame_at(Duration::from_millis(10)), 1);
        assert_eq!(frame_at(Duration::from_millis(175)), 17);
    }

    #[test]
    fn given_the_first_opening_frame_then_the_whole_window_is_above_the_work_area_and_fully_clipped() {
        let frame = slide_frame(WINDOW, 17, 0, SlideDirection::Opening).unwrap();
        assert_eq!(frame.clip_top, 510);
        assert_eq!(frame.rect, Rect::from_origin_size(100, 40 - 510, 800, 510));
    }

    #[test]
    fn given_a_middle_opening_frame_then_the_visible_part_is_a_linear_fraction_of_the_height() {
        let frame = slide_frame(WINDOW, 17, 8, SlideDirection::Opening).unwrap();
        let visible = 510 / 17 * 8;
        assert_eq!(frame.clip_top, 510 - visible);
        assert_eq!(frame.rect.top, 40 - (510 - visible));
        assert_eq!(frame.rect.height(), 510, "the window keeps its full size; only its position and clip change");
    }

    #[test]
    fn given_the_closing_direction_then_the_window_starts_nearly_visible_and_retracts() {
        let first = slide_frame(WINDOW, 17, 0, SlideDirection::Closing).unwrap();
        let last = slide_frame(WINDOW, 17, 16, SlideDirection::Closing).unwrap();
        assert!(first.clip_top < last.clip_top);
        assert_eq!(first.clip_top, 510 - 510 / 17 * 17);
        assert_eq!(last.clip_top, 510 - 510 / 17);
    }

    #[test]
    fn given_the_final_frame_or_beyond_when_sliding_then_the_animation_is_finished() {
        assert_eq!(slide_frame(WINDOW, 17, 17, SlideDirection::Opening), None);
        assert_eq!(slide_frame(WINDOW, 17, 500, SlideDirection::Closing), None);
    }

    #[test]
    fn given_zero_frames_when_sliding_then_there_is_no_animation() {
        assert_eq!(slide_frame(WINDOW, 0, 0, SlideDirection::Opening), None);
    }

    #[test]
    fn given_the_horizontal_position_when_sliding_then_it_never_changes() {
        for frame in 0..17 {
            let slide = slide_frame(WINDOW, 17, frame, SlideDirection::Opening).unwrap();
            assert_eq!((slide.rect.left, slide.rect.width()), (100, 800));
        }
    }

    #[test]
    fn given_a_settled_frame_then_nothing_is_clipped() {
        assert_eq!(SlideFrame::settled(WINDOW), SlideFrame { rect: WINDOW, clip_top: 0 });
    }
}
