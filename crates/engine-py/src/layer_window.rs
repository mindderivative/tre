//! 0.5.7 (#198): `LayerWindow`, a `Window` whose Wayland role is a layer surface.
//!
//! A bar, panel, dock or OSD: the compositor places it (an edge, a stacking layer, a
//! size, margins) and it can reserve screen space. It is a `Window` in every way that
//! makes sense (`root`, `create`, `show_layer`, `after`, `every`, `on`, `simulate`,
//! `advance`, `snapshot`), built on the same tree and renderer; what differs is the
//! settings in [`engine_platform::layer_config::LayerConfig`], how it is opened (the
//! layer thread in `engine-platform`), and the window keys that mean nothing to it.
//! See `docs/design/layer-window.md`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use engine_platform::layer_config::{
    Anchors, Availability, ConfigError, KeyboardMode, LayerConfig, LayerKind, availability,
    parse_anchor,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyString, PyTuple};

use crate::listeners::{self, WindowEventType};
use crate::window::PyWindow;

pyo3::create_exception!(
    tre,
    UnsupportedPlatform,
    PyRuntimeError,
    "A feature this platform does not have: `LayerWindow` needs a Wayland compositor with the \
     layer-shell protocol (KDE's KWin and wlroots' compositors have it; GNOME's Mutter, X11, \
     Windows and macOS do not)."
);

/// The logical size a layer window has before a compositor sizes it, on an axis that
/// stretches: what a headless `snapshot` draws, and what `resize` changes.
const HEADLESS_WIDTH: u32 = 1920;
const HEADLESS_HEIGHT: u32 = 1080;

/// Window keys with no meaning for a surface the compositor places.
const REFUSED: &[&str] = &[
    "title",
    "decorations",
    "maximized",
    "minimized",
    "fullscreen",
    "resize_border",
    "system_menu",
    "min_width",
    "min_height",
    "icon",
    "x",
    "y",
    "always_on_top",
    "resizable",
    "skip_taskbar",
    "blur_behind",
];

/// The keys `set()` changes on the layer surface itself.
const LIVE: &[&str] = &[
    "layer",
    "anchor",
    "size",
    "exclusive_zone",
    "exclusive_edge",
    "margin",
    "keyboard",
];

/// A layer window's state, shared with the run loop.
pub(crate) struct LayerState {
    pub(crate) config: RefCell<LayerConfig>,
    /// The surface's scale (physical pixels per logical one), from the compositor.
    pub(crate) scale: Cell<f64>,
    /// The loop's number for the surface once it exists.
    pub(crate) id: Cell<Option<u64>>,
    /// Why the surface closed: `"compositor"`, `"output_removed"` or `"app"`.
    pub(crate) closed_reason: RefCell<Option<&'static str>>,
    /// Warnings from the constructor, delivered as events once the window opens.
    pub(crate) pending_warnings: RefCell<Vec<(String, String)>>,
    #[cfg(target_os = "linux")]
    pub(crate) controller: RefCell<Option<engine_platform::layer::LayerController>>,
}

impl LayerState {
    fn new(config: LayerConfig, warnings: Vec<(String, String)>) -> Self {
        Self {
            config: RefCell::new(config),
            scale: Cell::new(1.0),
            id: Cell::new(None),
            closed_reason: RefCell::new(None),
            pending_warnings: RefCell::new(warnings),
            #[cfg(target_os = "linux")]
            controller: RefCell::new(None),
        }
    }
}

fn value_error(error: &ConfigError) -> PyErr {
    PyValueError::new_err(format!("LayerWindow {error}"))
}

fn names_error(key: &str, what: &str, names: &[&str]) -> PyErr {
    PyValueError::new_err(format!(
        "LayerWindow {key}: unknown {what}; valid: {}",
        names.join(", ")
    ))
}

fn parse_layer(name: &str) -> PyResult<LayerKind> {
    LayerKind::parse(name).ok_or_else(|| {
        names_error("layer", &format!("layer {name:?}"), &LayerKind::NAMES)
    })
}

fn parse_keyboard(name: &str) -> PyResult<KeyboardMode> {
    KeyboardMode::parse(name).ok_or_else(|| {
        names_error("keyboard", &format!("mode {name:?}"), &KeyboardMode::NAMES)
    })
}

/// `anchor` as a bit set: a sequence of edge names, or one name.
fn extract_anchor(value: &Bound<'_, PyAny>) -> PyResult<Anchors> {
    let names: Vec<String> = if let Ok(one) = value.cast::<PyString>() {
        vec![one.to_string()]
    } else {
        value.extract().map_err(|_| {
            PyValueError::new_err("LayerWindow anchor: must be a sequence of edge names")
        })?
    };
    parse_anchor(names.iter().map(String::as_str)).map_err(|e| value_error(&e))
}

fn extract_edge(key: &'static str, value: &Bound<'_, PyAny>) -> PyResult<Option<u8>> {
    if value.is_none() {
        return Ok(None);
    }
    let name: String = value
        .extract()
        .map_err(|_| PyValueError::new_err(format!("LayerWindow {key}: must be an edge name or None")))?;
    Anchors::edge(&name)
        .map(Some)
        .ok_or_else(|| names_error(key, &format!("edge {name:?}"), &Anchors::NAMES))
}

fn extract_size(value: &Bound<'_, PyAny>) -> PyResult<(u32, u32)> {
    value.extract::<(u32, u32)>().map_err(|_| {
        PyValueError::new_err(
            "LayerWindow size: must be (width, height), two whole numbers of pixels, zero or more",
        )
    })
}

fn extract_margin(value: &Bound<'_, PyAny>) -> PyResult<(i32, i32, i32, i32)> {
    if let Ok(all) = value.extract::<i32>() {
        return Ok((all, all, all, all));
    }
    value.extract::<(i32, i32, i32, i32)>().map_err(|_| {
        PyValueError::new_err(
            "LayerWindow margin: must be (top, right, bottom, left) in pixels, or one number for all four",
        )
    })
}

fn extract_zone(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    value.extract::<i32>().map_err(|_| {
        PyValueError::new_err("LayerWindow exclusive_zone: must be a whole number of pixels, 0, or -1")
    })
}

/// The edge a bit names.
fn edge_name(bit: u8) -> &'static str {
    match bit {
        Anchors::TOP => "top",
        Anchors::BOTTOM => "bottom",
        Anchors::LEFT => "left",
        _ => "right",
    }
}

/// A layer window's logical size before a compositor assigns one.
fn headless_size(config: &LayerConfig) -> (u32, u32) {
    (
        if config.size.0 == 0 { HEADLESS_WIDTH } else { config.size.0 },
        if config.size.1 == 0 { HEADLESS_HEIGHT } else { config.size.1 },
    )
}

fn warnings_of(config: &LayerConfig) -> PyResult<Vec<(String, String)>> {
    config
        .validate()
        .map(|warnings| {
            warnings
                .into_iter()
                .map(|w| (w.key.to_string(), w.message))
                .collect()
        })
        .map_err(|e| value_error(&e))
}

/// A window the compositor places: a bar, a panel, a dock, an OSD.
#[pyclass(name = "LayerWindow", extends = PyWindow)]
pub struct PyLayerWindow;

#[pymethods]
impl PyLayerWindow {
    #[new]
    #[pyo3(signature = (
        namespace,
        *,
        layer = "top",
        anchor = None,
        size = None,
        exclusive_zone = None,
        exclusive_edge = None,
        margin = None,
        keyboard = "none",
        output = None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        namespace: &str,
        layer: &str,
        anchor: Option<&Bound<'_, PyAny>>,
        size: Option<&Bound<'_, PyAny>>,
        exclusive_zone: Option<&Bound<'_, PyAny>>,
        exclusive_edge: Option<&Bound<'_, PyAny>>,
        margin: Option<&Bound<'_, PyAny>>,
        keyboard: &str,
        output: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        match availability() {
            Availability::Unsupported(why) => return Err(UnsupportedPlatform::new_err(why)),
            Availability::Supported | Availability::Headless => {}
        }
        let config = LayerConfig {
            namespace: namespace.to_string(),
            layer: parse_layer(layer)?,
            anchor: anchor.map(extract_anchor).transpose()?.unwrap_or_default(),
            size: size.map(extract_size).transpose()?.unwrap_or((0, 0)),
            exclusive_zone: exclusive_zone.map(extract_zone).transpose()?.unwrap_or(0),
            exclusive_edge: exclusive_edge
                .map(|v| extract_edge("exclusive_edge", v))
                .transpose()?
                .flatten(),
            margin: margin.map(extract_margin).transpose()?.unwrap_or((0, 0, 0, 0)),
            keyboard: parse_keyboard(keyboard)?,
            output,
        };
        if config.exclusive_edge.is_some() {
            return Err(PyValueError::new_err(
                "LayerWindow exclusive_edge: not supported yet (it needs layer-shell protocol \
                 version 5, which this build does not bind)",
            ));
        }
        let warnings = warnings_of(&config)?;
        let (width, height) = headless_size(&config);
        let mut base = PyWindow::construct(width, height, namespace, false, 0.0, 0.0)?;
        let handles = &mut base.handles;
        // Layout is in logical pixels, drawn at the surface's scale; transparent, so
        // a rounded or floating bar needs no background rectangle.
        handles.dpi_scaling.set(true);
        handles.transparent.set(true);
        handles.layer = Some(Rc::new(LayerState::new(config, warnings)));
        Ok(PyClassInitializer::from(base).add_subclass(Self))
    }

    /// Sets properties, the layer ones in one commit: `layer`, `anchor`, `size`,
    /// `exclusive_zone`, `margin`, `keyboard`; the rest as `Window.set` for the keys that
    /// mean something here. A bad value raises and changes nothing.
    #[pyo3(signature = (**props))]
    fn set(slf: &Bound<'_, Self>, props: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let py = slf.py();
        let base = slf.as_super().borrow();
        let state = base
            .handles
            .layer
            .clone()
            .expect("a LayerWindow has layer state");
        let mut config = state.config.borrow().clone();
        let rest = PyDict::new(py);
        if let Some(props) = props {
            for (name, value) in props.iter() {
                let name: String = name.extract()?;
                match name.as_str() {
                    "layer" => {
                        let name: String = value.extract().map_err(|_| {
                            PyValueError::new_err("LayerWindow layer: must be a str")
                        })?;
                        config.layer = parse_layer(&name)?;
                    }
                    "anchor" => config.anchor = extract_anchor(&value)?,
                    "size" => config.size = extract_size(&value)?,
                    "exclusive_zone" => config.exclusive_zone = extract_zone(&value)?,
                    "exclusive_edge" => {
                        if !value.is_none() {
                            return Err(PyValueError::new_err(
                                "LayerWindow exclusive_edge: not supported yet (it needs \
                                 layer-shell protocol version 5)",
                            ));
                        }
                        config.exclusive_edge = None;
                    }
                    "margin" => config.margin = extract_margin(&value)?,
                    "keyboard" => {
                        let name: String = value.extract().map_err(|_| {
                            PyValueError::new_err("LayerWindow keyboard: must be a str")
                        })?;
                        config.keyboard = parse_keyboard(&name)?;
                    }
                    "namespace" | "output" => {
                        return Err(PyValueError::new_err(format!(
                            "LayerWindow {name}: can only be given to the constructor"
                        )));
                    }
                    n if REFUSED.contains(&n) => {
                        return Err(PyValueError::new_err(format!(
                            "LayerWindow {name}: means nothing to a surface the compositor \
                             places (settable here: {}, and the keys Window shares)",
                            LIVE.join(", ")
                        )));
                    }
                    _ => rest.set_item(name, value)?,
                }
            }
        }
        let warnings = warnings_of(&config)?;
        // The shared keys first: Window.set is itself all-or-nothing, so an error
        // here leaves the layer settings alone too.
        if !rest.is_empty() {
            base.set(Some(&rest))?;
        }
        let layer_keys_given = props.is_some_and(|p| {
            p.keys()
                .iter()
                .filter_map(|k| k.extract::<String>().ok())
                .any(|k| LIVE.contains(&k.as_str()))
        });
        if layer_keys_given {
            *state.config.borrow_mut() = config.clone();
            #[cfg(target_os = "linux")]
            if let (Some(controller), Some(id)) =
                (state.controller.borrow().as_ref(), state.id.get())
            {
                controller.update(id, config.clone());
            }
            for (key, message) in warnings {
                listeners::deliver_window(
                    &base.handles.window_listeners,
                    py,
                    WindowEventType::Warning,
                    |e| {
                        e.key = Some(key.clone());
                        e.message = Some(message.clone());
                    },
                );
            }
        }
        Ok(())
    }

    /// Reads a property: the layer ones, or as `Window.get`.
    fn get(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let base = slf.as_super().borrow();
        let state = base
            .handles
            .layer
            .clone()
            .expect("a LayerWindow has layer state");
        let config = state.config.borrow();
        Ok(match name {
            "namespace" => config.namespace.clone().into_pyobject(py)?.into_any().unbind(),
            "layer" => config.layer.name().into_pyobject(py)?.into_any().unbind(),
            "anchor" => PyTuple::new(py, config.anchor.names())?.into_any().unbind(),
            "size" => config.size.into_pyobject(py)?.into_any().unbind(),
            "exclusive_zone" => config.exclusive_zone.into_pyobject(py)?.into_any().unbind(),
            "exclusive_edge" => config
                .exclusive_edge
                .map(edge_name)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "margin" => config.margin.into_pyobject(py)?.into_any().unbind(),
            "keyboard" => config.keyboard.name().into_pyobject(py)?.into_any().unbind(),
            "output" => config.output.clone().into_pyobject(py)?.into_any().unbind(),
            _ => {
                drop(config);
                base.get(name, py)?
            }
        })
    }

    /// Closes the surface: `close_requested` first (a listener may cancel it with
    /// `event.cancel()`), then `closed` with reason `"app"`. It happens on the loop's next
    /// turn. A window that is not open has nothing to close.
    fn close(slf: &Bound<'_, Self>) {
        #[cfg(target_os = "linux")]
        {
            let base = slf.as_super().borrow();
            if let (Some(layer), Some(waker)) = (&base.handles.layer, base.handles.waker.borrow().as_ref())
                && let Some(id) = layer.id.get()
            {
                waker.close_window(engine_platform::layer_window_id(id));
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = slf;
    }
}

/// `tre.supports(feature)`: whether this process can use an optional feature. Only
/// `"layer_shell"` so far; an unknown name is `False`.
#[pyfunction]
pub(crate) fn supports(feature: &str) -> bool {
    match feature {
        "layer_shell" => availability() == Availability::Supported,
        _ => false,
    }
}
