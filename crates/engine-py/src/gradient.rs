//! 0.5.4 (#109): `tre.Gradient`, a gradient fill for a box's `fill`.

use engine_core::{GradientShape, GradientStop};
use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// A gradient to give a box as its `fill`: `node.set(fill=Gradient.linear(...))`.
///
/// Built with `Gradient.linear`, `Gradient.radial` or `Gradient.sweep`. Positions are
/// relative to the box, so a gradient follows its box as layout resizes it.
/// Immutable: to change one, set or animate `fill` to another.
#[pyclass(frozen, name = "Gradient", eq, from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyGradient {
    pub(crate) inner: engine_core::Gradient,
}

/// A stop as Python gives it: `(offset, (r, g, b, a))`.
type StopTuple = (f32, (u8, u8, u8, u8));

fn color_of(value: &Bound<'_, PyAny>) -> PyResult<Color> {
    let (r, g, b, a): (u8, u8, u8, u8) = value.extract().map_err(|_| {
        PyValueError::new_err("a gradient stop's color must be an (r, g, b, a) tuple of 0-255 ints")
    })?;
    Ok(Color::from_rgba8(r, g, b, a))
}

/// `stops`: either colours, spaced evenly from 0 to 1, or `(offset, color)`
/// pairs.
fn parse_stops(stops: &Bound<'_, PyAny>) -> PyResult<Vec<GradientStop>> {
    let items: Vec<Bound<'_, PyAny>> = stops
        .try_iter()
        .map_err(|_| PyValueError::new_err("gradient `stops` must be a sequence"))?
        .collect::<PyResult<_>>()?;
    let pairs: Option<Vec<StopTuple>> = items
        .iter()
        .map(|item| item.extract::<StopTuple>().ok())
        .collect();
    if let Some(pairs) = pairs.filter(|p| !p.is_empty()) {
        return Ok(pairs
            .into_iter()
            .map(|(offset, (r, g, b, a))| GradientStop {
                offset,
                color: Color::from_rgba8(r, g, b, a),
            })
            .collect());
    }
    let n = items.len();
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            Ok(GradientStop {
                offset: if n > 1 {
                    i as f32 / (n - 1) as f32
                } else {
                    0.0
                },
                color: color_of(item).map_err(|_| {
                    PyValueError::new_err(
                        "gradient `stops` must be (r, g, b, a) colors, or (offset, color) pairs",
                    )
                })?,
            })
        })
        .collect()
}

fn build(shape: GradientShape, stops: &Bound<'_, PyAny>) -> PyResult<PyGradient> {
    let inner =
        engine_core::Gradient::new(shape, parse_stops(stops)?).map_err(PyValueError::new_err)?;
    Ok(PyGradient { inner })
}

#[pymethods]
impl PyGradient {
    /// A gradient along a line through the box's centre at `angle` degrees
    /// (0 points up, 90 right, 180 down, the default), spanning the box.
    /// `stops` are colours, spaced evenly, or `(offset, color)` pairs with
    /// offsets from 0 to 1.
    #[staticmethod]
    #[pyo3(signature = (stops, angle=180.0))]
    fn linear(stops: &Bound<'_, PyAny>, angle: f64) -> PyResult<Self> {
        build(GradientShape::Linear { angle_deg: angle }, stops)
    }

    /// A gradient outward from `center` (fractions of the box; `None`, the
    /// default, is `(0.5, 0.5)`, its middle). `radius` is a fraction of the box's half-diagonal, so `1.0`
    /// reaches the far corner of a centred gradient.
    #[staticmethod]
    #[pyo3(signature = (stops, center=None, radius=1.0))]
    fn radial(stops: &Bound<'_, PyAny>, center: Option<(f64, f64)>, radius: f64) -> PyResult<Self> {
        let center = center.unwrap_or((0.5, 0.5));
        build(GradientShape::Radial { center, radius }, stops)
    }

    /// A gradient around `center`, starting `start` degrees clockwise from up.
    #[staticmethod]
    #[pyo3(signature = (stops, center=None, start=0.0))]
    fn sweep(stops: &Bound<'_, PyAny>, center: Option<(f64, f64)>, start: f64) -> PyResult<Self> {
        let center = center.unwrap_or((0.5, 0.5));
        build(
            GradientShape::Sweep {
                center,
                start_deg: start,
            },
            stops,
        )
    }

    /// `"linear"`, `"radial"` or `"sweep"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner.shape() {
            GradientShape::Linear { .. } => "linear",
            GradientShape::Radial { .. } => "radial",
            GradientShape::Sweep { .. } => "sweep",
        }
    }

    /// The `(offset, (r, g, b, a))` stops.
    #[getter]
    fn stops(&self) -> Vec<(f32, (u8, u8, u8, u8))> {
        self.inner
            .stops()
            .iter()
            .map(|stop| {
                let [r, g, b, a] = stop.color.to_rgba8().to_u8_array();
                (stop.offset, (r, g, b, a))
            })
            .collect()
    }

    /// A linear gradient's angle in degrees, else `None`.
    #[getter]
    fn angle(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Linear { angle_deg } => Some(*angle_deg),
            _ => None,
        }
    }

    /// A radial or sweep gradient's centre, else `None`.
    #[getter]
    fn center(&self) -> Option<(f64, f64)> {
        match self.inner.shape() {
            GradientShape::Radial { center, .. } | GradientShape::Sweep { center, .. } => {
                Some(*center)
            }
            GradientShape::Linear { .. } => None,
        }
    }

    /// A radial gradient's radius, else `None`.
    #[getter]
    fn radius(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Radial { radius, .. } => Some(*radius),
            _ => None,
        }
    }

    /// A sweep gradient's start angle in degrees, else `None`.
    #[getter]
    fn start(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Sweep { start_deg, .. } => Some(*start_deg),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        let shape = match self.inner.shape() {
            GradientShape::Linear { angle_deg } => format!("linear, angle={angle_deg}"),
            GradientShape::Radial { center, radius } => {
                format!("radial, center={center:?}, radius={radius}")
            }
            GradientShape::Sweep { center, start_deg } => {
                format!("sweep, center={center:?}, start={start_deg}")
            }
        };
        format!("Gradient({shape}, {} stops)", self.inner.stops().len())
    }
}
