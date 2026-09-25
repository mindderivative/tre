//! M94: `node.on(event, handler)` and `window.on(event, handler)` -- the
//! M93 target API's event model (`docs/design/target-api.md`), beside the
//! legacy `set_on_*` handlers, which keep their exact non-bubbling
//! behavior until M100 removes them.
//!
//! Delivery happens in two places, both after `Tree::dispatch`:
//!
//! - **Raw input** (`route_input`): pointer, wheel, key, and text events.
//!   Their target is resolved *before* dispatch (`target_before`) --
//!   dispatch itself can move focus (Tab) or change what's under the
//!   pointer, and the event belongs to where it happened.
//! - **Outcomes** (`dispatch::run_dispatch_outcome`,
//!   `dispatch::fire_focus_transition`): `click`, `secondary_click`,
//!   `pointer_enter`/`pointer_leave`, `focus`/`blur`, `change` -- the same
//!   places the legacy handlers fire, so every path that already reaches
//!   those (live input, accessibility requests, the synthetic
//!   `Window.click` family) reaches listeners too.
//!
//! Callers deliver raw input before the outcome, so `pointer_up` precedes
//! `click`, as in the DOM.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use engine_core::{Action, InputEvent, Modifiers, NodeId, NodeKind, ScrollDelta, Tree};
use peniko::kurbo::Point;
use pyo3::prelude::*;

use crate::dispatch::{HandlerKey, log_uncaught_exception};
use crate::event::{Event, NodeContext, button_name};

thread_local! {
    /// The modifier keys currently held. One keyboard serves every window,
    /// and every event is handled on the event-loop thread, so this is
    /// per-thread rather than per-window: `engine-platform`'s
    /// `ModifiersChanged` updates it, `Window.simulate` sets it for the
    /// duration of one synthetic event.
    static MODIFIERS: Cell<Modifiers> = const {
        Cell::new(Modifiers {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        })
    };

    /// Whether the user's last interaction was the keyboard (or an
    /// assistive technology) rather than a pointer press -- the browsers'
    /// `:focus-visible` heuristic, reported as a `focus` event's
    /// `focus_visible`. Starts `true`: focus given before any pointer
    /// press is shown. Per-thread for the same reason as `MODIFIERS`.
    static KEYBOARD_MODALITY: Cell<bool> = const { Cell::new(true) };
}

/// Records the input modality `event` implies: a key press (without Ctrl,
/// Alt, or Meta, which are shortcuts, not navigation) means the keyboard,
/// a pointer press the pointer. Anything else leaves it as it was.
pub(crate) fn note_input_modality(event: &InputEvent) {
    let keyboard = match event {
        InputEvent::Key { pressed: true, .. } => {
            let held = modifiers();
            if held.ctrl || held.alt || held.meta {
                return;
            }
            true
        }
        InputEvent::PointerPressed { .. } => false,
        _ => return,
    };
    set_keyboard_modality(keyboard);
}

pub(crate) fn set_keyboard_modality(keyboard: bool) {
    KEYBOARD_MODALITY.with(|cell| cell.set(keyboard));
}

pub(crate) fn modifiers() -> Modifiers {
    MODIFIERS.with(Cell::get)
}

pub(crate) fn set_modifiers(modifiers: Modifiers) {
    MODIFIERS.with(|cell| cell.set(modifiers));
}

/// The engine counts a wheel "line" as this many pixels (`Tree::dispatch`'s
/// own `ScrollDelta::Lines` handling); `wheel` events report pixels.
const WHEEL_LINE_PX: f64 = 20.0;

/// Every event a node listener can register for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EventType {
    PointerEnter,
    PointerLeave,
    PointerDown,
    PointerMove,
    PointerUp,
    Click,
    SecondaryClick,
    Wheel,
    KeyDown,
    KeyUp,
    Input,
    Focus,
    Blur,
    Change,
    A11yAction,
    Dismiss,
}

impl EventType {
    const ALL: [EventType; 16] = [
        Self::PointerEnter,
        Self::PointerLeave,
        Self::PointerDown,
        Self::PointerMove,
        Self::PointerUp,
        Self::Click,
        Self::SecondaryClick,
        Self::Wheel,
        Self::KeyDown,
        Self::KeyUp,
        Self::Input,
        Self::Focus,
        Self::Blur,
        Self::Change,
        Self::A11yAction,
        Self::Dismiss,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::PointerEnter => "pointer_enter",
            Self::PointerLeave => "pointer_leave",
            Self::PointerDown => "pointer_down",
            Self::PointerMove => "pointer_move",
            Self::PointerUp => "pointer_up",
            Self::Click => "click",
            Self::SecondaryClick => "secondary_click",
            Self::Wheel => "wheel",
            Self::KeyDown => "key_down",
            Self::KeyUp => "key_up",
            Self::Input => "input",
            Self::Focus => "focus",
            Self::Blur => "blur",
            Self::Change => "change",
            Self::A11yAction => "a11y_action",
            Self::Dismiss => "dismiss",
        }
    }

    /// M93's R3: `pointer_enter`/`pointer_leave` are per-subtree, and
    /// `change` and `dismiss` belong to one node, so those stay on their
    /// target.
    fn bubbles(self) -> bool {
        !matches!(
            self,
            Self::PointerEnter | Self::PointerLeave | Self::Change | Self::Dismiss
        )
    }

    /// A node event name, or a `ValueError` listing every valid one.
    pub(crate) fn parse(name: &str) -> PyResult<Self> {
        Self::ALL
            .into_iter()
            .find(|event| event.name() == name)
            .ok_or_else(|| {
                let valid: Vec<&str> = Self::ALL.iter().map(|e| e.name()).collect();
                pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown node event {name:?} -- valid events: {}",
                    valid.join(", ")
                ))
            })
    }
}

/// Every event a window listener can register for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WindowEventType {
    Resize,
    ColorScheme,
    ScaleFactor,
    CloseRequested,
    Closed,
}

impl WindowEventType {
    const ALL: [WindowEventType; 5] = [
        Self::Resize,
        Self::ColorScheme,
        Self::ScaleFactor,
        Self::CloseRequested,
        Self::Closed,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Resize => "resize",
            Self::ColorScheme => "color_scheme",
            Self::ScaleFactor => "scale_factor",
            Self::CloseRequested => "close_requested",
            Self::Closed => "closed",
        }
    }

    pub(crate) fn parse(name: &str) -> PyResult<Self> {
        Self::ALL
            .into_iter()
            .find(|event| event.name() == name)
            .ok_or_else(|| {
                let valid: Vec<&str> = Self::ALL.iter().map(|e| e.name()).collect();
                pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown window event {name:?} -- valid events: {}",
                    valid.join(", ")
                ))
            })
    }
}

/// A window's own listeners -- not per-node, and independent of which
/// tree the window currently shows (`show_view` swaps trees).
pub(crate) type WindowListenerMap = Rc<RefCell<HashMap<WindowEventType, (Py<PyAny>, bool)>>>;

/// The node a raw input event is aimed at, resolved before `Tree::dispatch`
/// runs: the capturing node or the node under the pointer for pointer
/// events, the node under the pointer for the wheel, the focused node (or
/// the root, when nothing is focused) for keys, and the focused text input
/// for text. `None` for every other event. Needs a computed layout.
pub(crate) fn target_before(tree: &Tree, root: NodeId, event: &InputEvent) -> Option<NodeId> {
    match event {
        InputEvent::PointerMoved { position }
        | InputEvent::PointerPressed { position, .. }
        | InputEvent::PointerReleased { position, .. } => tree
            .pointer_capture()
            .or_else(|| tree.hit_test_input(root, *position)),
        InputEvent::Scroll { position, .. } => tree.hit_test_input(root, *position),
        InputEvent::Key { .. } => Some(tree.focused().unwrap_or(root)),
        InputEvent::TextInput(_) => tree.focused().filter(|&id| {
            matches!(
                tree.get(id).map(|node| &node.kind),
                Some(NodeKind::TextField(_))
            )
        }),
        _ => None,
    }
}

/// Delivers a raw input event to listeners, starting at `target` (from
/// `target_before`). Releases pointer capture after a `pointer_up`.
pub(crate) fn route_input(
    ctx: &NodeContext<'_>,
    target: Option<NodeId>,
    event: &InputEvent,
    py: Python<'_>,
) {
    if let Some(target) = target {
        match event {
            InputEvent::PointerPressed { position, button } => {
                deliver(
                    ctx,
                    py,
                    EventType::PointerDown,
                    target,
                    Some(*position),
                    |e| {
                        e.button = Some(button_name(*button).to_string());
                        stamp_modifiers(e);
                    },
                );
            }
            InputEvent::PointerMoved { position } => {
                deliver(
                    ctx,
                    py,
                    EventType::PointerMove,
                    target,
                    Some(*position),
                    stamp_modifiers,
                );
            }
            InputEvent::PointerReleased { position, button } => {
                deliver(
                    ctx,
                    py,
                    EventType::PointerUp,
                    target,
                    Some(*position),
                    |e| {
                        e.button = Some(button_name(*button).to_string());
                        stamp_modifiers(e);
                    },
                );
            }
            InputEvent::Scroll { delta, position } => {
                // Pixels, positive `delta_y` scrolling down and positive
                // `delta_x` scrolling right -- the opposite of `winit`'s
                // own wheel-up-is-positive sign.
                let (dx, dy) = match *delta {
                    ScrollDelta::Lines(x, y) => (x * WHEEL_LINE_PX, y * WHEEL_LINE_PX),
                    ScrollDelta::Pixels(x, y) => (x, y),
                };
                deliver(ctx, py, EventType::Wheel, target, Some(*position), |e| {
                    e.delta_x = Some(-dx);
                    e.delta_y = Some(-dy);
                    stamp_modifiers(e);
                });
            }
            InputEvent::Key {
                name,
                pressed,
                repeat,
            } => {
                let event_type = if *pressed {
                    EventType::KeyDown
                } else {
                    EventType::KeyUp
                };
                deliver(ctx, py, event_type, target, None, |e| {
                    e.key = Some(name.clone());
                    e.repeat = Some(*repeat);
                    stamp_modifiers(e);
                });
            }
            InputEvent::TextInput(text) => {
                deliver(ctx, py, EventType::Input, target, None, |e| {
                    e.text = Some(text.clone());
                });
            }
            _ => {}
        }
    }
    if matches!(event, InputEvent::PointerReleased { .. }) {
        ctx.tree.borrow_mut().set_pointer_capture(None);
    }
}

/// `pointer_enter`/`pointer_leave` for a hover change from `old` to `new`:
/// every node whose subtree the pointer left gets `pointer_leave`
/// (innermost first), then every node whose subtree it entered gets
/// `pointer_enter` (outermost first). Moving between a node and its own
/// descendants fires nothing on that node.
pub(crate) fn route_hover(
    ctx: &NodeContext<'_>,
    old: Option<NodeId>,
    new: Option<NodeId>,
    position: Option<Point>,
    py: Python<'_>,
) {
    let (old_chain, new_chain) = {
        let tree = ctx.tree.borrow();
        (
            old.map(|id| tree.ancestors(id).collect::<Vec<_>>())
                .unwrap_or_default(),
            new.map(|id| tree.ancestors(id).collect::<Vec<_>>())
                .unwrap_or_default(),
        )
    };
    for &left in old_chain.iter().filter(|id| !new_chain.contains(id)) {
        deliver(
            ctx,
            py,
            EventType::PointerLeave,
            left,
            position,
            stamp_modifiers,
        );
    }
    for &entered in new_chain.iter().rev().filter(|id| !old_chain.contains(id)) {
        deliver(
            ctx,
            py,
            EventType::PointerEnter,
            entered,
            position,
            stamp_modifiers,
        );
    }
}

/// `blur` on the node losing focus, then `focus` on the node gaining it --
/// both bubbling, so an ancestor learns that focus moved within it. Each
/// carries the other node as `related_target` (`None` when focus comes
/// from, or goes to, nowhere in the window), so a composite widget can
/// tell focus moving between its own children from focus leaving it.
pub(crate) fn route_focus(
    ctx: &NodeContext<'_>,
    old: Option<NodeId>,
    new: Option<NodeId>,
    py: Python<'_>,
) {
    let related = |id: Option<NodeId>| {
        id.and_then(|id| {
            Event::build_node(py, id, ctx)
                .map_err(|err| log_uncaught_exception(&err, py))
                .ok()
        })
    };
    if let Some(old) = old {
        deliver(ctx, py, EventType::Blur, old, None, |e| {
            e.related_target = related(new);
        });
    }
    if let Some(new) = new {
        deliver(ctx, py, EventType::Focus, new, None, |e| {
            e.related_target = related(old);
            e.focus_visible = Some(KEYBOARD_MODALITY.with(Cell::get));
        });
    }
}

/// The actions `a11y_action` reports, by name. Activation arrives as
/// `click` and focus requests as `focus`, as they would from a pointer or
/// keyboard, so neither is here.
pub(crate) const A11Y_ACTIONS: [&str; 6] = [
    "increment",
    "decrement",
    "expand",
    "collapse",
    "scroll_into_view",
    "set_value",
];

/// An assistive-technology action request's `a11y_action` name, if it has
/// one.
pub(crate) fn a11y_action_name(action: Action) -> Option<&'static str> {
    Some(match action {
        Action::Increment => "increment",
        Action::Decrement => "decrement",
        Action::Expand => "expand",
        Action::Collapse => "collapse",
        Action::ScrollIntoView => "scroll_into_view",
        Action::SetValue => "set_value",
        _ => return None,
    })
}

/// Delivers `a11y_action` to `node`'s listeners, bubbling. `value` is the
/// requested value for `set_value`.
pub(crate) fn deliver_a11y_action(
    ctx: &NodeContext<'_>,
    node: NodeId,
    action: &str,
    value: Option<Py<PyAny>>,
    py: Python<'_>,
) {
    deliver(ctx, py, EventType::A11yAction, node, None, |e| {
        e.action = Some(action.to_string());
        e.value = value;
    });
}

pub(crate) fn stamp_modifiers(event: &mut Event) {
    let held = modifiers();
    event.shift = Some(held.shift);
    event.ctrl = Some(held.ctrl);
    event.alt = Some(held.alt);
    event.meta = Some(held.meta);
}

/// Runs `event_type`'s listeners, starting at `target` and -- for a
/// bubbling type -- walking up through its ancestors until one calls
/// `event.stop()`. One `Event` is built (only if some node on the path is
/// listening) and shared by every listener, with `current`, and for a
/// pointer event `x`/`y`, updated to each listener's own node.
/// `window_point` is the event's window-space position, if it has one.
pub(crate) fn deliver(
    ctx: &NodeContext<'_>,
    py: Python<'_>,
    event_type: EventType,
    target: NodeId,
    window_point: Option<Point>,
    fill: impl FnOnce(&mut Event),
) {
    let path = {
        let tree = ctx.tree.borrow();
        if tree.get(target).is_none() {
            return;
        }
        if event_type.bubbles() {
            // M96: an event inside a layer bubbles to the layer and stops
            // there, never reaching the tree underneath.
            let mut path = Vec::new();
            for id in tree.ancestors(target) {
                path.push(id);
                if tree.is_layer(id) {
                    break;
                }
            }
            path
        } else {
            vec![target]
        }
    };
    let key = HandlerKey::Listener(event_type);
    let listening = {
        let handlers = ctx.handlers.borrow();
        path.iter().any(|&id| handlers.contains_key(&(id, key)))
    };
    if !listening {
        return;
    }

    let event = match Event::for_node(py, event_type.name(), target, ctx) {
        Ok(mut event) => {
            if let Some(point) = window_point {
                event.window_x = Some(point.x);
                event.window_y = Some(point.y);
            }
            fill(&mut event);
            Py::new(py, event)
        }
        Err(err) => Err(err),
    };
    let event = match event {
        Ok(event) => event,
        Err(err) => {
            log_uncaught_exception(&err, py);
            return;
        }
    };

    for id in path {
        // Cloned out and the borrow dropped before calling -- a listener
        // may register or remove listeners itself.
        let Some((handler, wants_event)) = ctx
            .handlers
            .borrow()
            .get(&(id, key))
            .map(|(handler, wants)| (handler.clone_ref(py), *wants))
        else {
            continue;
        };
        // A listener earlier in the walk may have removed this node.
        let local = {
            let tree = ctx.tree.borrow();
            if tree.get(id).is_none() {
                break;
            }
            window_point.map(|point| tree.window_to_local(id, point))
        };
        let result = if wants_event {
            match Event::build_node(py, id, ctx) {
                Ok(current) => {
                    {
                        let mut e = event.borrow_mut(py);
                        e.current = Some(current);
                        if let Some(local) = local {
                            e.x = Some(local.x);
                            e.y = Some(local.y);
                        }
                    }
                    handler.call1(py, (event.clone_ref(py),))
                }
                Err(err) => Err(err),
            }
        } else {
            handler.call0(py)
        };
        if let Err(err) = result {
            log_uncaught_exception(&err, py);
        }
        if event.borrow(py).stopped {
            break;
        }
    }
}

/// Runs a window's listener for `event_type`, if it has one. `fill` sets
/// the payload. Returns whether the listener cancelled the event (only
/// `close_requested` is cancellable).
pub(crate) fn deliver_window(
    listeners: &WindowListenerMap,
    py: Python<'_>,
    event_type: WindowEventType,
    fill: impl FnOnce(&mut Event),
) -> bool {
    let Some((handler, wants_event)) = listeners
        .borrow()
        .get(&event_type)
        .map(|(handler, wants)| (handler.clone_ref(py), *wants))
    else {
        return false;
    };
    let mut event = Event::for_window(py, event_type.name());
    event.cancellable = event_type == WindowEventType::CloseRequested;
    fill(&mut event);
    let event = match Py::new(py, event) {
        Ok(event) => event,
        Err(err) => {
            log_uncaught_exception(&err, py);
            return false;
        }
    };
    let result = if wants_event {
        handler.call1(py, (event.clone_ref(py),))
    } else {
        handler.call0(py)
    };
    if let Err(err) = result {
        log_uncaught_exception(&err, py);
    }
    event.borrow(py).cancelled
}
