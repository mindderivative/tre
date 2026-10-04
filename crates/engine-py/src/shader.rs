//! 0.5.1 (#66): `tre.Shader` -- the app's WGSL, checked when it is made --
//! and `tre.ShaderError`, the `ValueError` a mistake raises. The checking is
//! `engine_core::Shader`; this is the Python skin on it. Nothing here draws.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::{NodeId, ShaderError as CoreError, ShaderMode, UniformValue};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyTuple};

use crate::node::Node;

pyo3::create_exception!(
    tre,
    ShaderError,
    PyValueError,
    "A shader's source or names are wrong. `line`, `column` and `source_line` \
     (each `None` when the problem has no position) place it in the WGSL you gave."
);

/// The Python exception for a core error, with its position as attributes.
fn raise(error: &CoreError) -> PyErr {
    Python::attach(|py| {
        let err = PyErr::new::<ShaderError, _>(error.to_string());
        let value = err.value(py);
        // Setting an attribute on a fresh exception instance can't fail.
        let _ = value.setattr("line", error.line);
        let _ = value.setattr("column", error.column);
        let _ = value.setattr("source_line", error.source_line.clone());
        err
    })
}

fn number(value: &Bound<'_, PyAny>, what: &str) -> PyResult<f32> {
    let is_number = value.is_instance_of::<PyFloat>() || value.is_instance_of::<PyInt>();
    if value.is_instance_of::<PyBool>() || !is_number {
        return Err(PyTypeError::new_err(format!(
            "uniform `{what}` must be a number or a tuple of 2 to 4 numbers"
        )));
    }
    let v: f64 = value.extract()?;
    if !v.is_finite() {
        return Err(PyValueError::new_err(format!(
            "uniform `{what}` must be finite"
        )));
    }
    Ok(v as f32)
}

fn uniform_value(name: &str, value: &Bound<'_, PyAny>) -> PyResult<UniformValue> {
    if value.is_instance_of::<PyTuple>() || value.is_instance_of::<PyList>() {
        let parts = value
            .try_iter()?
            .map(|item| number(&item?, name))
            .collect::<PyResult<Vec<f32>>>()?;
        return match parts[..] {
            [a, b] => Ok(UniformValue::Vec2([a, b])),
            [a, b, c] => Ok(UniformValue::Vec3([a, b, c])),
            [a, b, c, d] => Ok(UniformValue::Vec4([a, b, c, d])),
            _ => Err(PyValueError::new_err(format!(
                "uniform `{name}` has {} numbers; a vector has 2, 3 or 4",
                parts.len()
            ))),
        };
    }
    Ok(UniformValue::F32(number(value, name)?))
}

fn uniforms_from(dict: &Bound<'_, PyDict>) -> PyResult<Vec<(String, UniformValue)>> {
    let mut out = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let name: String = key
            .extract()
            .map_err(|_| PyTypeError::new_err("uniform names must be str"))?;
        let value = uniform_value(&name, &value)?;
        out.push((name, value));
    }
    Ok(out)
}

fn value_to_py(py: Python<'_>, value: UniformValue) -> PyResult<Py<PyAny>> {
    Ok(match value {
        UniformValue::F32(v) => f64::from(v).into_pyobject(py)?.into_any().unbind(),
        UniformValue::Vec2(v) => PyTuple::new(py, v.map(f64::from))?.into_any().unbind(),
        UniformValue::Vec3(v) => PyTuple::new(py, v.map(f64::from))?.into_any().unbind(),
        UniformValue::Vec4(v) => PyTuple::new(py, v.map(f64::from))?.into_any().unbind(),
    })
}

/// One CSS filter function: its name (also the uniform's), how a value is
/// checked, and the WGSL line that applies `u.<name>` to `rgb`.
struct FilterFn {
    name: &'static str,
    /// The largest value, or `None` for no upper bound.
    max: Option<f32>,
    /// Whether a negative value means something (a hue rotation).
    signed: bool,
    wgsl: &'static str,
}

/// The CSS filter functions, with CSS's own definitions (the matrices of the
/// Filter Effects spec), applied to straight-alpha sRGB-encoded colour and
/// clamped after each, as a browser does.
const FILTERS: [FilterFn; 7] = [
    FilterFn {
        name: "saturate",
        max: None,
        signed: false,
        wgsl: "rgb = mix(vec3<f32>(dot(rgb, vec3<f32>(0.213, 0.715, 0.072))), rgb, u.saturate);",
    },
    FilterFn {
        name: "brightness",
        max: None,
        signed: false,
        wgsl: "rgb = rgb * u.brightness;",
    },
    FilterFn {
        name: "contrast",
        max: None,
        signed: false,
        wgsl: "rgb = (rgb - vec3<f32>(0.5)) * u.contrast + vec3<f32>(0.5);",
    },
    FilterFn {
        name: "grayscale",
        max: Some(1.0),
        signed: false,
        wgsl: "rgb = mix(rgb, vec3<f32>(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722))), u.grayscale);",
    },
    FilterFn {
        name: "hue_rotate",
        max: None,
        signed: true,
        wgsl: "{ let a = radians(u.hue_rotate); let c = cos(a); let s = sin(a); \
               rgb = vec3<f32>( \
               dot(rgb, vec3<f32>(0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928)), \
               dot(rgb, vec3<f32>(0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283)), \
               dot(rgb, vec3<f32>(0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072))); }",
    },
    FilterFn {
        name: "invert",
        max: Some(1.0),
        signed: false,
        wgsl: "rgb = mix(rgb, vec3<f32>(1.0) - rgb, u.invert);",
    },
    FilterFn {
        name: "sepia",
        max: Some(1.0),
        signed: false,
        wgsl: "rgb = mix(rgb, vec3<f32>(dot(rgb, vec3<f32>(0.393, 0.769, 0.189)), dot(rgb, vec3<f32>(0.349, 0.686, 0.168)), dot(rgb, vec3<f32>(0.272, 0.534, 0.131))), u.sepia);",
    },
];

/// The effect WGSL for `names` (indexes into `FILTERS`), applied in order.
fn filter_wgsl(order: &[usize]) -> String {
    let mut body = String::from("    let c = content(p.uv);\n    var rgb = c.rgb;\n");
    for &i in order {
        body.push_str("    ");
        body.push_str(FILTERS[i].wgsl);
        body.push_str("\n    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));\n");
    }
    format!("fn shade(p: Pixel) -> vec4<f32> {{\n{body}    return vec4<f32>(rgb, c.a);\n}}\n")
}

/// A shader: WGSL with one function, `fn shade(p: Pixel) -> vec4<f32>`,
/// checked when it is made.
#[pyclass(frozen, name = "Shader")]
pub struct Shader {
    pub(crate) core: Arc<engine_core::Shader>,
    /// The `Node` handles given as inputs, so `inputs` hands back the same
    /// objects.
    inputs: Vec<(String, Py<Node>)>,
}

impl Shader {
    /// A Python object for an already-built core shader (as `node.get("shader")`
    /// returns): equal to the one that was set, as equality is by identity of
    /// the core shader. `handle` makes a `Node` for an input.
    pub(crate) fn wrap(
        py: Python<'_>,
        core: Arc<engine_core::Shader>,
        handle: impl Fn(NodeId) -> Node,
    ) -> Self {
        let inputs = core
            .inputs()
            .iter()
            .filter_map(|(name, id)| Py::new(py, handle(*id)).ok().map(|n| (name.clone(), n)))
            .collect();
        Self { core, inputs }
    }

    /// Whether every input node lives in `tree`.
    pub(crate) fn inputs_in(&self, py: Python<'_>, tree: &Rc<RefCell<engine_core::Tree>>) -> bool {
        self.inputs
            .iter()
            .all(|(_, node)| Rc::ptr_eq(&node.borrow(py).tree, tree))
    }
}

#[pymethods]
impl Shader {
    #[new]
    #[pyo3(signature = (wgsl, uniforms=None, inputs=None, mode="fill", animated=false))]
    fn new(
        wgsl: String,
        uniforms: Option<Bound<'_, PyDict>>,
        inputs: Option<Bound<'_, PyDict>>,
        mode: &str,
        animated: bool,
    ) -> PyResult<Self> {
        let mode = match mode {
            "fill" => ShaderMode::Fill,
            "effect" => ShaderMode::Effect,
            other => {
                return Err(PyValueError::new_err(format!(
                    "`mode` must be \"fill\" or \"effect\", got {other:?}"
                )));
            }
        };
        let uniforms = match &uniforms {
            Some(dict) => uniforms_from(dict)?,
            None => Vec::new(),
        };
        let mut handles = Vec::new();
        let mut ids: Vec<(String, NodeId)> = Vec::new();
        let mut trees = HashSet::new();
        if let Some(dict) = &inputs {
            for (key, value) in dict.iter() {
                let name: String = key
                    .extract()
                    .map_err(|_| PyTypeError::new_err("input names must be str"))?;
                let node = value
                    .cast::<Node>()
                    .map_err(|_| PyTypeError::new_err(format!("input `{name}` must be a Node")))?;
                let state = node.borrow();
                state.check_alive()?;
                let usable = state.tree.borrow().get(state.id).is_some_and(|n| {
                    matches!(n.kind, engine_core::NodeKind::Image(_)) || n.shader.is_some()
                });
                if !usable {
                    return Err(PyValueError::new_err(format!(
                        "input `{name}` must be an image or video node, or a node that has a \
                         shader (set one on it first)"
                    )));
                }
                trees.insert(Rc::as_ptr(&state.tree) as usize);
                ids.push((name.clone(), state.id));
                handles.push((name, node.clone().unbind()));
            }
        }
        if trees.len() > 1 {
            return Err(PyValueError::new_err(
                "a shader's inputs must all be Nodes of one Window",
            ));
        }
        let core =
            engine_core::Shader::new(wgsl, uniforms, ids, mode, animated).map_err(|e| raise(&e))?;
        Ok(Self {
            core,
            inputs: handles,
        })
    }

    /// (0.5.4) An effect shader that applies CSS filter functions to a node
    /// and its subtree, in the order the keywords are given. See the stub.
    #[staticmethod]
    #[pyo3(signature = (**filters))]
    fn filter(filters: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut order = Vec::new();
        let mut uniforms: Vec<(String, UniformValue)> = Vec::new();
        for (key, value) in filters.into_iter().flat_map(|d| d.iter()) {
            let name: String = key.extract()?;
            let Some(index) = FILTERS.iter().position(|f| f.name == name) else {
                let known: Vec<&str> = FILTERS.iter().map(|f| f.name).collect();
                return Err(PyTypeError::new_err(format!(
                    "Shader.filter() has no filter `{name}`; the filters are {}",
                    known.join(", ")
                )));
            };
            let f = &FILTERS[index];
            let v = number(&value, &name)?;
            if !v.is_finite() || (v < 0.0 && !f.signed) || f.max.is_some_and(|m| v > m) {
                let range = match (f.signed, f.max) {
                    (true, _) => "a number of degrees".to_owned(),
                    (false, Some(m)) => format!("a number from 0 to {m}"),
                    (false, None) => "a number of 0 or more".to_owned(),
                };
                return Err(PyValueError::new_err(format!(
                    "filter `{name}` must be {range}, got {v}"
                )));
            }
            order.push(index);
            uniforms.push((name, UniformValue::F32(v)));
        }
        if order.is_empty() {
            return Err(PyValueError::new_err(
                "Shader.filter() needs at least one filter, e.g. Shader.filter(grayscale=1.0)",
            ));
        }
        let core = engine_core::Shader::new(
            filter_wgsl(&order),
            uniforms,
            Vec::new(),
            ShaderMode::Effect,
            false,
        )
        .map_err(|e| raise(&e))?;
        Ok(Self {
            core,
            inputs: Vec::new(),
        })
    }

    /// The WGSL, as given.
    #[getter]
    fn wgsl(&self) -> String {
        self.core.wgsl().to_owned()
    }

    /// `"fill"` or `"effect"`.
    #[getter]
    fn mode(&self) -> &'static str {
        self.core.mode().name()
    }

    /// Whether the shader redraws every frame.
    #[getter]
    fn animated(&self) -> bool {
        self.core.animated()
    }

    /// The uniforms and their current values, as a new dict.
    #[getter]
    fn uniforms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, value) in self.core.uniforms() {
            dict.set_item(name, value_to_py(py, value)?)?;
        }
        Ok(dict)
    }

    /// The input `Node`s by name, as a new dict.
    #[getter]
    fn inputs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, node) in &self.inputs {
            dict.set_item(name, node.clone_ref(py))?;
        }
        Ok(dict)
    }

    /// Replaces the uniforms, checked first and applied all at once: a
    /// mistake raises and changes nothing. Every node using this shader
    /// updates.
    #[pyo3(signature = (*, uniforms))]
    fn set(&self, uniforms: &Bound<'_, PyDict>) -> PyResult<()> {
        self.core
            .set_uniforms(uniforms_from(uniforms)?)
            .map_err(|e| raise(&e))
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .cast::<Shader>()
            .is_ok_and(|o| Arc::ptr_eq(&self.core, &o.get().core))
    }

    fn __hash__(&self) -> usize {
        Arc::as_ptr(&self.core) as usize
    }

    fn __repr__(&self) -> String {
        format!(
            "Shader(mode={:?}, animated={}, uniforms={}, inputs={})",
            self.core.mode().name(),
            self.core.animated(),
            self.core.uniforms().len(),
            self.inputs.len()
        )
    }
}
