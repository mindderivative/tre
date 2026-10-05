//! 0.5.4 (#113): touch input -- fingers, and the gestures they make.
//!
//! A touch reaches a node as `touch_start`/`touch_move`/`touch_end`/
//! `touch_cancel` (with `pointer_id`), captured to the node the finger landed
//! on, and is fed to the window's `GestureRecognizer`, whose tap, long press,
//! pan and pinch go to nodes as events of those names.
//!
//! The first finger on the screen is also a pointer, so an app written for a
//! mouse works under a finger: a tap is a `click`, a touch drags over a button
//! and hovers it. Once that finger pans, the press is cancelled (`pointer_cancel`,
//! no `click`) and the pan scrolls whatever scrolls under it, unless something
//! under the finger listens for `pan` itself.

use std::collections::HashMap;
use std::time::Instant;

use engine_core::{
    Gesture, GestureKind, GesturePhase, GestureRecognizer, InputEvent, NodeId, PointerButton,
    ScrollDelta, TouchPhase,
};
use peniko::kurbo::Point;
use pyo3::prelude::*;

use crate::dispatch::{HandlerKey, WindowIo, process_input};
use crate::event::{Event, NodeContext};
use crate::listeners::{EventType, deliver};

/// A window's touch state.
pub(crate) struct TouchRouter {
    recognizer: GestureRecognizer,
    /// Each finger down, and the node it landed on (if any).
    targets: HashMap<u64, Option<NodeId>>,
    /// The finger standing in for the pointer: the first, while it is the only one.
    primary: Option<u64>,
    /// Where the current pan, and pinch, are delivered.
    pan_target: Option<NodeId>,
    pinch_target: Option<NodeId>,
    /// A trackpad pinch's scale so far.
    trackpad_scale: f64,
    /// The pointer events the primary finger stood in for, for the live loop
    /// to replay into text-input handling (which needs the text renderer).
    pub(crate) emulated: Vec<Emulated>,
}

/// One thing for the live loop to replay into text handling after a touch.
pub(crate) enum Emulated {
    /// A pointer event the primary finger stood in for.
    Pointer(InputEvent),
    /// The press is not going to be a click or a drag: the touch was taken
    /// (cancelled), or became a pan. Any drag or link press it started ends.
    Cancel,
}

impl Default for TouchRouter {
    fn default() -> Self {
        Self {
            recognizer: GestureRecognizer::default(),
            targets: HashMap::new(),
            primary: None,
            pan_target: None,
            pinch_target: None,
            trackpad_scale: 1.0,
            emulated: Vec::new(),
        }
    }
}

impl TouchRouter {
    /// Whether the frame loop should keep running: a finger is down waiting to
    /// become a long press.
    pub(crate) fn needs_frames(&self) -> bool {
        self.recognizer.deadline().is_some()
    }
}

fn phase_name(phase: GesturePhase) -> &'static str {
    match phase {
        GesturePhase::Began => "began",
        GesturePhase::Changed => "changed",
        GesturePhase::Ended => "ended",
        GesturePhase::Cancelled => "cancelled",
    }
}

fn hit(ctx: &NodeContext<'_>, root: NodeId, at: Point) -> Option<NodeId> {
    ctx.tree.borrow().hit_test_local(root, at).map(|(id, _)| id)
}

/// Whether `node` or an ancestor listens for `pan`.
fn listens_for_pan(ctx: &NodeContext<'_>, node: NodeId) -> bool {
    let tree = ctx.tree.borrow();
    let handlers = ctx.handlers.borrow();
    tree.ancestors(node)
        .any(|id| handlers.contains_key(&(id, HandlerKey::Listener(EventType::Pan))))
}

/// Feeds a touch or trackpad pinch through the window.
pub(crate) fn process(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    event: &InputEvent,
    py: Python<'_>,
) {
    let now = crate::clock::now(ctx.tree);
    // Time that passed before this event may have made a long press.
    poll(ctx, io, root, now, py);
    match *event {
        InputEvent::Touch {
            id,
            phase,
            position,
        } => touch(ctx, io, root, id, phase, position, now, py),
        InputEvent::TrackpadPinch {
            delta,
            phase,
            position,
        } => trackpad_pinch(ctx, io, root, delta, phase, position, py),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn touch(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    id: u64,
    phase: TouchPhase,
    position: Point,
    now: Instant,
    py: Python<'_>,
) {
    let router = &io.window.touch;
    let (target, is_primary) = {
        let mut r = router.borrow_mut();
        if phase == TouchPhase::Started {
            let target = hit(ctx, root, position);
            // A finger landing on content that is coasting catches it.
            if let Some(node) = target {
                ctx.tree.borrow_mut().stop_scroll_animation(node);
            }
            if r.targets.is_empty() {
                r.primary = Some(id);
            }
            r.targets.insert(id, target);
        }
        (r.targets.get(&id).copied().flatten(), r.primary == Some(id))
    };

    let event_type = match phase {
        TouchPhase::Started => EventType::TouchStart,
        TouchPhase::Moved => EventType::TouchMove,
        TouchPhase::Ended => EventType::TouchEnd,
        TouchPhase::Cancelled => EventType::TouchCancel,
    };
    if let Some(target) = target {
        deliver(ctx, py, event_type, target, Some(position), |e| {
            e.pointer_id = Some(id);
        });
    }

    if is_primary {
        let pointer = match phase {
            TouchPhase::Started => Some(InputEvent::PointerPressed {
                position,
                button: PointerButton::Primary,
            }),
            TouchPhase::Moved => Some(InputEvent::PointerMoved { position }),
            TouchPhase::Ended => Some(InputEvent::PointerReleased {
                position,
                button: PointerButton::Primary,
            }),
            TouchPhase::Cancelled => None,
        };
        router
            .borrow_mut()
            .emulated
            .extend(pointer.map(Emulated::Pointer));
        match phase {
            TouchPhase::Started => {
                process_input(ctx, io, root, &InputEvent::PointerMoved { position }, py);
                process_input(
                    ctx,
                    io,
                    root,
                    &InputEvent::PointerPressed {
                        position,
                        button: PointerButton::Primary,
                    },
                    py,
                );
            }
            TouchPhase::Moved => {
                process_input(ctx, io, root, &InputEvent::PointerMoved { position }, py);
            }
            TouchPhase::Ended => {
                process_input(
                    ctx,
                    io,
                    root,
                    &InputEvent::PointerReleased {
                        position,
                        button: PointerButton::Primary,
                    },
                    py,
                );
                // Nothing is hovered once the finger is up.
                process_input(ctx, io, root, &InputEvent::PointerLeft, py);
            }
            TouchPhase::Cancelled => cancel_press(ctx, io, target, py),
        }
    }

    let gestures = router
        .borrow_mut()
        .recognizer
        .touch(id, phase, position, now);
    deliver_gestures(ctx, io, root, &gestures, target, py);

    if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
        let mut r = router.borrow_mut();
        r.targets.remove(&id);
        if r.primary == Some(id) {
            r.primary = None;
        }
        if r.targets.is_empty() {
            r.pan_target = None;
            r.pinch_target = None;
        }
    }
}

/// A press that is not going to be a click: the finger panned, or was taken.
fn cancel_press(ctx: &NodeContext<'_>, io: &WindowIo<'_>, target: Option<NodeId>, py: Python<'_>) {
    let pressed = ctx.tree.borrow_mut().cancel_press();
    io.window.press_cancelled.set(true);
    // Text handling runs later in the live loop; tell it the press is over.
    io.window.touch.borrow_mut().emulated.push(Emulated::Cancel);
    if let Some(node) = pressed.or(target) {
        deliver(ctx, py, EventType::PointerCancel, node, None, |_| {});
    }
}

fn trackpad_pinch(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    delta: f64,
    phase: TouchPhase,
    position: Point,
    py: Python<'_>,
) {
    let router = &io.window.touch;
    let (gesture_phase, scale, step) = {
        let mut r = router.borrow_mut();
        let step = 1.0 + delta;
        match phase {
            TouchPhase::Started => {
                r.trackpad_scale = step;
                r.pinch_target = hit(ctx, root, position);
                (GesturePhase::Began, step, step)
            }
            TouchPhase::Moved => {
                r.trackpad_scale *= step;
                (GesturePhase::Changed, r.trackpad_scale, step)
            }
            TouchPhase::Ended => (GesturePhase::Ended, r.trackpad_scale, 1.0),
            TouchPhase::Cancelled => (GesturePhase::Cancelled, r.trackpad_scale, 1.0),
        }
    };
    let mut gesture = Gesture {
        kind: GestureKind::Pinch,
        phase: gesture_phase,
        position,
        origin: position,
        delta: peniko::kurbo::Vec2::ZERO,
        total: peniko::kurbo::Vec2::ZERO,
        scale,
        scale_delta: step,
        velocity: peniko::kurbo::Vec2::ZERO,
    };
    gesture.scale_delta = step;
    let target = router.borrow().pinch_target;
    deliver_gestures(ctx, io, root, &[gesture], target, py);
}

/// Lets the time passing fire a long press.
pub(crate) fn poll(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    now: Instant,
    py: Python<'_>,
) {
    let gestures = io.window.touch.borrow_mut().recognizer.poll(now);
    if gestures.is_empty() {
        return;
    }
    let target = {
        let r = io.window.touch.borrow();
        r.primary
            .and_then(|id| r.targets.get(&id).copied().flatten())
    };
    deliver_gestures(ctx, io, root, &gestures, target, py);
}

fn deliver_gestures(
    ctx: &NodeContext<'_>,
    io: &WindowIo<'_>,
    root: NodeId,
    gestures: &[Gesture],
    touch_target: Option<NodeId>,
    py: Python<'_>,
) {
    for gesture in gestures {
        let fill = |e: &mut Event| {
            e.phase = Some(phase_name(gesture.phase).to_string());
            e.delta_x = Some(gesture.delta.x);
            e.delta_y = Some(gesture.delta.y);
            e.total_x = Some(gesture.total.x);
            e.total_y = Some(gesture.total.y);
            e.scale = Some(gesture.scale);
            e.scale_delta = Some(gesture.scale_delta);
            e.velocity_x = Some(gesture.velocity.x);
            e.velocity_y = Some(gesture.velocity.y);
        };
        match gesture.kind {
            GestureKind::Tap { count } => {
                if let Some(target) = touch_target {
                    deliver(
                        ctx,
                        py,
                        EventType::Tap,
                        target,
                        Some(gesture.position),
                        |e| {
                            e.count = Some(u32::from(count));
                        },
                    );
                }
            }
            GestureKind::LongPress => {
                if let Some(target) = touch_target {
                    deliver(
                        ctx,
                        py,
                        EventType::LongPress,
                        target,
                        Some(gesture.position),
                        |_| {},
                    );
                }
            }
            GestureKind::Pan => {
                if gesture.phase == GesturePhase::Began {
                    // The press became a drag: no click now.
                    cancel_press(ctx, io, touch_target, py);
                    io.window.touch.borrow_mut().pan_target = touch_target;
                }
                let target = io.window.touch.borrow().pan_target;
                if let Some(target) = target {
                    deliver(
                        ctx,
                        py,
                        EventType::Pan,
                        target,
                        Some(gesture.position),
                        fill,
                    );
                    // The pan scrolls what is under the finger, unless the app
                    // listens for `pan` there and does its own thing.
                    // Lifting the finger mid-flick lets the content coast on.
                    if gesture.phase == GesturePhase::Ended && !listens_for_pan(ctx, target) {
                        let now = crate::clock::now(ctx.tree);
                        ctx.tree
                            .borrow_mut()
                            .fling_scroll(target, gesture.velocity, now);
                    }
                    if matches!(gesture.phase, GesturePhase::Began | GesturePhase::Changed)
                        && !listens_for_pan(ctx, target)
                    {
                        process_input(
                            ctx,
                            io,
                            root,
                            &InputEvent::Scroll {
                                delta: ScrollDelta::Pixels(gesture.delta.x, gesture.delta.y),
                                position: gesture.position,
                            },
                            py,
                        );
                    }
                }
            }
            GestureKind::Pinch => {
                if gesture.phase == GesturePhase::Began && touch_target.is_none() {
                    io.window.touch.borrow_mut().pinch_target = hit(ctx, root, gesture.origin);
                } else if gesture.phase == GesturePhase::Began {
                    io.window.touch.borrow_mut().pinch_target =
                        hit(ctx, root, gesture.origin).or(touch_target);
                }
                let target = io.window.touch.borrow().pinch_target.or(touch_target);
                if let Some(target) = target {
                    deliver(
                        ctx,
                        py,
                        EventType::Pinch,
                        target,
                        Some(gesture.position),
                        fill,
                    );
                }
            }
        }
    }
}
