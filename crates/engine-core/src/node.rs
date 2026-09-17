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
//!   technique) landed at M7 Phase 4, once `ShapeKey` itself moved here
//!   from `engine-md3` (see `shape_morph.rs`'s own doc comment).
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

use crate::canvas::CanvasState;

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
    /// M5 Phase 3 (§11.10, §11.11): custom-drawn content plus an
    /// optional custom hit-test override. `CanvasState` is plain, inert
    /// data -- no `Py<PyAny>` -- resolved ahead of time by
    /// `Tree::set_canvas_content`, not computed live during paint or
    /// hit-testing (see `canvas.rs`'s own module doc comment for why).
    Canvas(CanvasState),
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
    /// M8 Phase 2 (§11.7): the real vertical scroll position, in local
    /// pixels -- composed into materialized children's own effective
    /// paint-time position (`paint_node`), not their `layout_style`
    /// (their real taffy layout never changes; only where they're
    /// *drawn* does). Plain `Animated<f64>`, driven directly (like a
    /// scrollbar being dragged, not eased toward a target) the same way
    /// `SplitterState.position` already is -- confirmed via reading
    /// `Tree::tick_all` directly that kind-specific `Animated<T>`
    /// fields are never ticked centrally, only by their own dedicated
    /// mechanism, so this follows that same real precedent rather than
    /// inventing a new one.
    pub scroll_offset: Animated<f64>,
    /// M12 Phase 1 (§11.7): real, resolved cumulative offsets for
    /// `Variable`-extent lists -- index `idx` maps to item `idx`'s own
    /// real top-offset (not its height). Always empty for `Fixed`
    /// (whose offsets are computed directly, `idx * item_extent`). The
    /// key `item_count` (one past the last real item, the same "one
    /// past the end" convention a `Range` already uses) holds the real
    /// total content extent. `engine-core` never computes a cumulative
    /// sum itself here -- only looks a resolved value up (`offset_of`/
    /// `total_extent` below); §4's own pyo3-agnostic boundary is why
    /// the actual per-item size-hint resolution lives in `engine-py`
    /// (populated via `Tree::set_virtual_list_resolved_offsets`), the
    /// same real reason `materialize` itself lives there too, not here.
    pub resolved_offsets: std::collections::BTreeMap<usize, f64>,
}

impl VirtualListState {
    pub fn new(item_count: usize, item_extent: ItemExtent) -> Self {
        Self {
            item_count,
            item_extent,
            materialized: std::collections::BTreeMap::new(),
            scroll_offset: Animated::new(0.0),
            resolved_offsets: std::collections::BTreeMap::new(),
        }
    }

    /// Item `idx`'s own real top-offset. Panics if `item_extent` is
    /// `Variable` and `idx` isn't yet resolved in `resolved_offsets` --
    /// an internal bookkeeping bug (the caller must resolve an index
    /// before positioning it), the same "internal bug, not a runtime
    /// condition" contract `set_splitter_position` already uses for its
    /// own malformed-call panics.
    pub fn offset_of(&self, idx: usize) -> f64 {
        match &self.item_extent {
            ItemExtent::Fixed(v) => idx as f64 * v,
            ItemExtent::Variable => *self.resolved_offsets.get(&idx).unwrap_or_else(|| {
                panic!(
                    "VirtualListState::offset_of: item {idx}'s own offset must be resolved \
                     (via Tree::set_virtual_list_resolved_offsets) before it can be positioned"
                )
            }),
        }
    }

    /// The real total content extent across every item -- `item_count *
    /// item_extent` for `Fixed`, or `offset_of(item_count)` (the
    /// resolved offset "one past the last item") for `Variable`.
    pub fn total_extent(&self) -> f64 {
        match &self.item_extent {
            ItemExtent::Fixed(v) => self.item_count as f64 * v,
            ItemExtent::Variable => self.offset_of(self.item_count),
        }
    }
}

/// §11.7's own text: "fixed, or a size-hint callback for variable-height
/// items." M12 Phase 1 (§11.7): `Variable` is real now -- a fieldless
/// marker, deliberately; the actual per-item data lives on `VirtualList
/// State::resolved_offsets`, not this enum itself, since only `Virtual
/// ListState` has a `Tree`-mutation path (`Tree::set_virtual_list_
/// resolved_offsets`) to populate it. No callback lives in `engine-core`
/// itself -- §4's own pyo3-agnostic boundary rules that out, the same
/// real reason `materialize` lives in `engine-py`, not here; resolving
/// a real per-item size-hint from Python is Phase 2's own concern.
pub enum ItemExtent {
    Fixed(f64),
    Variable,
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
    /// M7 Phase 4 (§7.4): the real MD3 shape-morph target -- defaults
    /// to `ShapeKey::empty()` (no shape ever set), which `paint_node`
    /// treats as a true no-op, matching every other additive field's
    /// "off unless a caller opts in" contract. See `shape_morph.rs`'s
    /// own module doc comment for why this lives here, in
    /// `engine-core`, and not `engine-md3` (where it was originally
    /// built, M3 step 10).
    pub shape: Animated<crate::shape_morph::ShapeKey>,
}

impl PaintProperties {
    pub fn new(background: Color, corner_radius: f64, elevation: f64, opacity: f64) -> Self {
        Self {
            background: Animated::new(background),
            corner_radius: Animated::new(corner_radius),
            elevation: Animated::new(elevation),
            opacity: Animated::new(opacity),
            transform: Animated::new(peniko::kurbo::Affine::IDENTITY),
            shape: Animated::new(crate::shape_morph::ShapeKey::empty()),
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
    pub fn tick(&mut self, now: Instant, completed: &mut Vec<crate::CompletionHandle>) -> bool {
        let background = self.background.tick(now, completed);
        let corner_radius = self.corner_radius.tick(now, completed);
        let elevation = self.elevation.tick(now, completed);
        let opacity = self.opacity.tick(now, completed);
        let transform = self.transform.tick(now, completed);
        let shape = self.shape.tick(now, completed);
        background || corner_radius || elevation || opacity || transform || shape
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
