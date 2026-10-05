//! 0.5.4 (#102): logical and physical pixels.
//!
//! An app lays out in *logical* pixels. The window's surface, and everything
//! `winit` reports -- pointer positions, pixel scroll deltas, the window's
//! size -- is in *physical* ones. With `dpi_scaling` on, the window's scale
//! factor is the ratio, and the conversion happens in two places only: input
//! comes in here, divided by the scale, and the renderer multiplies the scale
//! back in as the root transform of everything it draws. With it off the scale
//! is `1.0` and nothing changes.

use engine_core::{InputEvent, ScrollDelta};
use peniko::kurbo::Point;

/// `event` in logical pixels: pointer positions, pixel scroll deltas and sizes
/// divided by `scale`. A line-based scroll, keys, and every other event carry
/// no pixel measure and pass through.
pub(crate) fn to_logical(event: InputEvent, scale: f64) -> InputEvent {
    if scale == 1.0 {
        return event;
    }
    let point = |p: Point| Point::new(p.x / scale, p.y / scale);
    match event {
        InputEvent::FileHovered { path, position } => InputEvent::FileHovered {
            path,
            position: point(position),
        },
        InputEvent::FileDropped { path, position } => InputEvent::FileDropped {
            path,
            position: point(position),
        },
        InputEvent::Touch {
            id,
            phase,
            position,
        } => InputEvent::Touch {
            id,
            phase,
            position: point(position),
        },
        InputEvent::TrackpadPinch {
            delta,
            phase,
            position,
        } => InputEvent::TrackpadPinch {
            delta,
            phase,
            position: point(position),
        },
        InputEvent::PointerMoved { position } => InputEvent::PointerMoved {
            position: point(position),
        },
        InputEvent::PointerPressed { position, button } => InputEvent::PointerPressed {
            position: point(position),
            button,
        },
        InputEvent::PointerReleased { position, button } => InputEvent::PointerReleased {
            position: point(position),
            button,
        },
        InputEvent::Scroll { delta, position } => InputEvent::Scroll {
            delta: match delta {
                ScrollDelta::Pixels(x, y) => ScrollDelta::Pixels(x / scale, y / scale),
                lines @ ScrollDelta::Lines(..) => lines,
            },
            position: point(position),
        },
        InputEvent::Resized { width, height } => InputEvent::Resized {
            width: (f64::from(width) / scale) as f32,
            height: (f64::from(height) / scale) as f32,
        },
        other => other,
    }
}

/// A logical length as whole physical pixels, rounded to the nearest.
pub(crate) fn to_physical(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::PointerButton;

    #[test]
    fn at_scale_1_nothing_changes() {
        let event = InputEvent::PointerMoved {
            position: Point::new(10.5, 20.25),
        };
        assert_eq!(to_logical(event.clone(), 1.0), event);
    }

    #[test]
    fn pointer_positions_are_divided_by_the_scale() {
        let moved = to_logical(
            InputEvent::PointerMoved {
                position: Point::new(300.0, 150.0),
            },
            1.5,
        );
        assert_eq!(
            moved,
            InputEvent::PointerMoved {
                position: Point::new(200.0, 100.0)
            }
        );
        let pressed = to_logical(
            InputEvent::PointerPressed {
                position: Point::new(40.0, 80.0),
                button: PointerButton::Primary,
            },
            2.0,
        );
        assert_eq!(
            pressed,
            InputEvent::PointerPressed {
                position: Point::new(20.0, 40.0),
                button: PointerButton::Primary
            }
        );
    }

    #[test]
    fn pixel_scrolls_are_divided_and_line_scrolls_are_not() {
        let pixels = to_logical(
            InputEvent::Scroll {
                delta: ScrollDelta::Pixels(20.0, -40.0),
                position: Point::new(100.0, 100.0),
            },
            2.0,
        );
        assert_eq!(
            pixels,
            InputEvent::Scroll {
                delta: ScrollDelta::Pixels(10.0, -20.0),
                position: Point::new(50.0, 50.0)
            }
        );
        let lines = to_logical(
            InputEvent::Scroll {
                delta: ScrollDelta::Lines(0.0, -3.0),
                position: Point::new(100.0, 100.0),
            },
            2.0,
        );
        assert_eq!(
            lines,
            InputEvent::Scroll {
                delta: ScrollDelta::Lines(0.0, -3.0),
                position: Point::new(50.0, 50.0)
            }
        );
    }

    #[test]
    fn a_resize_is_reported_in_logical_pixels() {
        assert_eq!(
            to_logical(
                InputEvent::Resized {
                    width: 1280.0,
                    height: 800.0
                },
                2.0
            ),
            InputEvent::Resized {
                width: 640.0,
                height: 400.0
            }
        );
    }

    #[test]
    fn physical_lengths_round_to_the_nearest_pixel() {
        assert_eq!(to_physical(320.0, 2.0), 640);
        assert_eq!(to_physical(333.0, 1.5), 500); // 499.5 rounds up
        assert_eq!(to_physical(100.0, 1.25), 125);
        assert_eq!(to_physical(-5.0, 2.0), 0);
    }

    #[test]
    fn touches_and_trackpad_pinches_are_divided_by_the_scale() {
        use engine_core::TouchPhase;
        assert_eq!(
            to_logical(
                InputEvent::Touch {
                    id: 3,
                    phase: TouchPhase::Moved,
                    position: Point::new(300.0, 150.0),
                },
                1.5
            ),
            InputEvent::Touch {
                id: 3,
                phase: TouchPhase::Moved,
                position: Point::new(200.0, 100.0)
            }
        );
        assert_eq!(
            to_logical(
                InputEvent::TrackpadPinch {
                    delta: 0.1,
                    phase: TouchPhase::Started,
                    position: Point::new(40.0, 80.0),
                },
                2.0
            ),
            InputEvent::TrackpadPinch {
                delta: 0.1,
                phase: TouchPhase::Started,
                position: Point::new(20.0, 40.0)
            }
        );
    }

    #[test]
    fn file_positions_are_divided_by_the_scale() {
        assert_eq!(
            to_logical(
                InputEvent::FileDropped {
                    path: "/tmp/a".into(),
                    position: Point::new(60.0, 30.0),
                },
                2.0
            ),
            InputEvent::FileDropped {
                path: "/tmp/a".into(),
                position: Point::new(30.0, 15.0)
            }
        );
    }

    #[test]
    fn other_events_pass_through() {
        assert_eq!(
            to_logical(InputEvent::PointerLeft, 2.0),
            InputEvent::PointerLeft
        );
    }
}
