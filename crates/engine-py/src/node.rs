//! `Node` (§8's `PyNode`, renamed to match what Python actually sees --
//! `tre.Node`, not `tre.PyNode`) -- narrower than §8's own full sketch:
//! `animate()` only, for now. No `add_child` (no cycle to reject, so
//! `EngineError::CycleRejected` isn't implemented yet either) and no
//! `set_on_click` (storing a Python callback needs `PyWindow`'s
//! `#[pyclass(gc)]` cyclic-GC participation, §8's own review note --
//! deferred until something actually stores one). Each is additive when
//! its own later build-order step needs it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{MotionCurve, NodeId, NodeKind, Tree};
use peniko::Color;
use pyo3::prelude::*;

use crate::error::EngineError;

#[pyclass(unsendable)]
pub struct Node {
    pub(crate) id: NodeId,
    pub(crate) tree: Rc<RefCell<Tree>>,
}

#[pymethods]
impl Node {
    /// Starts (or retargets, §5's own `animate_to` semantics) an
    /// animation on one property. Registers the work and returns
    /// immediately -- never blocks waiting for the animation to finish
    /// (§8's own design rule). Dispatch is two-level per §8's review
    /// note: `PaintProperties`' own fields first (universal, every kind
    /// has them), then the node's `NodeKind` payload's fields if it has
    /// one -- `Text`'s `TextState` has none yet (§14 step 4 added no
    /// `Animated` fields to it), so that second level currently always
    /// falls through to `UnknownProperty`, which is the honest, correct
    /// behavior today, not a gap.
    #[pyo3(signature = (property, to, duration_ms=0))]
    pub(crate) fn animate(
        &self,
        property: &str,
        to: Bound<'_, PyAny>,
        duration_ms: u64,
    ) -> PyResult<()> {
        let duration = Duration::from_millis(duration_ms);
        let now = Instant::now();
        let mut tree = self.tree.borrow_mut();
        let node = tree.get_mut(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);

        match property {
            "opacity" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .opacity
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "corner_radius" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .corner_radius
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "elevation" => {
                let value = extract_f64(&to, property)?;
                node.paint
                    .elevation
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            "background" => {
                let value = extract_color(&to, property)?;
                node.paint
                    .background
                    .animate_to(value, duration, MotionCurve::Linear, now);
            }
            _ => {
                return Err(EngineError::UnknownProperty {
                    kind,
                    property: property.to_string(),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Reads a numeric property's current (possibly still-animating)
    /// value -- `animate()`'s missing counterpart, added at §14 step 12
    /// once something (a binding's own applied value, §16.2) actually
    /// needed to be observed from Python rather than only ever written.
    /// `background` isn't included: it isn't a single `f64`, and
    /// nothing yet needs to read it back.
    fn get(&self, property: &str) -> PyResult<f64> {
        let tree = self.tree.borrow();
        let node = tree.get(self.id).expect(
            "Node holds a NodeId missing from its own Tree -- an engine-py bug, not a user error",
        );
        let kind = kind_name(&node.kind);
        match property {
            "opacity" => Ok(node.paint.opacity.current),
            "corner_radius" => Ok(node.paint.corner_radius.current),
            "elevation" => Ok(node.paint.elevation.current),
            _ => Err(EngineError::UnknownProperty {
                kind,
                property: property.to_string(),
            }
            .into()),
        }
    }
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Rect => "Rect",
        NodeKind::Container => "Container",
        NodeKind::Text(_) => "Text",
    }
}

fn type_name_of(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "<unknown type>".to_string())
}

fn extract_f64(to: &Bound<'_, PyAny>, property: &str) -> Result<f64, EngineError> {
    to.extract::<f64>().map_err(|_| EngineError::TypeMismatch {
        property: property.to_string(),
        expected: "a float",
        actual: type_name_of(to),
    })
}

fn extract_color(to: &Bound<'_, PyAny>, property: &str) -> Result<Color, EngineError> {
    to.extract::<(u8, u8, u8, u8)>()
        .map(|(r, g, b, a)| Color::from_rgba8(r, g, b, a))
        .map_err(|_| EngineError::TypeMismatch {
            property: property.to_string(),
            expected: "an (r, g, b, a) tuple of 0-255 ints",
            actual: type_name_of(to),
        })
}
