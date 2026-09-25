//! M94: `Window`'s M93 event surface -- `on`/`off` window listeners,
//! `set`/`get` window properties, and `simulate`, the one headless-testing
//! entry point (D9, R7). `simulate` runs node events through
//! `dispatch::process_input`, the same pipeline `App.run()` uses for real
//! input, so a simulated event reaches listeners and legacy handlers
//! exactly as a real one would.

use std::rc::Rc;
use std::time::Duration;

use engine_core::{
    InputEvent, Key, Modifiers, NodeId, NodeKind, PaintProperties, PathData, PathState,
    PointerButton, ScrollDelta, Tree,
};
use peniko::Color;
use peniko::kurbo::BezPath;
use peniko::kurbo::Point;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use taffy::prelude::Style;
use taffy::prelude::{AvailableSpace, Size};

use crate::clock;
use crate::dispatch::{
    fire_focus_transition, interaction_config, process_input, run_completions, wants_event,
};
use crate::error::EngineError;
use crate::event::NodeContext;
use crate::listeners::{self, WindowEventType};
use crate::node::{Node, NodeState};
use crate::node_handles;
use crate::node_props::parse_all;
use crate::window::PyWindow;

/// Every event `simulate` accepts, for its own error message.
const SIMULATED_EVENTS: [&str; 20] = [
    "pointer_down",
    "pointer_up",
    "pointer_move",
    "pointer_enter",
    "pointer_leave",
    "click",
    "secondary_click",
    "wheel",
    "key_down",
    "key_up",
    "input",
    "focus",
    "blur",
    "a11y_action",
    "resize",
    "color_scheme",
    "scale_factor",
    "close_requested",
    "closed",
    "change",
];

/// `simulate`'s keyword fields, consumed one by one so anything left over
/// is reported as a mistake rather than silently ignored.
struct Fields<'py> {
    event: String,
    entries: Vec<(String, Bound<'py, PyAny>)>,
}

impl<'py> Fields<'py> {
    fn new(event: &str, fields: Option<&Bound<'py, PyDict>>) -> PyResult<Self> {
        let mut entries = Vec::new();
        if let Some(fields) = fields {
            for (name, value) in fields.iter() {
                entries.push((name.extract::<String>()?, value));
            }
        }
        Ok(Self {
            event: event.to_string(),
            entries,
        })
    }

    fn take(&mut self, name: &str) -> Option<Bound<'py, PyAny>> {
        let index = self.entries.iter().position(|(n, _)| n == name)?;
        Some(self.entries.remove(index).1)
    }

    fn typed<T: for<'a> FromPyObject<'a, 'py>>(
        &mut self,
        name: &str,
        type_name: &str,
    ) -> PyResult<Option<T>> {
        match self.take(name) {
            None => Ok(None),
            Some(value) => value.extract::<T>().map(Some).map_err(|_| {
                PyValueError::new_err(format!(
                    "simulate({:?}): `{name}` must be {type_name}",
                    self.event
                ))
            }),
        }
    }

    fn f64(&mut self, name: &str) -> PyResult<Option<f64>> {
        self.typed(name, "a number")
    }

    fn bool(&mut self, name: &str) -> PyResult<Option<bool>> {
        self.typed(name, "a bool")
    }

    fn string(&mut self, name: &str) -> PyResult<Option<String>> {
        self.typed(name, "a str")
    }

    fn required<T>(&self, name: &str, value: Option<T>) -> PyResult<T> {
        value.ok_or_else(|| {
            PyValueError::new_err(format!("simulate({:?}) needs `{name}`", self.event))
        })
    }

    fn modifiers(&mut self) -> PyResult<Modifiers> {
        Ok(Modifiers {
            shift: self.bool("shift")?.unwrap_or(false),
            ctrl: self.bool("ctrl")?.unwrap_or(false),
            alt: self.bool("alt")?.unwrap_or(false),
            meta: self.bool("meta")?.unwrap_or(false),
        })
    }

    fn done(self) -> PyResult<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = self.entries.iter().map(|(n, _)| n.as_str()).collect();
        Err(PyValueError::new_err(format!(
            "simulate({:?}) got unexpected field(s): {}",
            self.event,
            names.join(", ")
        )))
    }
}

/// The engine's own narrow key for a named key, so a simulated key press
/// drives focus movement and text editing exactly as a real one does.
fn engine_key(name: &str) -> Option<Key> {
    Some(match name {
        "tab" => Key::Tab,
        "enter" => Key::Enter,
        "space" => Key::Space,
        "escape" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "arrow_left" => Key::ArrowLeft,
        "arrow_right" => Key::ArrowRight,
        "arrow_up" => Key::ArrowUp,
        "arrow_down" => Key::ArrowDown,
        "home" => Key::Home,
        "end" => Key::End,
        _ => return None,
    })
}

fn pointer_button(name: Option<String>) -> PyResult<PointerButton> {
    match name.as_deref() {
        None | Some("primary") => Ok(PointerButton::Primary),
        Some("secondary") => Ok(PointerButton::Secondary),
        Some("middle") => Ok(PointerButton::Middle),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown button {other:?} -- valid buttons: primary, secondary, middle"
        ))),
    }
}

/// Runs `body` with `modifiers` held, then restores whatever was held.
fn with_modifiers<R>(modifiers: Modifiers, body: impl FnOnce() -> R) -> R {
    let held = listeners::modifiers();
    listeners::set_modifiers(modifiers);
    let result = body();
    listeners::set_modifiers(held);
    result
}

/// A simulated pointer position in window space: `node`'s center, or
/// `x`/`y` local to `node`, or `x`/`y` in window space when there's no
/// node.
fn pointer_point(
    tree: &Tree,
    node: Option<NodeId>,
    x: Option<f64>,
    y: Option<f64>,
    event: &str,
) -> PyResult<Point> {
    match (node, x, y) {
        (Some(id), None, None) => {
            let size = tree.layout(id).size;
            Ok(tree.local_to_window(
                id,
                Point::new(f64::from(size.width) / 2.0, f64::from(size.height) / 2.0),
            ))
        }
        (Some(id), Some(x), Some(y)) => Ok(tree.local_to_window(id, Point::new(x, y))),
        (None, Some(x), Some(y)) => Ok(Point::new(x, y)),
        _ => Err(PyValueError::new_err(format!(
            "simulate({event:?}) needs `node`, `x` and `y` (window space), or all three (local to node)"
        ))),
    }
}

#[pymethods]
impl PyWindow {
    /// The window's root node -- the box its content lives in (the
    /// currently shown one, after `show_view`).
    #[getter]
    fn root(&self) -> Node {
        let active = self.active.borrow();
        Node::from(NodeState {
            id: active.root,
            tree: active.tree.clone(),
            handlers: active.handlers.clone(),
            context_menus: active.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        })
    }

    /// M95: makes a detached node of `kind` in this window's tree and
    /// applies `props` atomically, as `node.set` does -- attach it with
    /// `add_child`. Builds `"box"` and `"path"` (which needs `data`);
    /// M96 adds the remaining kinds.
    #[pyo3(signature = (kind, **props))]
    fn create(&self, kind: &str, props: Option<&Bound<'_, PyDict>>) -> PyResult<Node> {
        let node_kind = match kind {
            "box" => NodeKind::Rect,
            "path" => {
                let has_data = match props {
                    Some(props) => props.contains("data")?,
                    None => false,
                };
                if !has_data {
                    return Err(PyValueError::new_err("create(\"path\") needs `data`"));
                }
                NodeKind::Path(PathState::new(PathData(BezPath::new())))
            }
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown node kind {kind:?} -- create builds: box, path"
                )));
            }
        };
        let changes = parse_all(props, &node_kind)?;
        let active = self.active.borrow();
        let id = {
            let mut tree = active.tree.borrow_mut();
            let id = tree.insert(
                node_kind,
                Style::default(),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
            );
            // M96: freed automatically once no handle points into it
            // (`node_handles`), until it's attached somewhere.
            tree.detach_collectible(id);
            id
        };
        let node = Node::from(NodeState {
            id,
            tree: active.tree.clone(),
            handlers: active.handlers.clone(),
            context_menus: active.context_menus.clone(),
            theme: self.theme.clone(),
            completions: self.completions.clone(),
        });
        drop(active);
        node.apply(changes);
        Ok(node)
    }

    /// Registers `handler` for the window event `event` (`resize`,
    /// `color_scheme`, `scale_factor`, `close_requested`, `closed`),
    /// replacing any earlier one. `handler` receives an `Event`, or
    /// nothing if it takes no parameters.
    fn on(&self, event: &str, handler: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        let event = WindowEventType::parse(event)?;
        let wants = wants_event(py, &handler)?;
        self.window_listeners
            .borrow_mut()
            .insert(event, (handler, wants));
        Ok(())
    }

    /// Removes the window's listener for `event`, if any.
    fn off(&self, event: &str) -> PyResult<()> {
        let event = WindowEventType::parse(event)?;
        self.window_listeners.borrow_mut().remove(&event);
        Ok(())
    }

    /// Sets window properties. Today only `title`; `width`, `height`, and
    /// `scale_factor` are read-only.
    #[pyo3(signature = (**props))]
    fn set(&mut self, props: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let mut title = None;
        if let Some(props) = props {
            for (name, value) in props.iter() {
                let name: String = name.extract()?;
                match name.as_str() {
                    "title" => {
                        title = Some(value.extract::<String>().map_err(|_| {
                            PyValueError::new_err("window property `title` must be a str")
                        })?);
                    }
                    "width" | "height" | "scale_factor" => {
                        return Err(PyValueError::new_err(format!(
                            "window property `{name}` is read-only -- settable: title"
                        )));
                    }
                    _ => {
                        return Err(PyValueError::new_err(format!(
                            "unknown window property {name:?} -- settable: title"
                        )));
                    }
                }
            }
        }
        if let Some(title) = title {
            if let Some(window) = self.os_window.borrow().as_ref() {
                window.set_title(&title);
            }
            self.title = title;
        }
        Ok(())
    }

    /// Reads a window property: `width`, `height`, `title`, or
    /// `scale_factor` (`1.0` until `App.run()` opens the window).
    fn get(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match name {
            "width" => f64::from(self.width.get())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "height" => f64::from(self.height.get())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "title" => self.title.clone().into_pyobject(py)?.into_any().unbind(),
            "scale_factor" => self
                .os_window
                .borrow()
                .as_ref()
                .map_or(1.0, |window| window.scale_factor())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown window property {name:?} -- valid: width, height, title, scale_factor"
                )));
            }
        })
    }

    /// M96 (R7): moves time forward by exactly `ms` milliseconds, then runs
    /// animations, their `on_complete` callbacks, and layout at the new
    /// time -- deterministic headless time for tests, where `App.run()`
    /// renders no frames. The first call pins this window's clock at the
    /// real current time; `App.run()` returns it to the real clock.
    fn advance(&self, ms: f64, py: Python<'_>) -> PyResult<()> {
        if !(ms.is_finite() && ms >= 0.0) {
            return Err(PyValueError::new_err(format!(
                "advance(ms): ms must be a non-negative number, got {ms}"
            )));
        }
        node_handles::reclaim();
        let (tree, root) = {
            let active = self.active.borrow();
            (active.tree.clone(), active.root)
        };
        let now = clock::advance(&tree, Duration::from_secs_f64(ms / 1000.0));
        let (_, completed) = tree.borrow_mut().tick_all(now);
        run_completions(&self.completions, completed, py);
        tree.borrow_mut().compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
        );
        Ok(())
    }

    /// Delivers a synthetic `event` exactly as real input would -- for
    /// headless tests. Pointer events aim at `node`'s center, at `x`/`y`
    /// local to `node`, or at window-space `x`/`y`; `shift`/`ctrl`/`alt`/
    /// `meta` hold modifiers for the event. See the Window API reference
    /// for each event's fields.
    #[pyo3(signature = (event, node=None, **fields))]
    fn simulate(
        &self,
        event: &str,
        node: Option<PyRef<'_, Node>>,
        fields: Option<&Bound<'_, PyDict>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let (tree, root, handlers, context_menus) = {
            let active = self.active.borrow();
            (
                active.tree.clone(),
                active.root,
                active.handlers.clone(),
                active.context_menus.clone(),
            )
        };
        let node_id = match &node {
            Some(node) if !Rc::ptr_eq(&node.tree, &tree) => {
                return Err(EngineError::ForeignNode.into());
            }
            Some(node) => Some(node.id),
            None => None,
        };
        tree.borrow_mut().compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(self.width.get() as f32),
                height: AvailableSpace::Definite(self.height.get() as f32),
            },
        );
        let ctx = NodeContext {
            tree: &tree,
            handlers: &handlers,
            context_menus: &context_menus,
            theme: &self.theme,
            completions: &self.completions,
        };
        let mut f = Fields::new(event, fields)?;
        let need_node = |f: &Fields<'_>| -> PyResult<NodeId> {
            node_id.ok_or_else(|| {
                PyValueError::new_err(format!("simulate({:?}) needs `node`", f.event))
            })
        };

        match event {
            "pointer_down" | "pointer_up" | "pointer_move" | "pointer_enter" | "click"
            | "secondary_click" | "wheel" => {
                let modifiers = f.modifiers()?;
                let (x, y) = (f.f64("x")?, f.f64("y")?);
                let position = pointer_point(&tree.borrow(), node_id, x, y, event)?;
                let inputs = match event {
                    "pointer_down" => vec![InputEvent::PointerPressed {
                        position,
                        button: pointer_button(f.string("button")?)?,
                    }],
                    "pointer_up" => vec![InputEvent::PointerReleased {
                        position,
                        button: pointer_button(f.string("button")?)?,
                    }],
                    "pointer_move" | "pointer_enter" => vec![InputEvent::PointerMoved { position }],
                    "wheel" => {
                        // `wheel`'s own sign: positive `delta_y` scrolls down.
                        let dx = f.f64("delta_x")?.unwrap_or(0.0);
                        let dy = f.f64("delta_y")?.unwrap_or(0.0);
                        vec![InputEvent::Scroll {
                            delta: ScrollDelta::Pixels(-dx, -dy),
                            position,
                        }]
                    }
                    _ => {
                        let button = if event == "click" {
                            PointerButton::Primary
                        } else {
                            PointerButton::Secondary
                        };
                        vec![
                            InputEvent::PointerPressed { position, button },
                            InputEvent::PointerReleased { position, button },
                        ]
                    }
                };
                f.done()?;
                with_modifiers(modifiers, || {
                    for input in &inputs {
                        process_input(&ctx, root, input, py);
                    }
                });
            }
            "pointer_leave" => {
                f.done()?;
                process_input(&ctx, root, &InputEvent::PointerLeft, py);
            }
            "key_down" | "key_up" => {
                let modifiers = f.modifiers()?;
                let key = f.string("key")?;
                let key = f.required("key", key)?;
                let repeat = f.bool("repeat")?.unwrap_or(false);
                f.done()?;
                let pressed = event == "key_down";
                // The same events a real key produces: the named key, then
                // the engine's own narrow key or, for an unmodified
                // character, the text it types.
                let mut inputs = vec![InputEvent::Key {
                    name: key.clone(),
                    pressed,
                    repeat,
                }];
                if let Some(engine_key) = engine_key(&key) {
                    inputs.push(if pressed {
                        InputEvent::KeyPressed {
                            key: engine_key,
                            shift: modifiers.shift,
                        }
                    } else {
                        InputEvent::KeyReleased {
                            key: engine_key,
                            shift: modifiers.shift,
                        }
                    });
                } else if pressed
                    && key.chars().count() == 1
                    && !(modifiers.ctrl || modifiers.alt || modifiers.meta)
                {
                    inputs.push(InputEvent::TextInput(key));
                }
                with_modifiers(modifiers, || {
                    for input in &inputs {
                        process_input(&ctx, root, input, py);
                    }
                });
            }
            "input" => {
                let text = f.string("text")?;
                let text = f.required("text", text)?;
                f.done()?;
                process_input(&ctx, root, &InputEvent::TextInput(text), py);
            }
            "focus" | "blur" => {
                let id = need_node(&f)?;
                f.done()?;
                let config = interaction_config();
                let now = crate::clock::now(&tree);
                let transition = if event == "focus" {
                    tree.borrow_mut().set_focus_to(
                        id,
                        config.focus_ring_opacity,
                        config.focus_ring_duration,
                        now,
                    )
                } else if tree.borrow().focused() == Some(id) {
                    tree.borrow_mut().clear_focus(
                        config.focus_ring_opacity,
                        config.focus_ring_duration,
                        now,
                    )
                } else {
                    None
                };
                if let Some((old, new)) = transition {
                    fire_focus_transition(
                        &handlers,
                        &tree,
                        &context_menus,
                        &self.theme,
                        &self.completions,
                        old,
                        new,
                        py,
                    );
                }
            }
            "a11y_action" => {
                let id = need_node(&f)?;
                let action = f.string("action")?;
                let action = f.required("action", action)?;
                let value = f.take("value").map(Bound::unbind);
                f.done()?;
                if !listeners::A11Y_ACTIONS.contains(&action.as_str()) {
                    return Err(PyValueError::new_err(format!(
                        "unknown a11y action {action:?} -- valid actions: {}",
                        listeners::A11Y_ACTIONS.join(", ")
                    )));
                }
                listeners::deliver_a11y_action(&ctx, id, &action, value, py);
            }
            "change" => {
                return Err(PyValueError::new_err(
                    "`change` fires when a text_input's text changes -- simulate `input` or `key_down` instead",
                ));
            }
            "resize" => {
                let width = f.f64("width")?;
                let width = f.required("width", width)?;
                let height = f.f64("height")?;
                let height = f.required("height", height)?;
                f.done()?;
                self.width.set(width as u32);
                self.height.set(height as u32);
                tree.borrow_mut().dispatch(
                    root,
                    InputEvent::Resized {
                        width: width as f32,
                        height: height as f32,
                    },
                    &interaction_config(),
                    crate::clock::now(&tree),
                );
                listeners::deliver_window(
                    &self.window_listeners,
                    py,
                    WindowEventType::Resize,
                    |e| {
                        e.width = Some(width);
                        e.height = Some(height);
                    },
                );
            }
            "color_scheme" => {
                let dark = f.bool("dark")?;
                let dark = f.required("dark", dark)?;
                f.done()?;
                // What `App.run()` does for a real OS light/dark switch.
                let tint = {
                    let mut theme = self.theme.borrow_mut();
                    theme.set_dark(dark);
                    theme.on_surface()
                };
                {
                    let mut tree = tree.borrow_mut();
                    tree.set_all_interaction_tints(tint);
                    tree.set_all_component_tints(tint);
                }
                listeners::deliver_window(
                    &self.window_listeners,
                    py,
                    WindowEventType::ColorScheme,
                    |e| e.dark = Some(dark),
                );
            }
            "scale_factor" => {
                let scale_factor = f.f64("scale_factor")?;
                let scale_factor = f.required("scale_factor", scale_factor)?;
                f.done()?;
                listeners::deliver_window(
                    &self.window_listeners,
                    py,
                    WindowEventType::ScaleFactor,
                    |e| e.scale_factor = Some(scale_factor),
                );
            }
            "close_requested" | "closed" => {
                f.done()?;
                let event_type = if event == "closed" {
                    WindowEventType::Closed
                } else {
                    WindowEventType::CloseRequested
                };
                listeners::deliver_window(&self.window_listeners, py, event_type, |_| {});
            }
            _ => {
                return Err(PyValueError::new_err(format!(
                    "simulate can't deliver {event:?} -- valid events: {}",
                    SIMULATED_EVENTS.join(", ")
                )));
            }
        }
        Ok(())
    }
}
