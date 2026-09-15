//! §16.2's real `BindingResolver`: `engine_spec::BindingResolver`
//! evaluated against one live Python `ViewModel` object, via real
//! `pyo3` attribute/index/method access. `engine-spec` itself can't do
//! this -- no `pyo3` dependency, per its own crate-boundary rule
//! (§16.1) -- so this crate, the one place with real GIL access,
//! supplies the capability `engine-spec`'s generic evaluator only
//! defined the shape of.

use std::cell::RefCell;

use engine_spec::{BinOp, BindingResolver, ResolveError, Value};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;

/// Resolves `engine-spec`'s whitelisted binding grammar against one
/// live Python `ViewModel`. A resolved Python value that isn't already
/// one of `Value`'s primitive variants is stored in `handles` and
/// threaded back as an opaque `Value::Handle` -- see
/// `engine_spec::binding`'s own module doc comment for why.
pub struct PyViewModelResolver {
    viewmodel: Py<PyAny>,
    handles: RefCell<Vec<Py<PyAny>>>,
}

impl PyViewModelResolver {
    pub fn new(viewmodel: Py<PyAny>) -> Self {
        Self {
            viewmodel,
            handles: RefCell::new(Vec::new()),
        }
    }

    fn store_handle(&self, obj: Py<PyAny>) -> Value {
        let mut handles = self.handles.borrow_mut();
        let id = handles.len() as u64;
        handles.push(obj);
        Value::Handle(id)
    }

    /// A real Python object, converted to a primitive `Value` whenever
    /// it actually is one -- `bool` is checked *before* `i64`, since a
    /// Python `bool` is a subtype of `int` and would otherwise extract
    /// successfully (and wrongly) as one -- or stored as an opaque
    /// `Handle` otherwise.
    fn to_value(&self, obj: &Bound<'_, PyAny>) -> Value {
        if let Ok(b) = obj.extract::<bool>() {
            return Value::Bool(b);
        }
        if let Ok(i) = obj.extract::<i64>() {
            return Value::Int(i);
        }
        if let Ok(f) = obj.extract::<f64>() {
            return Value::Float(f);
        }
        if let Ok(s) = obj.extract::<String>() {
            return Value::Str(s);
        }
        self.store_handle(obj.clone().unbind())
    }

    /// The reverse: rebuilds a real Python object from a `Value` --
    /// needed because `attr`/`index`/`call`/`binary_op` all receive
    /// `Value`s (possibly primitives `engine-spec` itself already
    /// computed) but must call back into real Python objects.
    fn to_pyobject<'py>(&self, py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
        match value {
            Value::Int(i) => i.into_bound_py_any(py),
            Value::Float(f) => f.into_bound_py_any(py),
            Value::Str(s) => s.into_bound_py_any(py),
            Value::Bool(b) => b.into_bound_py_any(py),
            Value::Handle(id) => Ok(self.handles.borrow()[*id as usize].bind(py).clone()),
        }
    }
}

fn to_resolve_error(err: PyErr) -> ResolveError {
    ResolveError(err.to_string())
}

impl BindingResolver for PyViewModelResolver {
    fn ident(&self, name: &str) -> Result<Value, ResolveError> {
        Python::attach(|py| {
            let attr = self
                .viewmodel
                .bind(py)
                .getattr(name)
                .map_err(to_resolve_error)?;
            Ok(self.to_value(&attr))
        })
    }

    fn attr(&self, base: &Value, name: &str) -> Result<Value, ResolveError> {
        Python::attach(|py| {
            let obj = self.to_pyobject(py, base).map_err(to_resolve_error)?;
            let attr = obj.getattr(name).map_err(to_resolve_error)?;
            Ok(self.to_value(&attr))
        })
    }

    fn index(&self, base: &Value, index: &Value) -> Result<Value, ResolveError> {
        Python::attach(|py| {
            let obj = self.to_pyobject(py, base).map_err(to_resolve_error)?;
            let idx = self.to_pyobject(py, index).map_err(to_resolve_error)?;
            let item = obj.get_item(idx).map_err(to_resolve_error)?;
            Ok(self.to_value(&item))
        })
    }

    fn call(&self, base: &Value, method: &str) -> Result<Value, ResolveError> {
        Python::attach(|py| {
            let obj = self.to_pyobject(py, base).map_err(to_resolve_error)?;
            let result = obj.call_method0(method).map_err(to_resolve_error)?;
            Ok(self.to_value(&result))
        })
    }

    fn binary_op(&self, op: BinOp, left: &Value, right: &Value) -> Result<Value, ResolveError> {
        Python::attach(|py| {
            let l = self.to_pyobject(py, left).map_err(to_resolve_error)?;
            let r = self.to_pyobject(py, right).map_err(to_resolve_error)?;
            match op {
                BinOp::Add => Ok(self.to_value(&l.add(&r).map_err(to_resolve_error)?)),
                BinOp::Sub => Ok(self.to_value(&l.sub(&r).map_err(to_resolve_error)?)),
                BinOp::Mul => Ok(self.to_value(&l.mul(&r).map_err(to_resolve_error)?)),
                BinOp::Div => Ok(self.to_value(&l.div(&r).map_err(to_resolve_error)?)),
                BinOp::Eq => Ok(Value::Bool(l.eq(&r).map_err(to_resolve_error)?)),
                BinOp::Ne => Ok(Value::Bool(l.ne(&r).map_err(to_resolve_error)?)),
                BinOp::Lt => Ok(Value::Bool(l.lt(&r).map_err(to_resolve_error)?)),
                BinOp::Le => Ok(Value::Bool(l.le(&r).map_err(to_resolve_error)?)),
                BinOp::Gt => Ok(Value::Bool(l.gt(&r).map_err(to_resolve_error)?)),
                BinOp::Ge => Ok(Value::Bool(l.ge(&r).map_err(to_resolve_error)?)),
                BinOp::And | BinOp::Or => Err(ResolveError(
                    "and/or short-circuit in evaluate() and never reach binary_op".to_string(),
                )),
            }
        })
    }

    fn truthy(&self, value: &Value) -> Result<bool, ResolveError> {
        Python::attach(|py| {
            let obj = self.to_pyobject(py, value).map_err(to_resolve_error)?;
            obj.is_truthy().map_err(to_resolve_error)
        })
    }
}
