//! M94: `Window`'s M93 event surface -- `on`/`off` window listeners,
//! `set`/`get` window properties, and `simulate`, the one headless-testing
//! entry point (D9, R7). `simulate` runs node events through
//! `dispatch::process_input`, the same pipeline `App.run()` uses for real
//! input, so a simulated event reaches listeners exactly as a real one
//! would.

use std::rc::Rc;
use std::time::Duration;

use engine_core::{
    AccessNodeData, Action, Animated, CanvasState, ImageState, InputEvent, ItemExtent, Key,
    Modifiers, NodeId, NodeKind, PaintProperties, PathData, PathState, PointerButton, Role,
    ScrollDelta, ScrollViewState, TerminalState, TextAlign, TextFieldState, TextOptions, TextState,
    Tree, VirtualListState,
};
use engine_render::{FontSpec, MONOSPACE_FONT_FAMILY};
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
    WindowIo, fire_focus_transition, process_input, run_completions, wants_event,
};
use crate::error::EngineError;
use crate::event::NodeContext;
use crate::listeners::{self, WindowEventType};
use crate::node::{Node, NodeState};
use crate::node_callbacks;
use crate::node_handles;
use crate::node_kind_props::{FONT_STYLE, OVERFLOW, WRAP};
use crate::node_props::parse_all;
use crate::terminal::TerminalSession;
use crate::window::PyWindow;

/// Every event `simulate` accepts, for its own error message.
const SIMULATED_EVENTS: [&str; 23] = [
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
    "unfocus",
    "a11y_action",
    "resize",
    "color_scheme",
    "scale_factor",
    "close_requested",
    "closed",
    "change",
    "maximized",
    "active",
    "titlebar_inset",
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
        "page_up" => Key::PageUp,
        "page_down" => Key::PageDown,
        _ => return None,
    })
}

fn pointer_button(name: Option<String>) -> PyResult<PointerButton> {
    match name.as_deref() {
        None | Some("primary") => Ok(PointerButton::Primary),
        Some("secondary") => Ok(PointerButton::Secondary),
        Some("middle") => Ok(PointerButton::Middle),
        Some("back") => Ok(PointerButton::Back),
        Some("forward") => Ok(PointerButton::Forward),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown button {other:?} -- valid buttons: primary, secondary, middle, back, forward"
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

impl PyWindow {
    /// The window's size, as layout's available space.
    fn available(&self) -> Size<AvailableSpace> {
        Size {
            width: AvailableSpace::Definite(self.handles.width.get() as f32),
            height: AvailableSpace::Definite(self.handles.height.get() as f32),
        }
    }
}

/// 0.5.0 M2: the window properties `set` takes, for its error messages.
const SETTABLE: &str = "title, partial_redraw, show_damage, decorations, fullscreen, \
    min_width, min_height, icon, resize_border, system_menu";

/// 0.5.0 M2: a window icon from `(rgba, width, height)` -- straight-alpha
/// RGBA8 bytes, `width * height * 4` of them.
fn parse_icon(value: &Bound<'_, PyAny>) -> PyResult<(Vec<u8>, u32, u32)> {
    let bad = || {
        PyValueError::new_err(
            "window property `icon` must be (rgba, width, height): RGBA8 bytes and two \
             positive ints, or None",
        )
    };
    let (rgba, width, height): (Vec<u8>, u32, u32) = value.extract().map_err(|_| bad())?;
    if width == 0 || height == 0 {
        return Err(bad());
    }
    let wanted = width as usize * height as usize * 4;
    if rgba.len() != wanted {
        return Err(PyValueError::new_err(format!(
            "window property `icon`: {width}x{height} needs {wanted} bytes of RGBA8, got {}",
            rgba.len()
        )));
    }
    Ok((rgba, width, height))
}

/// 0.5.0 M4: whether the OS draws its window controls over the content --
/// macOS's traffic lights on an undecorated window, hidden in fullscreen --
/// so the framework leaves room for them rather than drawing its own.
pub(crate) fn native_controls(handles: &crate::window::WindowHandles) -> bool {
    engine_platform::titlebar::OVERLAY_TITLEBAR
        && !handles.decorations.get()
        && !handles.fullscreen.get()
}

/// 0.5.0 M4: re-reads the open window's titlebar inset, firing
/// `titlebar_inset` if it changed.
pub(crate) fn refresh_titlebar_inset(handles: &crate::window::WindowHandles, py: Python<'_>) {
    let inset = match handles.os_window.borrow().as_deref() {
        Some(window) => engine_platform::titlebar::titlebar_inset(window),
        None => return,
    };
    listeners::update_titlebar_inset(
        &handles.window_listeners,
        py,
        &handles.titlebar_inset,
        inset,
    );
}

/// 0.5.0 M2: grows the open window to its minimum size if it's smaller --
/// found live on Wayland, where a minimum only limits what the user can
/// resize to: a window already smaller, or restored from fullscreen to a
/// smaller size, stays that size. Checked when the minimum is set and after
/// every resize. A maximized or fullscreen window is left alone -- resizing
/// it would un-maximize it.
pub(crate) fn grow_to_minimum(handles: &crate::window::WindowHandles) {
    let (min_w, min_h) = handles.min_size.get();
    let window = handles.os_window.borrow();
    let Some(window) = window.as_ref() else {
        return;
    };
    if window.is_maximized() || window.fullscreen().is_some() {
        return;
    }
    let now: winit::dpi::LogicalSize<f64> = window.inner_size().to_logical(window.scale_factor());
    if now.width >= min_w && now.height >= min_h {
        return;
    }
    let applied = window.request_inner_size(winit::dpi::LogicalSize::new(
        now.width.max(min_w),
        now.height.max(min_h),
    ));
    // Applied at once (Wayland), maybe with no resize event: the loop
    // reports it, so layout and `resize` listeners follow.
    if applied.is_some()
        && let Some(waker) = handles.waker.borrow().as_ref()
    {
        waker.report_size(window.id());
    }
}

/// 0.5.0 M2: a minimum-size edge, a non-negative number of pixels.
fn parse_min_edge(name: &str, value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let bad = || PyValueError::new_err(format!("window property `{name}` must be a number >= 0"));
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(bad());
    }
    let edge: f64 = value.extract().map_err(|_| bad())?;
    if edge < 0.0 || !edge.is_finite() {
        return Err(bad());
    }
    Ok(edge)
}

#[pymethods]
impl PyWindow {
    /// The window's root node -- the box its content lives in.
    #[getter]
    fn root(&self) -> Node {
        Node::from(NodeState {
            id: self.handles.root,
            tree: self.handles.tree.clone(),
            handlers: self.handles.handlers.clone(),
            completions: self.handles.completions.clone(),
        })
    }

    /// M95/M96: makes a detached node of `kind` in this window's tree and
    /// applies `props` atomically, as `node.set` does -- attach it with
    /// `add_child`. A kind's required properties must be among `props`; a
    /// terminal's `shell` and `scrollback_lines` are given here only. The
    /// node is freed once no handle points into it, until it's attached.
    #[pyo3(signature = (kind, **props))]
    fn create(
        &self,
        kind: &str,
        props: Option<&Bound<'_, PyDict>>,
        py: Python<'_>,
    ) -> PyResult<Node> {
        const KINDS: &str =
            "box, text, text_input, image, path, canvas, scroll_view, virtual_list, terminal";
        let props = match props {
            Some(props) => props.copy()?,
            None => PyDict::new(py),
        };
        let require = |names: &[&str]| -> PyResult<()> {
            for name in names {
                if !props.contains(*name)? {
                    return Err(PyValueError::new_err(format!(
                        "create({kind:?}) needs `{name}`"
                    )));
                }
            }
            Ok(())
        };
        let transparent = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
        let mut paint = transparent;
        let mut session = None;
        let node_kind = match kind {
            "box" => NodeKind::Rect,
            "path" => {
                require(&["data"])?;
                NodeKind::Path(PathState::new(PathData(BezPath::new())))
            }
            "text" => {
                require(&["text"])?;
                // A text's fill is its glyph color: opaque black, like CSS.
                paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
                NodeKind::Text(TextState {
                    content: String::new(),
                    font_family: "Roboto".to_string(),
                    font_weight: 400.0,
                    font_size: 16.0,
                    align: TextAlign::Start,
                    line_height: None,
                    options: Default::default(),
                })
            }
            "text_input" => {
                let mut state = TextFieldState::new("", "Roboto", 400.0, 16.0);
                state.text_tint = Animated::new(Color::from_rgba8(0, 0, 0, 255));
                NodeKind::TextField(state)
            }
            "image" => {
                require(&["rgba", "pixel_width", "pixel_height"])?;
                NodeKind::Image(ImageState::blank())
            }
            "scroll_view" => NodeKind::ScrollView(ScrollViewState::new(false)),
            "canvas" => {
                require(&["draw"])?;
                NodeKind::Canvas(CanvasState::new())
            }
            "virtual_list" => {
                require(&["item_count", "materialize"])?;
                if props.contains("item_extent")? == props.contains("size_hint")? {
                    return Err(PyValueError::new_err(
                        "create(\"virtual_list\") needs exactly one of `item_extent` or `size_hint`",
                    ));
                }
                NodeKind::VirtualList(VirtualListState::new(0, ItemExtent::Fixed(1.0)))
            }
            "terminal" => {
                require(&["shell", "cols", "rows"])?;
                let shell: String = props.get_item("shell")?.map_or(Ok(String::new()), |v| {
                    v.extract().map_err(|_| {
                        PyValueError::new_err("create(\"terminal\"): `shell` must be a str")
                    })
                })?;
                let scrollback: usize = match props.get_item("scrollback_lines")? {
                    Some(v) => v.extract().map_err(|_| {
                        PyValueError::new_err(
                            "create(\"terminal\"): `scrollback_lines` must be a non-negative int",
                        )
                    })?,
                    None => 1000,
                };
                props.del_item("shell")?;
                if props.contains("scrollback_lines")? {
                    props.del_item("scrollback_lines")?;
                }
                let cols: u16 = props
                    .get_item("cols")?
                    .and_then(|v| v.extract().ok())
                    .unwrap_or(1);
                let rows: u16 = props
                    .get_item("rows")?
                    .and_then(|v| v.extract().ok())
                    .unwrap_or(1);
                session = Some((shell, cols, rows, scrollback));
                NodeKind::Terminal(TerminalState::new(
                    cols.max(1),
                    rows.max(1),
                    MONOSPACE_FONT_FAMILY,
                    14.0,
                ))
            }
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown node kind {kind:?} -- create builds: {KINDS}"
                )));
            }
        };
        let changes = parse_all(Some(&props), &node_kind)?;
        let draws = matches!(node_kind, NodeKind::Canvas(_));
        let session = match session {
            Some((shell, cols, rows, scrollback)) => Some(
                TerminalSession::spawn(&shell, cols, rows, scrollback)
                    .map_err(|reason| EngineError::TerminalSpawnFailed { shell, reason })?,
            ),
            None => None,
        };
        let interactive = matches!(node_kind, NodeKind::TextField(_) | NodeKind::Terminal(_));
        let id = {
            let mut tree = self.handles.tree.borrow_mut();
            let id = tree.insert(node_kind, Style::default(), paint);
            if interactive {
                tree.set_access(
                    id,
                    AccessNodeData::new(Role::TextInput).with_action(Action::Focus),
                );
            }
            // M96: freed automatically once no handle points into it
            // (`node_handles`), until it's attached somewhere.
            tree.detach_collectible(id);
            id
        };
        let node = Node::from(NodeState {
            id,
            tree: self.handles.tree.clone(),
            handlers: self.handles.handlers.clone(),
            completions: self.handles.completions.clone(),
        });
        if let Some(session) = session {
            self.handles.terminals.borrow_mut().insert(id, session);
        }
        node.apply(changes);
        // 0.4.3 M15: a scroll view starts where `create` put it, so that's
        // what its first `scroll` event's `old_value` says, not 0.
        if let Some(NodeKind::ScrollView(state)) = self
            .handles
            .tree
            .borrow_mut()
            .get_mut(id)
            .map(|n| &mut n.kind)
        {
            state.reported = state.scroll.current;
        }
        if draws {
            node_callbacks::redraw(&node.tree, &node.handlers, id, py)?;
        }
        Ok(node)
    }

    /// Registers `handler` for the window event `event` (`resize`,
    /// `color_scheme`, `scale_factor`, `close_requested`, `closed`),
    /// replacing any earlier one. `handler` receives an `Event`, or
    /// nothing if it takes no parameters.
    fn on(&self, event: &str, handler: Py<PyAny>, py: Python<'_>) -> PyResult<()> {
        let event = WindowEventType::parse(event)?;
        let wants = wants_event(py, &handler)?;
        self.handles
            .window_listeners
            .borrow_mut()
            .insert(event, (handler, wants));
        Ok(())
    }

    /// Removes the window's listener for `event`, if any.
    fn off(&self, event: &str) -> PyResult<()> {
        let event = WindowEventType::parse(event)?;
        self.handles.window_listeners.borrow_mut().remove(&event);
        Ok(())
    }

    /// 0.5.0 M2 (issue #28): minimizes the window -- once `App.run()` opens
    /// it, if it isn't open yet.
    fn minimize(&self) {
        match self.handles.os_window.borrow().as_ref() {
            Some(window) => window.set_minimized(true),
            None => self.handles.minimized.set(true),
        }
    }

    /// 0.5.0 M2: maximizes the window -- or opens it maximized.
    fn maximize(&self) {
        match self.handles.os_window.borrow().as_ref() {
            Some(window) => window.set_maximized(true),
            None => self.handles.maximized.set(true),
        }
    }

    /// 0.5.0 M2: restores a minimized or maximized window to its normal
    /// size -- or, before `App.run()`, undoes `minimize()`/`maximize()`.
    fn restore(&self) {
        match self.handles.os_window.borrow().as_ref() {
            Some(window) => {
                if window.is_minimized() == Some(true) {
                    window.set_minimized(false);
                }
                if window.is_maximized() {
                    window.set_maximized(false);
                }
            }
            None => {
                self.handles.minimized.set(false);
                self.handles.maximized.set(false);
            }
        }
    }

    /// 0.5.0 M2: closes the window as if the user had: `close_requested`
    /// fires first, and a listener that cancels it keeps the window open.
    /// It happens on the loop's next turn, not during this call. A window
    /// that isn't open has nothing to close.
    fn close(&self) {
        let window = self.handles.os_window.borrow();
        if let (Some(window), Some(waker)) = (window.as_ref(), self.handles.waker.borrow().as_ref())
        {
            waker.close_window(window.id());
        }
    }

    /// Sets window properties: `title`, (0.4.0 M5) `partial_redraw`,
    /// (0.4.1 M8) `show_damage`, and (0.5.0 M2) `decorations`; `width`,
    /// `height`, and `scale_factor` are read-only.
    #[pyo3(signature = (**props))]
    fn set(&mut self, props: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let mut title = None;
        let mut partial_redraw = None;
        let mut show_damage = None;
        let mut decorations = None;
        let mut fullscreen = None;
        let (mut min_width, mut min_height) = (None, None);
        let mut icon = None;
        let mut resize_border = None;
        let mut system_menu = None;
        if let Some(props) = props {
            for (name, value) in props.iter() {
                let name: String = name.extract()?;
                match name.as_str() {
                    "title" => {
                        title = Some(value.extract::<String>().map_err(|_| {
                            PyValueError::new_err("window property `title` must be a str")
                        })?);
                    }
                    "partial_redraw" => {
                        partial_redraw = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("window property `partial_redraw` must be a bool")
                        })?);
                    }
                    "show_damage" => {
                        show_damage = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("window property `show_damage` must be a bool")
                        })?);
                    }
                    "decorations" => {
                        decorations = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("window property `decorations` must be a bool")
                        })?);
                    }
                    "fullscreen" => {
                        fullscreen = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("window property `fullscreen` must be a bool")
                        })?);
                    }
                    "min_width" => min_width = Some(parse_min_edge(&name, &value)?),
                    "min_height" => min_height = Some(parse_min_edge(&name, &value)?),
                    "resize_border" => resize_border = Some(parse_min_edge(&name, &value)?),
                    "system_menu" => {
                        system_menu = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("window property `system_menu` must be a bool")
                        })?);
                    }
                    "icon" => {
                        icon = Some(if value.is_none() {
                            None
                        } else {
                            Some(parse_icon(&value)?)
                        });
                    }
                    "width" | "height" | "scale_factor" | "maximized" | "minimized" | "active"
                    | "platform" | "titlebar_inset" | "native_controls" => {
                        return Err(PyValueError::new_err(format!(
                            "window property `{name}` is read-only -- settable: {SETTABLE}"
                        )));
                    }
                    _ => {
                        return Err(PyValueError::new_err(format!(
                            "unknown window property {name:?} -- settable: {SETTABLE}"
                        )));
                    }
                }
            }
        }
        if let Some(title) = title {
            if let Some(window) = self.handles.os_window.borrow().as_ref() {
                window.set_title(&title);
            }
            self.title = title;
        }
        if let Some(on) = partial_redraw {
            self.handles.partial_redraw.set(on);
        }
        if let Some(on) = show_damage {
            self.handles.show_damage.set(on);
        }
        if let Some(on) = decorations {
            if let Some(window) = self.handles.os_window.borrow().as_ref() {
                engine_platform::titlebar::set_decorations(window, on);
            }
            self.handles.decorations.set(on);
        }
        if let Some(border) = resize_border {
            self.handles.resize_border.set(border);
        }
        if let Some(on) = system_menu {
            self.handles.system_menu.set(on);
        }
        let window = self.handles.os_window.borrow();
        if let Some(on) = fullscreen {
            if let Some(window) = window.as_ref() {
                window.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
            }
            self.handles.fullscreen.set(on);
        }
        if min_width.is_some() || min_height.is_some() {
            let (w, h) = self.handles.min_size.get();
            let size = (min_width.unwrap_or(w), min_height.unwrap_or(h));
            self.handles.min_size.set(size);
            if let Some(window) = window.as_ref() {
                window.set_min_inner_size(
                    (size != (0.0, 0.0)).then(|| winit::dpi::LogicalSize::new(size.0, size.1)),
                );
                grow_to_minimum(&self.handles);
            }
        }
        if let Some(icon) = icon {
            if let Some(window) = window.as_ref() {
                window.set_window_icon(
                    icon.clone()
                        .and_then(|(rgba, w, h)| winit::window::Icon::from_rgba(rgba, w, h).ok()),
                );
            }
            *self.handles.icon.borrow_mut() = icon;
        }
        Ok(())
    }

    /// Reads a window property: `width`, `height`, `title`,
    /// `scale_factor` (`1.0` until `App.run()` opens the window),
    /// `dark` -- M106 (issue #18): the OS's current appearance, `True`
    /// for dark, or `None` where the platform can't say (on macOS and
    /// Windows, until `App.run()` opens the window) -- `partial_redraw`,
    /// (0.4.0) `partial_redraw_active`, or (0.4.1) `show_damage`.
    fn get(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match name {
            "width" => f64::from(self.handles.width.get())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "height" => f64::from(self.handles.height.get())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "title" => self.title.clone().into_pyobject(py)?.into_any().unbind(),
            "partial_redraw" => self
                .handles
                .partial_redraw
                .get()
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            // 0.4.0 M6: whether the open window really redraws only what
            // changed -- the setting, and a surface that allows it; `None`
            // until `App.run()` opens the window.
            // 0.5.0 M2: the open window's own answer, else the setting.
            // 0.5.0 M2: the open window's own answer, else the last known
            // state -- or, before `App.run()`, what it opens as.
            "maximized" => {
                let window = self.handles.os_window.borrow();
                window
                    .as_ref()
                    .map_or(self.handles.maximized.get(), |w| w.is_maximized())
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()
                    .unbind()
            }
            "minimized" => {
                let window = self.handles.os_window.borrow();
                window
                    .as_ref()
                    .and_then(|w| w.is_minimized())
                    .unwrap_or(self.handles.minimized.get())
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()
                    .unbind()
            }
            "active" => {
                let window = self.handles.os_window.borrow();
                window
                    .as_ref()
                    .map_or(self.handles.active.get(), |w| w.has_focus())
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()
                    .unbind()
            }
            "fullscreen" => {
                let window = self.handles.os_window.borrow();
                window
                    .as_ref()
                    .map_or(self.handles.fullscreen.get(), |w| w.fullscreen().is_some())
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()
                    .unbind()
            }
            "min_width" => self
                .handles
                .min_size
                .get()
                .0
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "system_menu" => self
                .handles
                .system_menu
                .get()
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            "resize_border" => self
                .handles
                .resize_border
                .get()
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "min_height" => self
                .handles
                .min_size
                .get()
                .1
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "platform" => {
                let window = self.handles.os_window.borrow();
                engine_platform::platform_name(window.as_deref())
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
            // On macOS `winit` keeps an undecorated window decorated, under
            // an overlay title bar, so the setting answers there.
            "decorations" => self
                .handles
                .os_window
                .borrow()
                .as_ref()
                .filter(|_| !engine_platform::titlebar::OVERLAY_TITLEBAR)
                .map_or(self.handles.decorations.get(), |window| {
                    window.is_decorated()
                })
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            // 0.5.0 M4: the open window's own answer, else the last known.
            "titlebar_inset" => {
                let window = self.handles.os_window.borrow();
                window
                    .as_deref()
                    .map_or(
                        self.handles.titlebar_inset.get(),
                        engine_platform::titlebar::titlebar_inset,
                    )
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
            "native_controls" => native_controls(&self.handles)
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            "show_damage" => self
                .handles
                .show_damage
                .get()
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            "partial_redraw_active" => self
                .handles
                .surface_partial
                .get()
                .map(|allowed| allowed && self.handles.partial_redraw.get())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "scale_factor" => self
                .handles
                .os_window
                .borrow()
                .as_ref()
                .map_or(1.0, |window| window.scale_factor())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "dark" => {
                let window = self.handles.os_window.borrow().clone();
                engine_platform::appearance::current_dark(window.as_deref())
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown window property {name:?} -- valid: width, height, title, \
                     scale_factor, dark, partial_redraw, partial_redraw_active, show_damage, \
                     decorations, maximized, minimized, active, fullscreen, min_width, \
                     min_height, platform, resize_border, system_menu, titlebar_inset, \
                     native_controls"
                )));
            }
        })
    }

    /// M96: the size `text` takes, as `(width, height)`, laid out exactly
    /// as a text node with these properties paints it -- wrapped within
    /// `max_width` when given, cut to `max_lines`. For sizing a widget to
    /// its content; a text node has no size of its own.
    #[pyo3(signature = (
        text, font_family="Roboto", font_size=16.0, font_weight=400.0, font_style="normal",
        letter_spacing=0.0, line_height=None, max_width=None, wrap="word", max_lines=None,
        overflow="clip",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn measure_text(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: f32,
        font_style: &str,
        letter_spacing: f32,
        line_height: Option<f32>,
        max_width: Option<f32>,
        wrap: &str,
        max_lines: Option<usize>,
        overflow: &str,
    ) -> PyResult<(f64, f64)> {
        let word = |table: &[(&str, bool)], name: &str, value: &str| {
            crate::node_layout::lookup(table, value).map_err(|expected| {
                PyValueError::new_err(format!("measure_text: `{name}` must be {expected}"))
            })
        };
        let options = TextOptions {
            italic: word(&FONT_STYLE, "font_style", font_style)?,
            letter_spacing,
            wrap: word(&WRAP, "wrap", wrap)?,
            max_lines: max_lines.filter(|n| *n > 0),
            ellipsis: word(&OVERFLOW, "overflow", overflow)?,
        };
        if !(font_size > 0.0 && font_size.is_finite()) {
            return Err(PyValueError::new_err(
                "measure_text: `font_size` must be a positive number",
            ));
        }
        let font = FontSpec {
            family: font_family,
            weight: font_weight,
            size: font_size,
            line_height,
            options: &options,
        };
        let (width, height) = crate::shaper::with(|shaper| shaper.measure(text, &font, max_width));
        Ok((f64::from(width), f64::from(height)))
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
        let (tree, root, handlers) = (
            self.handles.tree.clone(),
            self.handles.root,
            self.handles.handlers.clone(),
        );
        let now = clock::advance(&tree, Duration::from_secs_f64(ms / 1000.0));
        let (_, completed) = tree.borrow_mut().tick_all(now);
        run_completions(&self.handles.completions, completed, py);
        node_callbacks::layout(&tree, root, self.available(), &handlers, py);
        listeners::fire_scroll_changes(
            &NodeContext {
                tree: &tree,
                handlers: &handlers,
                completions: &self.handles.completions,
            },
            py,
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
        let (tree, root, handlers) = (
            self.handles.tree.clone(),
            self.handles.root,
            self.handles.handlers.clone(),
        );
        let node_id = match &node {
            Some(node) if !Rc::ptr_eq(&node.tree, &tree) => {
                return Err(EngineError::ForeignNode.into());
            }
            Some(node) => Some(node.id),
            None => None,
        };
        node_callbacks::layout(&tree, root, self.available(), &handlers, py);
        let ctx = NodeContext {
            tree: &tree,
            handlers: &handlers,
            completions: &self.handles.completions,
        };
        let io = WindowIo {
            dock: &self.handles.dock,
            listeners: &self.handles.window_listeners,
            terminals: &self.handles.terminals,
            window: &self.handles,
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
                        process_input(&ctx, &io, root, input, py);
                    }
                });
            }
            "pointer_leave" => {
                f.done()?;
                process_input(&ctx, &io, root, &InputEvent::PointerLeft, py);
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
                    && modifiers.ctrl
                    && let (Some(letter), None) = (key.chars().next(), key.chars().nth(1))
                    && let Some(shortcut) = engine_core::ctrl_shortcut(letter, modifiers.shift)
                {
                    // M100: Ctrl+letter means what it means live -- copy,
                    // cut, paste, select all, a terminal's control byte.
                    inputs.push(shortcut);
                } else if pressed
                    && key.chars().count() == 1
                    && !(modifiers.ctrl || modifiers.alt || modifiers.meta)
                {
                    inputs.push(InputEvent::TextInput(key));
                }
                with_modifiers(modifiers, || {
                    for input in &inputs {
                        process_input(&ctx, &io, root, input, py);
                    }
                });
            }
            "input" => {
                let text = f.string("text")?;
                let text = f.required("text", text)?;
                f.done()?;
                process_input(&ctx, &io, root, &InputEvent::TextInput(text), py);
            }
            "focus" | "unfocus" => {
                let id = need_node(&f)?;
                f.done()?;
                let transition = if event == "focus" {
                    tree.borrow_mut().set_focus_to(id)
                } else if tree.borrow().focused() == Some(id) {
                    tree.borrow_mut().clear_focus()
                } else {
                    None
                };
                if let Some((old, new)) = transition {
                    fire_focus_transition(
                        &handlers,
                        &tree,
                        &self.handles.completions,
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
                listeners::fire_scroll_changes(&ctx, py);
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
                if !(width.is_finite() && height.is_finite() && width >= 0.0 && height >= 0.0) {
                    return Err(PyValueError::new_err(
                        "`resize` needs a finite, non-negative width and height",
                    ));
                }
                self.handles.width.set(width as u32);
                self.handles.height.set(height as u32);
                tree.borrow_mut().dispatch(
                    root,
                    InputEvent::Resized {
                        width: width as f32,
                        height: height as f32,
                    },
                    crate::clock::now(&tree),
                );
                listeners::deliver_window(
                    &self.handles.window_listeners,
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
                listeners::deliver_window(
                    &self.handles.window_listeners,
                    py,
                    WindowEventType::ColorScheme,
                    |e| e.dark = Some(dark),
                );
            }
            // 0.5.0 M2: what the live window reports when it's maximized or
            // restored, or gains or loses focus -- the state changes, and
            // the event fires if it changed.
            "maximized" | "active" => {
                let value = f.bool(event)?;
                let value = f.required(event, value)?;
                f.done()?;
                let (cell, event_type) = if event == "maximized" {
                    (&self.handles.maximized, WindowEventType::Maximized)
                } else {
                    (&self.handles.active, WindowEventType::Active)
                };
                listeners::update_window_state(
                    &self.handles.window_listeners,
                    py,
                    cell,
                    value,
                    event_type,
                );
            }
            // 0.5.0 M4: what the live window reports when its title bar's
            // controls take a different area -- a macOS window entering or
            // leaving fullscreen, say.
            "titlebar_inset" => {
                let height = f.f64("height")?;
                let height = f.required("height", height)?;
                let width = f.f64("width")?;
                let width = f.required("width", width)?;
                f.done()?;
                listeners::update_titlebar_inset(
                    &self.handles.window_listeners,
                    py,
                    &self.handles.titlebar_inset,
                    (height, width),
                );
            }
            "scale_factor" => {
                let scale_factor = f.f64("scale_factor")?;
                let scale_factor = f.required("scale_factor", scale_factor)?;
                f.done()?;
                listeners::deliver_window(
                    &self.handles.window_listeners,
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
                listeners::deliver_window(&self.handles.window_listeners, py, event_type, |_| {});
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
