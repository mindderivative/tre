//! §5's core data model: `NodeId`, `Node`, `NodeKind`, `PaintProperties`.
//!
//! Deliberately narrower than ARCHITECTURE.md §5's full struct shapes, for
//! this build-order step (§14 step 3, "layout of multiple static nodes"):
//!
//! - `Node` omits `access: AccessNodeData` (AccessKit wiring is step 7)
//!   and `interaction: Option<InteractionState>` (ripple/state-layer is
//!   §7.3, step 9) -- neither has anything to feed yet.
//! - `PaintProperties` omits `transform: Animated<kurbo::Affine>` (needs
//!   a non-trivial `Interpolate` impl -- matrix decomposition -- no step
//!   before its own real use needs) and `shape: Animated<ShapeKey>` (the
//!   §7.4 shape-correspondence-then-lerp technique, same reasoning).
//! - `NodeKind` carries only `Rect`/`Container`, the two kinds this step's
//!   layout demo actually needs; `Text`/`Image`/`Slider`/`Checkbox`/
//!   `Canvas` land with their own build-order steps.
//!
//! Each omission is additive to restore later, per Design Principle 5 and
//! the same "don't build ahead of need" discipline already applied to
//! `MotionCurve` (step 2, `Linear`-only) and here again to `NodeKind`.

use std::time::Instant;

use peniko::Color;
use taffy::Style;

use crate::animation::Animated;

slotmap::new_key_type! {
    /// A generational index (slot index + reuse generation), matching
    /// §5's own `NodeId { index: u32, generation: u32 }` doc comment
    /// exactly in spirit: a stale `NodeId` from a removed node fails a
    /// checked generation comparison instead of silently resolving to
    /// whatever now occupies that slot (§1 Locked Decisions).
    ///
    /// Built on `slotmap`'s key type rather than a hand-rolled struct:
    /// `taffy` (this crate's own dependency) already pulls in `slotmap`
    /// transitively at the exact same version, so this costs no new
    /// entry in the dependency graph, and `SlotMap`/`SecondaryMap`'s
    /// generation-checked `get`/`remove` are exactly the semantics §5
    /// asks for.
    pub struct NodeId;
}

/// Component-specific animatable state lives here, per kind -- §1 Locked
/// Decisions ("common core + per-kind payload"). Both variants below
/// carry no payload beyond `PaintProperties`, so neither needs one yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Rect,
    Container,
}

/// Universal paint state every node has, regardless of `NodeKind`.
pub struct PaintProperties {
    pub background: Animated<Color>,
    pub corner_radius: Animated<f64>,
    pub elevation: Animated<f64>,
    pub opacity: Animated<f64>,
}

impl PaintProperties {
    pub fn new(background: Color, corner_radius: f64, elevation: f64, opacity: f64) -> Self {
        Self {
            background: Animated::new(background),
            corner_radius: Animated::new(corner_radius),
            elevation: Animated::new(elevation),
            opacity: Animated::new(opacity),
        }
    }

    /// The per-node half of the central tick (§5): advances every owned
    /// `Animated<T>`, returning `true` if any is still mid-animation.
    /// `Tree::tick_all` calls this for every node -- a naive whole-tree
    /// walk, not the "walks only the active set" scoped version §5
    /// describes. That scoping is real dirty-tracking machinery that
    /// belongs to §6's per-frame pipeline design, not manufactured here
    /// ahead of a step that profiles it as actually necessary; the
    /// frame-time CI benchmark this same step adds is exactly what would
    /// catch it if a naive walk ever stopped meeting the 16.6ms budget.
    pub fn tick(&mut self, now: Instant) -> bool {
        let background = self.background.tick(now);
        let corner_radius = self.corner_radius.tick(now);
        let elevation = self.elevation.tick(now);
        let opacity = self.opacity.tick(now);
        background || corner_radius || elevation || opacity
    }
}

pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub layout_style: Style,
    pub paint: PaintProperties,
}
