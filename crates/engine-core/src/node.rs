//! §5's core data model: `NodeId`, `Node`, `NodeKind`, `PaintProperties`.
//!
//! Deliberately narrower than ARCHITECTURE.md §5's full struct shapes, for
//! this build-order step (§14 step 3, "layout of multiple static nodes"):
//!
//! - `Node` originally omitted `access: AccessNodeData` (AccessKit
//!   wiring, step 7) and `interaction: Option<InteractionState>`
//!   (ripple/state-layer, §7.3, step 9); both fields have since landed
//!   at their own steps, per Design Principle 5's "add it when its own
//!   step needs it" discipline.
//! - `PaintProperties` originally omitted `transform: Animated<kurbo::
//!   Affine>` (needed a non-trivial `Interpolate` impl); it landed at
//!   M5 Phase 1 (§11.9) as a plain componentwise coefficient lerp, not
//!   a rotation-aware decomposition -- see `animation.rs`'s own
//!   `Interpolate for Affine` doc comment for why that's still correct
//!   for this framework's actual pan/zoom scope. `shape:
//!   Animated<ShapeKey>` (the §7.4 shape-correspondence-then-lerp
//!   technique) remains omitted, same reasoning as ever: additive
//!   whenever its own step needs it.
//! - `NodeKind` carried only `Rect`/`Container` through step 3;
//!   `Text(TextState)` is added at step 4 (§14 step 4, the typography
//!   spike). `Image`/`Slider`/`Checkbox`/`Canvas` still land with their
//!   own later build-order steps.
//!
//! Each omission is additive to restore later, per Design Principle 5 and
//! the same "don't build ahead of need" discipline already applied to
//! `MotionCurve` (step 2, `Linear`-only) and here again to `NodeKind`.
//! `TextState` itself is narrower than a real future MD3 text component
//! will need: no `Animated` fields (no MD3 component in scope yet
//! animates a text property -- cursor blink, reveal-on-scroll, etc. --
//! so none is manufactured here ahead of a step that needs one), no
//! wrapping/overflow policy (this step measures `parley`'s own
//! line-breaking against a fixed box width, it doesn't design a CSS-like
//! overflow model).

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
/// Decisions ("common core + per-kind payload"). `Rect`/`Container`
/// carry no payload beyond `PaintProperties`; `Text` carries `TextState`
/// (§14 step 4).
/// No longer derives `Clone`/`Debug`/`PartialEq` now that `Splitter`
/// carries an `Animated<f64>` -- `Animated<T>` implements none of those
/// (same reason `PaintProperties`, also full of `Animated` fields,
/// never derived them either); nothing in this codebase actually
/// cloned, printed, or compared a `NodeKind` value directly (checked
/// directly, not assumed), so this costs nothing real.
pub enum NodeKind {
    Rect,
    Container,
    Text(TextState),
    /// §14 step 15 (§11.5): a draggable divider between two sibling
    /// regions. Carries no paint of its own beyond `PaintProperties`
    /// (a real splitter typically just wants a `background` for its
    /// own grip/handle) -- `SplitterState` is purely the mechanism's
    /// own animatable position, not appearance.
    Splitter(SplitterState),
    /// §14 step 15 (§11.7): a windowed logical list -- only the small
    /// visible-window subset in `VirtualListState::materialized` are
    /// ever real `Node`s, regardless of `item_count`.
    VirtualList(VirtualListState),
}

/// §11.7's own struct sketch, unchanged in shape (`item_count`,
/// `item_extent`, `materialized`). `materialized`'s values are exactly
/// this node's own `children` (§5) with "which logical index" attached
/// on top -- not a second, separately-tracked child set; `Tree::
/// set_virtual_list_window` is what keeps the two in lockstep, the same
/// "engine-core carries inert state, a `Tree` method is what makes it do
/// anything" shape every other `NodeKind` payload already uses.
pub struct VirtualListState {
    pub item_count: usize,
    pub item_extent: ItemExtent,
    pub materialized: std::collections::BTreeMap<usize, NodeId>,
}

impl VirtualListState {
    pub fn new(item_count: usize, item_extent: ItemExtent) -> Self {
        Self {
            item_count,
            item_extent,
            materialized: std::collections::BTreeMap::new(),
        }
    }
}

/// §11.7's own text: "fixed, or a size-hint callback for variable-height
/// items." Only `Fixed` is built here -- no consumer needs the
/// callback-based variant yet (the same "additive when its own step
/// needs it" discipline this module's own doc comment already applies
/// to `set_on_click`), and a per-item Rust/Python size-hint callback
/// raises the exact same real PyObject-callback-storage/GC design
/// question `set_on_click` is itself still deferred over -- not
/// reintroduced here ahead of a real consumer.
pub enum ItemExtent {
    Fixed(f64),
}

impl ItemExtent {
    pub(crate) fn value(&self) -> f64 {
        match self {
            ItemExtent::Fixed(v) => *v,
        }
    }
}

/// §11.5's own struct sketch, unchanged: `position` is 0.0..=1.0 along
/// the split axis (not an absolute size -- "implementation detail" per
/// the architecture's own text, and a fraction is what lets the same
/// splitter keep working correctly if its parent is later resized).
/// `Tree::set_splitter_position` is what actually moves the two
/// flanking siblings this value nominally describes; the field alone
/// is inert data, matching every other `NodeKind` payload's own
/// "engine-core carries the state, a `Tree` method is what makes it do
/// anything" shape.
pub struct SplitterState {
    pub position: Animated<f64>,
}

/// A text node's content and shaping inputs -- everything `parley` needs
/// to shape a run, and nothing about how it got styled (that's
/// `engine_md3`'s future job, not this crate's -- `engine-core` stays
/// MD3-agnostic per §1 Locked Decisions). `font_family` names an
/// already-registered family (by exact name, matching the font's own
/// name table) rather than carrying a weight/style axis: this step's two type
/// roles are two distinct font files (Roboto Regular vs. Medium), not
/// one variable font interpolated at draw time.
#[derive(Clone, Debug, PartialEq)]
pub struct TextState {
    pub content: String,
    pub font_family: String,
    /// OpenType weight class (100.0..=950.0, matching CSS `font-weight`'s
    /// numeric range; 400.0 is normal). A real, non-obvious finding from
    /// wiring this up: distinct static weights of the same type family
    /// (e.g. Roboto Regular vs. Medium) commonly register under the
    /// *same* family name -- their typographic family name (OpenType
    /// name ID 16) is shared, only the subfamily (ID 17) differs -- so
    /// weight cannot be selected by family name alone. Confirmed
    /// directly against Roboto's own name table, not assumed.
    pub font_weight: f32,
    pub font_size: f32,
}

/// Universal paint state every node has, regardless of `NodeKind`.
pub struct PaintProperties {
    pub background: Animated<Color>,
    pub corner_radius: Animated<f64>,
    pub elevation: Animated<f64>,
    pub opacity: Animated<f64>,
    /// §11.9 (M5 Phase 1): composed down the tree during the paint walk
    /// -- a node's effective transform is its parent's effective
    /// transform composed with its own, exactly like nested `<g
    /// transform>` in SVG. Defaults to `Affine::IDENTITY`, so every
    /// node that never sets this paints exactly where its taffy layout
    /// already places it -- purely additive, matching every other
    /// `PaintProperties` field's own "off unless a caller opts in"
    /// shape.
    pub transform: Animated<peniko::kurbo::Affine>,
}

impl PaintProperties {
    pub fn new(background: Color, corner_radius: f64, elevation: f64, opacity: f64) -> Self {
        Self {
            background: Animated::new(background),
            corner_radius: Animated::new(corner_radius),
            elevation: Animated::new(elevation),
            opacity: Animated::new(opacity),
            transform: Animated::new(peniko::kurbo::Affine::IDENTITY),
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
        let transform = self.transform.tick(now);
        background || corner_radius || elevation || opacity || transform
    }
}

pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub layout_style: Style,
    pub paint: PaintProperties,
    /// §14 step 7: `AccessNodeData::default()` (`Role::Unknown`, no
    /// label/actions) unless a caller opts a node in via
    /// `Tree::set_access`.
    pub access: crate::access::AccessNodeData,
    /// §14 step 9 (§7.3): `None` until a caller opts a node into
    /// pointer interaction via `Tree::interaction_mut` -- most nodes
    /// (plain rects, text, containers) never touch this and pay nothing
    /// for it.
    pub interaction: Option<crate::interaction::InteractionState>,
}
