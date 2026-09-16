# Log: M7 Phase 4 — Shape Morphing, Wired for Real (§7.4)

Corresponds to `BUILD_TRACKER.md` M7 Phase 4. Two steps: `Paint
Properties` gains a real `Animated<ShapeKey>` field wired into
`paint_node`'s `Rect` fill path; a Python-facing way to trigger a morph
between two real shapes.

## Investigation before writing code

- `engine_md3::shape_morph::ShapeKey` (M3 step 10) was real, tested,
  proven math with zero real consumers anywhere (confirmed via grep) --
  matching this milestone's own scoping text.
- **Real finding: `ShapeKey` has no actual MD3-specific content at
  all.** Read the full 336-line file — its only imports were
  `engine_core::Interpolate` and `peniko::kurbo::{BezPath, PathEl,
  Point}`. No `ColorScheme`/`DynamicTheme`/`material_colors`.
- **Direct textual confirmation this was always meant to live in
  `engine-core`:** `ARCHITECTURE.md` §2's own struct sketch already
  wrote `pub shape: Animated<ShapeKey>` directly inside `PaintProperties`
  — the same struct now `engine-core::node::PaintProperties`.
  `engine-core/src/node.rs`'s own module doc comment stated this field
  "remains omitted, same reasoning as ever: additive whenever its own
  step needs it" — i.e. always intended to land directly on
  `PaintProperties`, not routed through a separate crate.
- This is the identical shape M7 Phase 1 already resolved for
  `MotionCurve` — real MD3 values landed directly in `engine-core`
  since the underlying math has no genuine MD3 dependency and
  `engine-core` already owns `Animated`/`Interpolate`. `engine-md3`'s
  own `Cargo.toml` description ("shape morph, motion-curve presets")
  was stale evidence of exactly this drift.
- `engine-core` cannot depend on `engine-md3` (§4) — so `PaintProperties`
  could never hold an `engine_md3::ShapeKey` field as long as `ShapeKey`
  stayed in `engine-md3`. Resolution: moved `ShapeKey`'s implementation
  from `engine-md3::shape_morph` into `engine-core::shape_morph`
  verbatim (`git mv`, only the `use engine_core::Interpolate` import
  line changed to `use crate::animation::Interpolate`) — confirmed a
  clean move via grep (zero references to `engine_md3::ShapeKey`/
  `engine_md3::shape_morph` anywhere outside `engine-md3` itself), no
  backward-compat re-export needed.
- `PaintProperties::new` is the single real construction choke point
  (confirmed via grep, no other struct literal exists) — `transform`
  was added there directly at M5 Phase 1, `shape` the same way here.

## What happened

`engine-core`: `shape_morph.rs` moved in verbatim, gains `ShapeKey::
empty()`/`is_empty()` (the "no shape ever set" sentinel — `to_path()`
already renders an empty `BezPath` for an empty point list, so no new
`Option` wrapper was needed). `PaintProperties` gains `pub shape:
Animated<ShapeKey>`, defaulting to `ShapeKey::empty()`; `tick()` advances
it like every other field.

`engine-md3`: drops its own `shape_morph` module and `pub use`; module
doc comment and `Cargo.toml` description corrected to reflect the move
and the already-stale "motion-curve presets" claim from Phase 1.

`engine-render`: `paint_node`'s `Rect | Splitter(_)` arm now checks
`node.paint.shape.current.is_empty()` — empty (the default) still fills
the plain `RoundedRect`, byte-for-byte unchanged; non-empty fills
`shape.current.to_path()` instead.

`engine-py`: `Node::animate` gains a `"shape"` arm — a new
`extract_shape_points` helper (mirroring `extract_translate_scale`'s own
role) takes a list of `(x, y)` vertices, builds a closed `BezPath`, and
converts via `ShapeKey::from_path`, then `animate_to`s `paint.shape` the
same way `"transform"`'s arm already does.

New tests: `ShapeKey::empty()`/`is_empty()` (2, alongside the full,
unmodified pre-existing `shape_morph` test suite, now living in
`engine-core`); a new `engine-render` pixel test (`shape_morph_paint.
rs`) proving the default paints the unmodified plain rect, and a real
active morph (a triangle) paints the real silhouette — a corner inside
the rect's bounding box but outside the triangle is plain background,
not fill. New `examples/shape_morph.py`: a rect morphing into a
triangle via `Node.animate("shape", ...)`.

Full `cargo test --workspace --release` clean (`engine-core` 65, up from
58 — the relocated `shape_morph` suite plus 2 new tests; `engine-md3`
down to 4, having lost that same suite; `engine-render` +2), `cargo
clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
all clean — every prior test passed unmodified. `maturin develop
--release` + full `pytest tests/` (78 passed, 1 skipped, unaffected) and
all thirteen examples (twelve existing + new `shape_morph.py`) confirmed
clean.
