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
use pyo3::types::{PyAny, PyDict, PyList, PyString, PyTuple};

use crate::listeners::{self, WindowEventType};
use crate::window::PyWindow;
use engine_core::{Animated, CompletionHandle, MotionCurve};
use std::time::{Duration, Instant};

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
    "input_region",
];

/// A layer window's state, shared with the run loop.
pub(crate) struct LayerState {
    pub(crate) config: RefCell<LayerConfig>,
    /// The surface's scale (physical pixels per logical one), from the compositor.
    pub(crate) scale: Cell<f64>,
    /// The loop's number for the surface once it exists.
    pub(crate) id: Cell<Option<u64>>,
    /// The logical size last reported by `configure`; a compositor may re-send the same
    /// size (sway does), which is not a change.
    pub(crate) last_configure: Cell<Option<(f64, f64)>>,
    /// `input_region="auto"`: the region follows the drawn shapes, recomputed after layout.
    pub(crate) auto_region: Cell<bool>,
    /// The four margins `(top, right, bottom, left)` as animated numbers: what `animate`
    /// moves and `set(margin=...)` snaps. The surface gets the rounded values.
    pub(crate) motion: RefCell<[Animated<f64>; 4]>,
    /// Why the surface closed: `"compositor"`, `"output_removed"` or `"app"`.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) closed_reason: RefCell<Option<&'static str>>,
    /// Warnings from the constructor, delivered as events once the window opens.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) pending_warnings: RefCell<Vec<(String, String)>>,
    #[cfg(target_os = "linux")]
    pub(crate) controller: RefCell<Option<engine_platform::layer::LayerController>>,
}

impl LayerState {
    /// Makes the margin motion hold `margin` at rest, cancelling any running animation (a
    /// cancelled one ends with `finished=False` and never calls `on_complete`).
    pub(crate) fn snap_margin(&self, margin: (i32, i32, i32, i32), now: Instant) {
        let values = [margin.0, margin.1, margin.2, margin.3];
        for (field, value) in self.motion.borrow_mut().iter_mut().zip(values) {
            let value = f64::from(value);
            field.animate_to(value, Duration::ZERO, MotionCurve::Linear, now);
            field.active = None;
            field.current = value;
        }
    }

    /// Starts or retargets one margin's animation, from its current value.
    pub(crate) fn animate_margin(
        &self,
        index: usize,
        to: f64,
        duration: Duration,
        curve: MotionCurve,
        now: Instant,
        handle: CompletionHandle,
    ) {
        self.motion.borrow_mut()[index]
            .animate_to_with_completion(to, duration, curve, now, handle);
    }

    /// Advances the margins to `now`, pushing finished animations' handles onto
    /// `completed`; when the rounded margin changed, sends it to the surface (one cheap
    /// commit). Returns whether any margin is still moving.
    pub(crate) fn tick_margins(&self, now: Instant, completed: &mut Vec<CompletionHandle>) -> bool {
        let mut motion = self.motion.borrow_mut();
        let mut active = false;
        let mut before = [0.0; 4];
        for (i, field) in motion.iter().enumerate() {
            before[i] = field.current;
        }
        for field in motion.iter_mut() {
            active |= field.tick(now, completed);
        }
        let moved = motion.iter().zip(before).any(|(f, b)| f.current != b);
        if moved {
            let round = |v: f64| v.round() as i32;
            let margin = (
                round(motion[0].current),
                round(motion[1].current),
                round(motion[2].current),
                round(motion[3].current),
            );
            let mut config = self.config.borrow_mut();
            if config.margin != margin {
                config.margin = margin;
                #[cfg(target_os = "linux")]
                if let (Some(controller), Some(id)) =
                    (self.controller.borrow().as_ref(), self.id.get())
                {
                    controller.set_margin(id, margin);
                }
            }
        }
        active
    }
    /// `input_region="auto"`: recomputes the region from the drawn shapes after a layout and
    /// sends it if it changed. While something animates the new region is joined to the
    /// last one, so input never lands outside what is visible; it snaps to the exact
    /// region once the animation ends.
    #[cfg(target_os = "linux")]
    pub(crate) fn update_auto_region(
        &self,
        tree: &RefCell<engine_core::Tree>,
        root: engine_core::NodeId,
        bounds: (f64, f64),
        animating: bool,
    ) {
        use engine_platform::layer_config::MAX_REGION_RECTS;
        let mut rects = tree.borrow().input_region(root, bounds, MAX_REGION_RECTS);
        let last = self
            .config
            .borrow()
            .input_region
            .clone()
            .unwrap_or_default();
        if animating {
            for r in &last {
                if !rects.contains(r) {
                    rects.push(*r);
                }
            }
            if rects.len() > MAX_REGION_RECTS {
                let (x0, y0) = rects
                    .iter()
                    .fold((i32::MAX, i32::MAX), |a, r| (a.0.min(r.0), a.1.min(r.1)));
                let (x1, y1) = rects.iter().fold((i32::MIN, i32::MIN), |a, r| {
                    (a.0.max(r.0 + r.2), a.1.max(r.1 + r.3))
                });
                rects = vec![(x0, y0, x1 - x0, y1 - y0)];
            }
        }
        if rects == last {
            return;
        }
        self.config.borrow_mut().input_region = Some(rects.clone());
        if let (Some(controller), Some(id)) = (self.controller.borrow().as_ref(), self.id.get()) {
            controller.set_input_region(id, Some(rects));
        }
    }

    fn new(config: LayerConfig, warnings: Vec<(String, String)>) -> Self {
        Self {
            config: RefCell::new(config),
            scale: Cell::new(1.0),
            id: Cell::new(None),
            last_configure: Cell::new(None),
            auto_region: Cell::new(false),
            motion: RefCell::new([0, 1, 2, 3].map(|_| Animated::new(0.0))),
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
    LayerKind::parse(name)
        .ok_or_else(|| names_error("layer", &format!("layer {name:?}"), &LayerKind::NAMES))
}

fn parse_keyboard(name: &str) -> PyResult<KeyboardMode> {
    KeyboardMode::parse(name)
        .ok_or_else(|| names_error("keyboard", &format!("mode {name:?}"), &KeyboardMode::NAMES))
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
    let name: String = value.extract().map_err(|_| {
        PyValueError::new_err(format!("LayerWindow {key}: must be an edge name or None"))
    })?;
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

/// `output`: an `Output`, its connector name, or `None`.
fn extract_output(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(output) = value.extract::<PyRef<'_, crate::outputs::PyOutput>>() {
        return Ok(Some(output.info.name.clone()));
    }
    value.extract::<String>().map(Some).map_err(|_| {
        PyValueError::new_err("LayerWindow output: must be an Output, a connector name, or None")
    })
}

fn extract_zone(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    value.extract::<i32>().map_err(|_| {
        PyValueError::new_err(
            "LayerWindow exclusive_zone: must be a whole number of pixels, 0, or -1",
        )
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

/// `input_region`: `None` (the whole surface), `[]` (nothing), or rectangles
/// `[(x, y, width, height), ...]` in logical pixels.
fn extract_region(value: &Bound<'_, PyAny>) -> PyResult<Region> {
    if value.is_none() {
        return Ok(Region::Rects(None));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return if text == "auto" {
            Ok(Region::Auto)
        } else {
            Err(PyValueError::new_err(format!(
                "LayerWindow input_region: {:?} is not a region; use None, [], \"auto\" or a \
                 list of (x, y, width, height) rectangles",
                text.to_string()
            )))
        };
    }
    let bad = || {
        PyValueError::new_err(
            "LayerWindow input_region: must be None, [], \"auto\" or a list of \
             (x, y, width, height) rectangles",
        )
    };
    // Each rectangle may be a tuple or a list of four numbers.
    let rows: Vec<Vec<f64>> = value.extract().map_err(|_| bad())?;
    let rects: Vec<(f64, f64, f64, f64)> = rows
        .into_iter()
        .map(|r| match r[..] {
            [x, y, w, h] => Ok((x, y, w, h)),
            _ => Err(bad()),
        })
        .collect::<PyResult<_>>()?;
    engine_platform::layer_config::region_from_floats(&rects)
        .map(|r| Region::Rects(Some(r)))
        .map_err(|e| value_error(&e))
}

/// What `input_region` was asked to be.
#[derive(Clone, PartialEq)]
enum Region {
    /// `None` (the whole surface), `[]` or rectangles.
    Rects(Option<Vec<engine_platform::layer_config::Rect>>),
    /// `"auto"`: follows what is drawn.
    Auto,
}

/// The region after an `input_region` and/or `click_through` was given: `click_through`
/// is an alias for `input_region=[]`, and the two must not disagree.
fn resolve_region(
    current: Region,
    input_region: Option<&Bound<'_, PyAny>>,
    click_through: Option<bool>,
) -> PyResult<Region> {
    let region = match input_region {
        Some(value) => extract_region(value)?,
        None => current.clone(),
    };
    let nothing = region == Region::Rects(Some(Vec::new()));
    match (click_through, input_region.is_some()) {
        (Some(through), true) if through != nothing => Err(PyValueError::new_err(
            "LayerWindow click_through and input_region disagree: click_through=True is \
             input_region=[]",
        )),
        (Some(true), false) => Ok(Region::Rects(Some(Vec::new()))),
        // Clearing the alias gives the whole surface back, not a stale empty region.
        (Some(false), false) if nothing => Ok(Region::Rects(None)),
        _ => Ok(region),
    }
}

/// A layer window's logical size before a compositor assigns one.
fn headless_size(config: &LayerConfig) -> (u32, u32) {
    (
        if config.size.0 == 0 {
            HEADLESS_WIDTH
        } else {
            config.size.0
        },
        if config.size.1 == 0 {
            HEADLESS_HEIGHT
        } else {
            config.size.1
        },
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
        output = None,
        input_region = None,
        click_through = None
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
        output: Option<&Bound<'_, PyAny>>,
        input_region: Option<&Bound<'_, PyAny>>,
        click_through: Option<bool>,
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
            margin: margin
                .map(extract_margin)
                .transpose()?
                .unwrap_or((0, 0, 0, 0)),
            keyboard: parse_keyboard(keyboard)?,
            output: output.map(extract_output).transpose()?.flatten(),
            input_region: None,
        };
        let region = resolve_region(Region::Rects(None), input_region, click_through)?;
        let mut config = config;
        let auto = region == Region::Auto;
        config.input_region = match region {
            Region::Rects(rects) => rects,
            // Nothing takes input until the first layout says what is drawn.
            Region::Auto => Some(Vec::new()),
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
        let layer = LayerState::new(config, warnings);
        layer.auto_region.set(auto);
        layer.snap_margin(layer.config.borrow().margin, Instant::now());
        handles.layer = Some(Rc::new(layer));
        handles.tree.borrow_mut().set_window_drag_allowed(false);
        handles.tree.borrow_mut().set_margin_drag_allowed(true);
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
        let mut region_given = None;
        let mut margin_given = false;
        let mut auto_now = None;
        let mut through_given = None;
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
                    "margin" => {
                        config.margin = extract_margin(&value)?;
                        margin_given = true;
                    }
                    "input_region" => region_given = Some(value),
                    "click_through" => {
                        through_given = Some(value.extract::<bool>().map_err(|_| {
                            PyValueError::new_err("LayerWindow click_through: must be a bool")
                        })?);
                    }
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
        if region_given.is_some() || through_given.is_some() {
            let current = if state.auto_region.get() {
                Region::Auto
            } else {
                Region::Rects(config.input_region.clone())
            };
            let region = resolve_region(current, region_given.as_ref(), through_given)?;
            auto_now = Some(region == Region::Auto);
            config.input_region = match region {
                Region::Rects(rects) => rects,
                Region::Auto => Some(Vec::new()),
            };
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
                .any(|k| LIVE.contains(&k.as_str()) || k == "click_through")
        });
        if layer_keys_given {
            if let Some(auto) = auto_now {
                state.auto_region.set(auto);
                // An auto region starts empty; the next layout fills it in.
                if auto {
                    base.handles.tree.borrow_mut().mark_dirty();
                }
            }
            *state.config.borrow_mut() = config.clone();
            if margin_given {
                // A step change cancels a running margin animation.
                state.snap_margin(config.margin, crate::clock::now(&base.handles.tree));
                base.handles.tree.borrow_mut().mark_dirty();
            }
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
            "namespace" => config
                .namespace
                .clone()
                .into_pyobject(py)?
                .into_any()
                .unbind(),
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
            "keyboard" => config
                .keyboard
                .name()
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "output" => config.output.clone().into_pyobject(py)?.into_any().unbind(),
            "input_region" if state.auto_region.get() => {
                "auto".into_pyobject(py)?.into_any().unbind()
            }
            "input_region" => match &config.input_region {
                None => py.None(),
                Some(rects) => PyList::new(py, rects.iter().copied())?.into_any().unbind(),
            },
            "click_through" => (!state.auto_region.get()
                && config.input_region.as_ref().is_some_and(Vec::is_empty))
            .into_pyobject(py)?
            .to_owned()
            .into_any()
            .unbind(),
            _ => {
                drop(config);
                base.get(name, py)?
            }
        })
    }

    /// Animates one margin to `to` pixels from where it is now: `property` is `"margin_top"`,
    /// `"margin_right"`, `"margin_bottom"` or `"margin_left"`. Same `duration_ms`, `easing`
    /// (linear, a cubic bezier, or a spring) and `on_complete` as `Node.animate`. The engine
    /// runs it, one cheap commit per step, with no Python until `on_complete`;
    /// `advance(ms)` runs it headless. `set(margin=...)` while it runs cancels it
    /// (`animation_end` with `finished=False`, no `on_complete`).
    #[pyo3(signature = (property, to, duration_ms=0, easing=None, on_complete=None))]
    fn animate(
        slf: &Bound<'_, Self>,
        property: &str,
        to: &Bound<'_, PyAny>,
        duration_ms: u64,
        easing: Option<Bound<'_, PyAny>>,
        on_complete: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        const NAMES: [&str; 4] = ["margin_top", "margin_right", "margin_bottom", "margin_left"];
        let Some(index) = NAMES.iter().position(|n| *n == property) else {
            return Err(PyValueError::new_err(format!(
                "LayerWindow.animate: {property:?} is not animatable here; only the margins are: {}",
                NAMES.join(", ")
            )));
        };
        let value: f64 = to.extract().map_err(|_| {
            PyValueError::new_err(format!(
                "LayerWindow.animate {property}: `to` must be a number"
            ))
        })?;
        if !value.is_finite() || value.abs() > f64::from(i32::MAX / 2) {
            return Err(PyValueError::new_err(format!(
                "LayerWindow.animate {property}: `to` must be a finite number of pixels"
            )));
        }
        if let Some(callback) = &on_complete
            && !callback.bind(slf.py()).is_callable()
        {
            return Err(PyValueError::new_err(
                "LayerWindow.animate: on_complete must be callable",
            ));
        }
        let duration = Duration::from_millis(duration_ms);
        let curve = crate::node::parse_easing(easing.as_ref(), duration)?;
        let base = slf.as_super().borrow();
        let state = base
            .handles
            .layer
            .clone()
            .expect("a LayerWindow has layer state");
        let now = crate::clock::now(&base.handles.tree);
        let handle = base.handles.completions.borrow_mut().register(
            on_complete,
            base.handles.root,
            property,
        );
        state.animate_margin(index, value, duration, curve, now, handle);
        // A running loop picks the animation up on its next pass.
        base.handles.tree.borrow_mut().mark_dirty();
        if let Some(waker) = base.handles.waker.borrow().as_ref() {
            waker.wake();
        }
        Ok(())
    }

    /// Closes the surface: `close_requested` first (a listener may cancel it with
    /// `event.cancel()`), then `closed` with reason `"app"`. It happens on the loop's next
    /// turn. A window that is not open has nothing to close.
    fn close(slf: &Bound<'_, Self>) {
        #[cfg(target_os = "linux")]
        {
            let base = slf.as_super().borrow();
            if let (Some(layer), Some(waker)) =
                (&base.handles.layer, base.handles.waker.borrow().as_ref())
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
