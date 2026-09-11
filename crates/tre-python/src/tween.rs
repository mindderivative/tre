//! `tre.Tween`/`tre.Easing`/`tre.Spring` (Phase 13 Step 13.2: `treTween`),
//! binding directly to `tre-tween`'s own real, pure interpolation math --
//! no new tweening logic lives in this file, only the Python-facing
//! shape of it.
//!
//! `tre_tween::Tween<T>` is generic over anything `Lerp`, but PyO3
//! cannot export a generic class -- `PyTween` instead stores one of two
//! concrete instantiations (`f32` or `glam::Vec2`) chosen from the
//! Python types passed to its constructor (`float` or a real 2-tuple),
//! and `sample()` returns the matching Python type back. A real,
//! disclosed scope choice: only these two `Lerp` instantiations are
//! exposed to Python today (`glam::Vec3`/`Quat` are real, straightforward
//! additions once a real 3D consumer exists in this project -- not
//! speculative work).

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use tre_tween::{Easing, Spring, Tween};

/// Mirrors `tre_tween::Easing` -- see its own doc comment for what each
/// curve does; every variant here dispatches straight through.
#[pyclass(name = "Easing", eq)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PyEasing {
    #[default]
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
}

impl From<PyEasing> for Easing {
    fn from(easing: PyEasing) -> Self {
        match easing {
            PyEasing::Linear => Self::Linear,
            PyEasing::EaseInQuad => Self::EaseInQuad,
            PyEasing::EaseOutQuad => Self::EaseOutQuad,
            PyEasing::EaseInOutQuad => Self::EaseInOutQuad,
            PyEasing::EaseInCubic => Self::EaseInCubic,
            PyEasing::EaseOutCubic => Self::EaseOutCubic,
            PyEasing::EaseInOutCubic => Self::EaseInOutCubic,
            PyEasing::EaseInQuart => Self::EaseInQuart,
            PyEasing::EaseOutQuart => Self::EaseOutQuart,
            PyEasing::EaseInOutQuart => Self::EaseInOutQuart,
            PyEasing::EaseInQuint => Self::EaseInQuint,
            PyEasing::EaseOutQuint => Self::EaseOutQuint,
            PyEasing::EaseInOutQuint => Self::EaseInOutQuint,
        }
    }
}

/// Either `Lerp` instantiation `PyTween` actually stores -- chosen once,
/// at construction, from whether `from_`/`to` were passed as a plain
/// `float` or a real `(x, y)` tuple.
enum TweenValue {
    Scalar(Tween<f32>),
    Vec2(Tween<glam::Vec2>),
}

/// A real number or a real 2D point -- whichever `PyTween` was
/// constructed with. Rejects anything else with a `TypeError` naming
/// the two real accepted shapes, rather than a confusing downstream
/// failure.
#[derive(Clone, Copy)]
enum PyLerpValue {
    Scalar(f32),
    Vec2(f32, f32),
}

impl PyLerpValue {
    fn from_python(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(scalar) = value.extract::<f32>() {
            return Ok(Self::Scalar(scalar));
        }
        if let Ok((x, y)) = value.extract::<(f32, f32)>() {
            return Ok(Self::Vec2(x, y));
        }
        Err(PyTypeError::new_err(
            "Tween from_/to must both be a real number or a real (x, y) tuple",
        ))
    }
}

#[pyclass(name = "Tween")]
pub struct PyTween {
    value: TweenValue,
}

#[pymethods]
impl PyTween {
    /// # Errors
    /// Raises `TypeError` if `from_`/`to` are not both a plain number or
    /// both a real `(x, y)` tuple, or if they mismatch each other's shape.
    #[new]
    #[pyo3(signature = (from_, to, duration, easing = PyEasing::Linear))]
    fn new(
        from_: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
        duration: f32,
        easing: PyEasing,
    ) -> PyResult<Self> {
        let from_ = PyLerpValue::from_python(from_)?;
        let to = PyLerpValue::from_python(to)?;
        let easing: Easing = easing.into();
        let value = match (from_, to) {
            (PyLerpValue::Scalar(from_), PyLerpValue::Scalar(to)) => {
                TweenValue::Scalar(Tween::new(from_, to, duration, easing))
            }
            (PyLerpValue::Vec2(fx, fy), PyLerpValue::Vec2(tx, ty)) => TweenValue::Vec2(Tween::new(
                glam::Vec2::new(fx, fy),
                glam::Vec2::new(tx, ty),
                duration,
                easing,
            )),
            _ => {
                return Err(PyTypeError::new_err(
                    "Tween from_ and to must both be the same shape: both a number, or both an \
                     (x, y) tuple",
                ));
            }
        };
        Ok(Self { value })
    }

    /// Samples this tween at `elapsed` real seconds since it started,
    /// returning a `float` (scalar tween) or an `(x, y)` tuple (2D
    /// tween) matching whatever shape it was constructed with.
    fn sample(&self, py: Python<'_>, elapsed: f32) -> PyResult<Py<PyAny>> {
        match &self.value {
            TweenValue::Scalar(tween) => {
                Ok(tween.sample(elapsed).into_pyobject(py)?.into_any().unbind())
            }
            TweenValue::Vec2(tween) => {
                let sampled = tween.sample(elapsed);
                Ok((sampled.x, sampled.y)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind())
            }
        }
    }
}

/// A real damped mass-spring-damper integrator. Mirrors
/// `tre_tween::Spring` -- see its own doc comment for why this is a
/// genuine second-order ODE (can overshoot/oscillate), unlike
/// `tre_math::spring_decay`'s plain exponential smoothing.
#[pyclass(name = "Spring")]
pub struct PySpring {
    inner: Spring,
}

#[pymethods]
impl PySpring {
    #[new]
    #[pyo3(signature = (stiffness, damping, mass, initial_position = 0.0))]
    fn new(stiffness: f32, damping: f32, mass: f32, initial_position: f32) -> Self {
        Self {
            inner: Spring::new(stiffness, damping, mass, initial_position),
        }
    }

    #[getter]
    fn position(&self) -> f32 {
        self.inner.position()
    }

    #[getter]
    fn velocity(&self) -> f32 {
        self.inner.velocity()
    }

    /// Advances this spring by `dt` real seconds toward `target`,
    /// returning the new position.
    fn update(&mut self, target: f32, dt: f32) -> f32 {
        self.inner.update(target, dt)
    }
}
