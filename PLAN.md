# Plan: M7 Phase 4 — Shape Morphing, Wired for Real (§7.4)

Corresponds to `BUILD_TRACKER.md` M7 Phase 4's own scoping: `Paint
Properties` (or a `NodeKind::Rect`-specific payload) gains a real
`Animated<ShapeKey>` field wired into `paint_node`'s `Rect` fill path
(Step 1), plus a Python-facing way to trigger a morph between two real
shapes (Step 2).

## Investigation before writing code

- `engine_md3::shape_morph::ShapeKey` (M3 step 10) is real, tested,
  proven math — `ShapeKey::from_path(&BezPath)`/`to_path(&self) ->
  BezPath`, `impl Interpolate for ShapeKey` (a real correspondence-
  search-then-lerp, not naive per-index pairing). Confirmed via grep:
  **zero real consumers anywhere in the codebase** — matches this
  milestone's own scoping text ("zero integration").
- **Real finding: `ShapeKey` has no actual MD3-specific content at
  all.** Read the full 336-line file — its only imports are
  `engine_core::Interpolate` and `peniko::kurbo::{BezPath, PathEl,
  Point}`. No `ColorScheme`, no `DynamicTheme`, no `material_colors`.
  It's pure vertex-correspondence geometry, organizationally under
  "§7.4" (an MD3 spec section) but not technically MD3 domain
  knowledge the way color science genuinely is.
- **Direct textual confirmation this was always meant to live in
  `engine-core`:** `ARCHITECTURE.md`'s own §2 struct sketch (line ~207)
  already writes `pub shape: Animated<ShapeKey>` directly inside
  `PaintProperties` — the same struct that is now `engine-core::node::
  PaintProperties`. `engine-core/src/node.rs`'s own module doc comment
  states explicitly: "`shape: Animated<ShapeKey>` (the §7.4 shape-
  correspondence-then-lerp technique) remains omitted, same reasoning
  as ever: additive whenever its own step needs it" — i.e. always
  intended to land directly in `PaintProperties`, not routed through a
  separate crate.
- **This is the identical shape M7 Phase 1 already resolved for
  `MotionCurve`:** real MD3 curve *values* landed directly in
  `engine-core`, not a separate `engine_md3::motion` module, because
  the underlying math has no real MD3-specific dependency and `engine-
  core` already owns `Animated`/`Interpolate`. `engine-md3`'s own
  `Cargo.toml` description ("color scheme, shadow/ripple helpers, shape
  morph, motion-curve presets") is stale evidence of this drift — it
  still lists "motion-curve presets" despite Phase 1 already moving
  that reality into `engine-core`.
- `engine-core` cannot depend on `engine-md3` (§4, re-confirmed every
  phase this milestone) — so `PaintProperties` (engine-core) can never
  hold an `engine_md3::ShapeKey` field as long as `ShapeKey` stays in
  `engine-md3`. Given the two facts above, the correct resolution is
  the same one Phase 1 already established for `MotionCurve`: **move
  `ShapeKey`'s implementation from `engine-md3::shape_morph` into
  `engine-core::shape_morph`**, not invent a workaround. Verified this
  is a clean move: nothing outside `crates/engine-md3/src/shape_morph.
  rs` and `crates/engine-md3/src/lib.rs` (its own `mod`/`pub use`)
  references `ShapeKey`/`shape_morph` anywhere (grep, zero hits for
  `engine_md3::ShapeKey`/`engine_md3::shape_morph` in the rest of the
  workspace) — no re-export needed for backward compatibility.
- `PaintProperties::new(background, corner_radius, elevation, opacity)`
  is the single real construction choke point (confirmed via grep —
  no other `PaintProperties { ... }` struct literal exists anywhere);
  `transform` was added there directly at M5 Phase 1 the same way
  `shape` will be here.
- `paint_node`'s `NodeKind::Rect | NodeKind::Splitter(_)` arm currently
  always builds `RoundedRect::new(0.0, 0.0, w, h, radius).to_path(0.1)`.
  An "active morph" needs a real way to tell "this node has a real
  shape set" from "this node never touched `shape` at all" — `ShapeKey`
  has no `Default`; a fresh, empty `ShapeKey { points: vec![] }` (via a
  new `ShapeKey::empty()`/`Default` impl) is the natural "no shape set"
  sentinel, since `to_path()` already renders an empty `BezPath` for an
  empty point list (confirmed by reading `to_path`'s own `if let
  Some(&first) = points.next()` short-circuit) — no new enum/`Option`
  wrapper needed.
- Python-facing trigger (Step 2): the same `animate()` dispatch shape
  M6 Phase 2 added for `"transform"` — a new `"shape"` arm taking two
  lists of `(x, y)` tuples (the target shape's own vertices, since
  Python has no `BezPath` type to hand over) is the natural surface;
  `ShapeKey`'s own constructor is `from_path(&BezPath)`, so the new
  Python arm builds a `BezPath` from the given points (`move_to` +
  `line_to`s + `close_path`) and calls `ShapeKey::from_path` on it,
  mirroring `extract_translate_scale`'s own "convert Python's plain
  tuples into the real engine-core value" role for `"transform"`.

## Design

- Move `crates/engine-md3/src/shape_morph.rs` to `crates/engine-core/
  src/shape_morph.rs` verbatim (only its `use engine_core::Interpolate`
  import line changes, to `use crate::animation::Interpolate`).
  `engine-core/src/lib.rs` gains `mod shape_morph;` / `pub use
  shape_morph::ShapeKey;`. `engine-md3/src/lib.rs` drops `mod
  shape_morph;`/its own `pub use` (no re-export needed, confirmed zero
  consumers) and its own module doc comment's "step 10" reference is
  corrected to note the real move. `engine-md3/Cargo.toml`'s
  description drops "shape morph" (and, while touching it, the already
  known-stale "motion-curve presets" — both real, in-scope corrections
  of the same drift this investigation found).
- `ShapeKey` gains `pub fn empty() -> Self` (an empty point list) and
  `pub fn is_empty(&self) -> bool` — the "no morph set" sentinel
  `PaintProperties::new` uses and `paint_node` checks.
- `PaintProperties` gains `pub shape: Animated<ShapeKey>`, initialized
  to `Animated::new(ShapeKey::empty())` in `::new()`; `PaintProperties::
  tick` advances it like every other field.
- `paint_node`'s `Rect | Splitter(_)` arm: if `!node.paint.shape.
  current.is_empty()`, fill `node.paint.shape.current.to_path()`
  instead of the plain `RoundedRect` — an inactive/never-set shape is a
  true no-op, byte-for-byte the existing rendering, matching every
  additive `PaintProperties` field's own contract in this codebase.
- `engine-py::Node::animate`'s dispatch gains a `"shape"` arm: takes a
  `Vec<(f64, f64)>` of target vertices, builds a closed `BezPath`,
  converts via `ShapeKey::from_path`, and calls `animate_to` on `paint.
  shape` the same way `"transform"`'s arm does today.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — the full, unmodified
  `shape_morph.rs` test suite must still pass after the move (byte-for-
  byte relocated, only the `use` line differs). New tests: `ShapeKey::
  empty()`/`is_empty()`; `PaintProperties::new` defaults to an empty
  shape (`paint_node` no-op, same standard every additive feature has
  been held to since M5 Phase 1); a real `engine-render` pixel test
  proving an active morph paints the *morphed* silhouette, not a plain
  rounded rect.
- `maturin develop --release` + `pytest tests/` + all examples. New
  `examples/shape_morph.py`: a rect morphing into a triangle via
  `Node.animate("shape", ...)`.
