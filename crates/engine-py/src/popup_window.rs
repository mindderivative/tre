//! 0.5.7 (#223): `PopupWindow`, a window whose Wayland role is `xdg_popup`, parented to a
//! `LayerWindow` (or to another popup): a menu or flyout that hangs off a thin bar.
//!
//! It is a layer-thread surface like a `LayerWindow` (so its GPU surface, scale, input, frames,
//! input method and screen-reader tree are the layer path's) with a different role: the
//! compositor places it from the positioner (it flips and slides it to stay on screen), and a
//! grab makes an outside click or Escape end it (`closed`, reason `"dismissed"`).

use std::rc::Rc;

use engine_platform::layer_config::{Availability, PopupConfig, PopupEdge, adjust, availability};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyString};

use crate::layer_window::{LayerState, PopupSpec, UnsupportedPlatform};
use crate::window::PyWindow;

/// Window keys that mean nothing to a popup the compositor places and sizes.
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

/// The popup's own settings: fixed once the popup is made.
const FIXED: &[&str] = &[
    "parent",
    "size",
    "anchor_rect",
    "anchor",
    "gravity",
    "constraint_adjustment",
    "offset",
    "grab",
];

fn bad(key: &str, what: &str) -> PyErr {
    PyValueError::new_err(format!("PopupWindow {key}: {what}"))
}

fn edge(key: &str, value: &str) -> PyResult<PopupEdge> {
    PopupEdge::parse(value).ok_or_else(|| {
        bad(
            key,
            &format!(
                "unknown edge {value:?}; valid: {}",
                PopupEdge::NAMES.join(", ")
            ),
        )
    })
}

fn pair_i32(key: &str, value: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    value
        .extract::<(i32, i32)>()
        .map_err(|_| bad(key, "must be two whole numbers"))
}

/// The parent's layer-thread state, from a `LayerWindow` or a `PopupWindow`.
fn parent_state(parent: &Bound<'_, PyAny>) -> PyResult<Rc<LayerState>> {
    let window = parent
        .cast::<PyWindow>()
        .map_err(|_| bad("parent", "must be a LayerWindow or a PopupWindow"))?;
    let state = window.borrow().handles.layer.clone();
    state.ok_or_else(|| {
        bad(
            "parent",
            "must be a LayerWindow or a PopupWindow, not an ordinary Window",
        )
    })
}

/// A menu or flyout the compositor places next to a rectangle of its parent.
#[pyclass(name = "PopupWindow", extends = PyWindow)]
pub struct PyPopupWindow;

#[pymethods]
impl PyPopupWindow {
    #[new]
    #[pyo3(signature = (
        parent,
        *,
        size,
        anchor_rect,
        anchor = "bottom_left",
        gravity = "bottom_right",
        constraint_adjustment = None,
        offset = None,
        grab = true
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        parent: &Bound<'_, PyAny>,
        size: &Bound<'_, PyAny>,
        anchor_rect: &Bound<'_, PyAny>,
        anchor: &str,
        gravity: &str,
        constraint_adjustment: Option<&Bound<'_, PyAny>>,
        offset: Option<&Bound<'_, PyAny>>,
        grab: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        match availability() {
            Availability::Unsupported(why) => return Err(UnsupportedPlatform::new_err(why)),
            Availability::Supported | Availability::Headless => {}
        }
        let parent_state = parent_state(parent)?;
        let (width, height): (u32, u32) = size.extract().map_err(|_| {
            bad(
                "size",
                "must be (width, height), two whole numbers of pixels above 0",
            )
        })?;
        let rect: (i32, i32, i32, i32) = anchor_rect.extract().map_err(|_| {
            bad(
                "anchor_rect",
                "must be (x, y, width, height), whole numbers",
            )
        })?;
        // Names, or one name, as in `constraint_adjustment=("flip_y", "slide_x")`.
        let names: Vec<String> = match constraint_adjustment {
            None => vec!["flip_y".into(), "slide_x".into()],
            Some(value) if value.is_none() => Vec::new(),
            Some(value) => {
                if let Ok(one) = value.cast::<PyString>() {
                    vec![one.to_string()]
                } else {
                    value.extract().map_err(|_| {
                        bad(
                            "constraint_adjustment",
                            "must be a name or a sequence of names",
                        )
                    })?
                }
            }
        };
        let mut adjustments = 0;
        for name in &names {
            adjustments |= adjust::parse(name).ok_or_else(|| {
                bad(
                    "constraint_adjustment",
                    &format!(
                        "unknown adjustment {name:?}; valid: {}",
                        adjust::NAMES.map(|(n, _)| n).join(", ")
                    ),
                )
            })?;
        }
        let config = PopupConfig {
            parent: 0,
            size: (width, height),
            anchor_rect: rect,
            anchor: edge("anchor", anchor)?,
            gravity: edge("gravity", gravity)?,
            adjustments,
            offset: offset
                .map(|v| pair_i32("offset", v))
                .transpose()?
                .unwrap_or((0, 0)),
            grab,
        };
        config
            .validate()
            .map_err(|e| PyValueError::new_err(format!("PopupWindow {e}")))?;
        let mut base = PyWindow::construct(width, height, "popup", false, 0.0, 0.0)?;
        let handles = &mut base.handles;
        // Layout in logical pixels, drawn at the surface's scale; transparent so a rounded menu
        // needs no backdrop.
        handles.dpi_scaling.set(true);
        handles.transparent.set(true);
        let mut state = LayerState::new_popup();
        state.popup = Some(PopupSpec {
            parent: parent_state,
            config,
        });
        handles.layer = Some(Rc::new(state));
        handles.tree.borrow_mut().set_window_drag_allowed(false);
        Ok(PyClassInitializer::from(base).add_subclass(Self))
    }

    /// Sets the window keys a popup shares (`touch_slop`, `partial_redraw`, ...). The popup's own
    /// settings are fixed when it is made, and the keys that mean nothing to a popup raise.
    #[pyo3(signature = (**props))]
    fn set(slf: &Bound<'_, Self>, props: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let base = slf.as_super().borrow();
        let Some(props) = props else {
            return Ok(());
        };
        for name in props.keys() {
            let name: String = name.extract()?;
            if FIXED.contains(&name.as_str()) {
                return Err(bad(&name, "can only be given to the constructor"));
            }
            if REFUSED.contains(&name.as_str()) {
                return Err(bad(
                    &name,
                    "means nothing to a popup the compositor places and sizes",
                ));
            }
        }
        base.set(Some(props))
    }

    /// Reads a popup setting (`size`, `anchor_rect`, `anchor`, `gravity`,
    /// `constraint_adjustment`, `offset`, `grab`), where the compositor put it (`position`, in
    /// the parent's coordinates, `None` until it is placed), or anything `Window.get` reads.
    fn get(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let base = slf.as_super().borrow();
        let state = base
            .handles
            .layer
            .clone()
            .expect("a PopupWindow has popup state");
        let spec = state
            .popup
            .as_ref()
            .expect("a PopupWindow has a popup spec");
        let c = &spec.config;
        Ok(match name {
            "size" => c.size.into_pyobject(py)?.into_any().unbind(),
            "anchor_rect" => c.anchor_rect.into_pyobject(py)?.into_any().unbind(),
            "anchor" => c.anchor.name().into_pyobject(py)?.into_any().unbind(),
            "gravity" => c.gravity.name().into_pyobject(py)?.into_any().unbind(),
            "constraint_adjustment" => pyo3::types::PyTuple::new(py, adjust::names(c.adjustments))?
                .into_any()
                .unbind(),
            "offset" => c.offset.into_pyobject(py)?.into_any().unbind(),
            "grab" => c.grab.into_pyobject(py)?.to_owned().into_any().unbind(),
            "position" => state
                .popup_position
                .get()
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            _ => base.get(name, py)?,
        })
    }

    /// Closes the popup: `close_requested` first (a listener may cancel it), then `closed` with
    /// reason `"app"`. It happens on the loop's next turn.
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
