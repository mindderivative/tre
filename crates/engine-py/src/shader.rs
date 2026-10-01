//! 0.5.1 (#66): `tre.Shader` -- the app's WGSL, checked when it is made --
//! and `tre.ShaderError`, the `ValueError` a mistake raises. The checking is
//! `engine_core::Shader`; this is the Python skin on it. Nothing here draws.

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

/// A shader: WGSL with one function, `fn shade(p: Pixel) -> vec4<f32>`,
/// checked when it is made.
#[pyclass(frozen, name = "Shader")]
pub struct Shader {
    pub(crate) core: Arc<engine_core::Shader>,
    /// The `Node` handles given as inputs, so `inputs` hands back the same
    /// objects.
    inputs: Vec<(String, Py<Node>)>,
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
