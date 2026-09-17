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
    /// M14 Phase 1 (§5, §7.3): a real MD3 checkbox. `checked` is plain,
    /// app-owned state (Design Principle 6 -- "selection/checked-state
    /// ... depend on what the app's data means," not anything the
    /// engine determines on its own); `check_progress` is the engine-
    /// driven visual consequence, animated toward `1.0`/`0.0` whenever
    /// the app sets `checked`. The engine deliberately does not toggle
    /// `checked` on click itself -- the already-generic `Click`
    /// dispatch/`Node.enable_interaction()` ripple mechanism (works for
    /// any `NodeKind` already) is what this phase reuses unchanged; an
    /// app's own `on_click` handler is what actually flips `checked`.
    Checkbox(CheckboxState),
    /// M14 Phase 2 (§5, §7.3): a real MD3 slider. `thumb_position` *is*
    /// the real value (0.0..=1.0 along the track, the identical shape
    /// `SplitterState.position` already has) -- no separate field, the
    /// same "the animated field is the value" precedent. A real drag
    /// (`Tree::set_slider_position`) sets it the same instant, `Duration
    /// ::ZERO` + immediate-manual-tick way `SplitterState.position`
    /// already does, live-following the pointer. Unlike `SplitterState.
    /// position` (never exposed to `Node.animate()` at all -- confirmed
    /// via direct read, so it never needs central ticking), `thumb_
    /// position` *is* exposed (`"thumb_position"`, this phase's own
    /// second real kind-payload `animate()` arm) for a real, app-
    /// triggered eased move (e.g. a keyboard nudge, not a drag) -- real
    /// finding while designing this: that path needs `Tree::tick_all`
    /// to actually tick it centrally, or a nonzero-duration `animate()`
    /// call would set an active animation that never progresses. Ticked
    /// there, unconditionally, alongside `CheckboxState.check_progress`.
    Slider(SliderState),
    /// M15 Phase 1 (§5, §16.7): a real, single-line editable text
    /// field. `content`/`cursor`/`selection_anchor` are plain, engine-
    /// core-native byte-offset state -- Design Principle 6's own
    /// "app-owned meaning" shape `CheckboxState.checked` already
    /// established, except here the *engine* is the one real mutator
    /// (via `Tree::dispatch`'s own keyboard-editing arm, M15 Phase 2),
    /// since typing is mechanical, not app-defined meaning the way a
    /// checkbox's "checked" is. Deliberately does *not* carry a
    /// `parley::Layout`/`Selection` directly: `engine-core` has no
    /// `parley` dependency at all (§4's crate-boundary rule) --
    /// `engine-render` reconstructs both, each frame, purely to compute
    /// real caret/highlight paint geometry, the same "engine-core holds
    /// inert data, engine-render re-derives it" split `TextState`
    /// itself already uses.
    TextField(TextFieldState),
}

/// M15 Phase 1 (§5, §16.7): mirrors `TextState`'s own four font/content
/// fields exactly (so `engine-render`'s own layout-building code can be
/// shared between the two), plus real editable-field state. `cursor`/
/// `selection_anchor` are plain UTF-8 *byte* offsets into `content`,
/// not char or grapheme-cluster indices -- `String` slicing/`char_
/// indices` are what M15 Phase 2's own real keyboard-editing mutation
/// uses to keep every offset on a real UTF-8 boundary; `engine-core`
/// itself never validates this beyond what those std APIs already
/// guarantee, since `content` is never sliced at an arbitrary offset
/// here, only ever at boundaries `char_indices` itself produced.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldState {
    pub content: String,
    pub font_family: String,
    pub font_weight: f32,
    pub font_size: f32,
    /// A real UTF-8 byte offset into `content`, `0..=content.len()`.
    pub cursor: usize,
    /// `Some(byte_offset)` when a real selection is active (`cursor`
    /// is the selection's own "focus" end, this is its "anchor" end,
    /// the identical two-endpoint shape `parley::editing::Selection`
    /// itself uses) -- `None` (the default) means no selection, the
    /// overwhelmingly common case for a freshly-created field.
    pub selection_anchor: Option<usize>,
    /// M17 Phase 2 (§8): a real, *uncommitted* IME composition preview
    /// -- `Some` while an input method is composing (e.g. pinyin
    /// candidates before the user picks one), `None` otherwise.
    /// Deliberately not spliced into `content` itself: nothing is
    /// really "typed" until a real `winit::event::Ime::Commit`, which
    /// reaches the exact same `TextInput` mechanism a plain keypress
    /// already uses (M15 Phase 2) -- this field exists purely so
    /// `engine-render` can paint the real, visible in-progress preview
    /// (with a real underline) without ever mutating real content for
    /// text that might still be revised or cancelled mid-composition.
    /// Drops `winit::event::Ime::Preedit`'s own real sub-cursor range
    /// (`Option<(usize, usize)>`, where *within* the preedit text the
    /// composition cursor sits) -- real, but strictly more detail than
    /// "a preedit underline" needs; a real, stated simplification.
    pub preedit: Option<String>,
    /// M20 Phase 2 (§7.1, §7.3): the field's own real text/caret/
    /// selection-highlight color -- `CheckboxState.mark_tint`'s own
    /// real sibling, same reasoning. Defaults to real, byte-for-byte
    /// the historical hardcoded `0x1C1B1F` `engine-render`'s own
    /// `TextField` paint used before this phase.
    pub text_tint: Color,
}

impl TextFieldState {
    /// Seeds `cursor` at `content`'s own real end -- a real text
    /// field's own real, expected initial-cursor-at-end convention
    /// (every desktop toolkit's own default), not `0`.
    pub fn new(
        content: impl Into<String>,
        font_family: impl Into<String>,
        font_weight: f32,
        font_size: f32,
    ) -> Self {
        let content = content.into();
        let cursor = content.len();
        Self {
            content,
            font_family: font_family.into(),
            font_weight,
            font_size,
            cursor,
            selection_anchor: None,
            preedit: None,
            text_tint: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
        }
    }
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

/// §5's own struct sketch, plus the real `checked: bool` §7.3's own
/// text separately names (the struct sketch only showed the animated
/// half). `check_progress` starts already matching `checked` (`1.0`
/// for checked, `0.0` for not) so a checkbox created already-checked
/// shows its own real initial state without a spurious animation from
/// `0.0` the instant it first paints.
pub struct CheckboxState {
    pub checked: bool,
    pub check_progress: Animated<f64>,
    /// M20 Phase 1 (§7.1, §7.3): the checkmark's own real paint color
    /// -- plain, already-resolved data (Design Principle 6), same as
    /// `InteractionState.tint`'s own real precedent (M7 Phase 3), but
    /// living here since every real `CheckboxState` always has one
    /// (not an optional opt-in capability the way `InteractionState`
    /// is). Defaults to real white -- byte-for-byte the hardcoded
    /// value `engine-render`'s own checkmark paint used before this
    /// phase, so a checkbox whose app never calls `Window.set_theme`
    /// sees zero visual change.
    pub mark_tint: Color,
}

impl CheckboxState {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            check_progress: Animated::new(if checked { 1.0 } else { 0.0 }),
            mark_tint: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF),
        }
    }
}

/// §5's own struct sketch, unchanged in shape -- `thumb_position` is
/// the real value itself, `0.0..=1.0` along the track, the identical
/// shape `SplitterState.position` already established.
pub struct SliderState {
    pub thumb_position: Animated<f64>,
    /// M20 Phase 1 (§7.1, §7.3): the track's own real paint color --
    /// `CheckboxState.mark_tint`'s own real sibling, same reasoning.
    /// The thumb itself already paints with the node's own real,
    /// per-node `PaintProperties.background` (already themeable
    /// directly) -- only the track ever needed a fixed literal.
    /// Defaults to real byte-for-byte the historical hardcoded gray.
    pub track_tint: Color,
}

impl SliderState {
    pub fn new(value: f64) -> Self {
        Self {
            thumb_position: Animated::new(value.clamp(0.0, 1.0)),
            track_tint: Color::from_rgba8(0x79, 0x74, 0x7A, 0xFF),
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
