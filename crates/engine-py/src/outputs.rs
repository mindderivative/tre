//! 0.5.7 (#202): `Output`, a screen the compositor has, and the `App` side of knowing
//! about them (`App.outputs()`, `App.on("output_added" ...)`, `App.simulate`).

use std::hash::{Hash, Hasher};

use engine_platform::outputs::OutputInfo;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// The output events an app can listen for.
pub(crate) const EVENTS: [&str; 3] = ["output_added", "output_removed", "output_updated"];

/// A screen: read-only, equal to another `Output` with the same connector name.
#[pyclass(name = "Output", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub struct PyOutput {
    pub(crate) info: OutputInfo,
}

impl PyOutput {
    pub(crate) fn new(info: OutputInfo) -> Self {
        Self { info }
    }
}

#[pymethods]
impl PyOutput {
    /// The connector name (`"DP-1"`): stable across runs.
    #[getter]
    fn name(&self) -> &str {
        &self.info.name
    }

    #[getter]
    fn description(&self) -> &str {
        &self.info.description
    }

    /// The output's left edge in the desktop's logical layout.
    #[getter]
    fn x(&self) -> i32 {
        self.info.x
    }

    #[getter]
    fn y(&self) -> i32 {
        self.info.y
    }

    /// The output's logical width, in pixels.
    #[getter]
    fn width(&self) -> u32 {
        self.info.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.info.height
    }

    /// Physical over logical pixels: 1.5 on a 1.5x output.
    #[getter]
    fn scale_factor(&self) -> f64 {
        self.info.scale_factor
    }

    /// `"normal"`, `"90"`, `"180"`, `"270"`, or one of those after `"flipped_"`.
    #[getter]
    fn transform(&self) -> &str {
        &self.info.transform
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.info.name == self.info.name)
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.info.name.hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!(
            "Output({:?}, {}x{} at ({}, {}), scale {})",
            self.info.name,
            self.info.width,
            self.info.height,
            self.info.x,
            self.info.y,
            self.info.scale_factor
        )
    }
}

/// `Output` fields for `App.simulate`, with defaults for what is not given.
pub(crate) fn simulated(fields: Option<&Bound<'_, PyDict>>) -> PyResult<OutputInfo> {
    let mut info = OutputInfo {
        name: String::new(),
        description: String::new(),
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        scale_factor: 1.0,
        transform: "normal".to_string(),
    };
    let mut named = false;
    if let Some(fields) = fields {
        for (key, value) in fields.iter() {
            let key: String = key.extract()?;
            let bad = || PyValueError::new_err(format!("App.simulate: `{key}` has the wrong type"));
            match key.as_str() {
                "name" => {
                    info.name = value.extract().map_err(|_| bad())?;
                    named = true;
                }
                "description" => info.description = value.extract().map_err(|_| bad())?,
                "x" => info.x = value.extract().map_err(|_| bad())?,
                "y" => info.y = value.extract().map_err(|_| bad())?,
                "width" => info.width = value.extract().map_err(|_| bad())?,
                "height" => info.height = value.extract().map_err(|_| bad())?,
                "scale_factor" => info.scale_factor = value.extract().map_err(|_| bad())?,
                "transform" => info.transform = value.extract().map_err(|_| bad())?,
                other => {
                    return Err(PyValueError::new_err(format!(
                        "App.simulate: unknown field `{other}`; an output has name, description, \
                         x, y, width, height, scale_factor and transform"
                    )));
                }
            }
        }
    }
    if !named {
        return Err(PyValueError::new_err(
            "App.simulate: an output event needs a `name`",
        ));
    }
    Ok(info)
}

pub(crate) fn unknown_event(name: &str) -> PyErr {
    PyValueError::new_err(format!(
        "unknown app event {name:?} -- valid events: {}",
        EVENTS.join(", ")
    ))
}

/// `App`'s listeners for the output events, by event name: the handler and whether it
/// takes the `Output`.
pub(crate) type OutputListeners =
    std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, (Py<PyAny>, bool)>>>;

/// Calls the listener for `event` with an `Output`, logging what it raises.
pub(crate) fn deliver(listeners: &OutputListeners, py: Python<'_>, event: &str, info: OutputInfo) {
    let Some((handler, wants)) = listeners
        .borrow()
        .get(event)
        .map(|(handler, wants)| (handler.clone_ref(py), *wants))
    else {
        return;
    };
    let result = Py::new(py, PyOutput::new(info)).and_then(|output| {
        if wants {
            handler.call1(py, (output,))
        } else {
            handler.call0(py)
        }
    });
    if let Err(err) = result {
        crate::dispatch::log_uncaught_exception(&err, py);
    }
}
