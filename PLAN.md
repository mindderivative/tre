# Plan: M7 Phase 2 — Elevation → Real Shadow Rendering (§7.2)

## Context

`node.paint.elevation.current` is a real, animatable, Python-settable
`PaintProperties` field that `paint_node` never reads — no node has
ever actually painted a shadow, despite `build_shadow_scene` (M3 step
8) already proving `vello_hybrid::Scene::fill_blurred_rounded_rect`
works against this pinned version.

## Investigation before writing code

- **`engine-render` cannot depend on `engine-md3`** — confirmed
  directly from `ARCHITECTURE.md` §4's own dependency diagram: the only
  edge into `engine-render` is `H --> D` (`engine-core`); there is no
  `H --> G` (`engine-md3`) edge anywhere in the diagram or its prose.
  `engine-md3` hands out *resolved* `engine-core` values to `engine-py`/
  `engine-spec`; it never feeds `engine-render` directly. This matches
  the already-established real precedent for ripple/hover (M4 Phase 5):
  that state-layer math (a hardcoded black tint, not a resolved MD3
  scheme token) is computed directly inside `paint_node`, in
  `engine-render` itself, not delegated to `engine-md3`. Elevation
  follows the identical shape: the shadow-geometry math lives directly
  in `engine-render`, not a new `engine-md3` module.
- **The exact MD3 elevation shadow values had to be verified against a
  real, authoritative source, not recalled or guessed.** A first web
  source gave single-layer shadow values explicitly marked "alpha-stage
  ... may change before stable release" — rejected. Found and fetched
  the real, current, authoritative source directly: Material Web's own
  `elevation/internal/_elevation.scss` (the actual production CSS
  Google ships), which documents the exact per-level values in its own
  comments and encodes them as a continuous, animatable piecewise-linear
  formula in `--_level` (not a hard integer lookup) — exactly the shape
  needed for `PaintProperties.elevation` being a real `Animated<f64>`,
  not a discrete 0-5 enum. Two stacked shadow layers per elevation,
  confirmed: a "key" shadow (opacity 0.3, no spread) and an "ambient"
  shadow (opacity 0.15, with spread) — verified by hand-checking the
  formula reproduces the documented value at all 6 integer levels for
  both layers before trusting it.
- **The CSS blur-radius → Gaussian standard-deviation conversion also
  needed real verification**, not assumption: confirmed against the W3C
  CSS Backgrounds and Borders Module Level 3 spec directly — "a Gaussian
  blur with a standard deviation equal to half the blur radius." So
  `std_dev = blur_px / 2.0`, not `blur_px` directly.
- **The shadow color doesn't need to wait for Phase 3's dynamic-color
  wiring.** MD3's `shadow` color role is computed from the neutral
  palette's own tone-0 (blackest) position — confirmed via search,
  constant black regardless of the active theme's seed color, unlike
  `on-surface` (ripple/hover's own still-open gap). Real black
  (`Color::from_rgba8(0, 0, 0, ...)`), scaled by each layer's own
  0.3/0.15 opacity, is the *correct* MD3 answer today, not a
  placeholder standing in for Phase 3.
- **Applies uniformly to every `NodeKind`, not just `Rect`/`Splitter`**
  — `elevation` is a universal `PaintProperties` field, and a shadow is
  just "draw two blurred rects at this node's own bounds, behind
  everything else, if elevation > 0," independent of what the node
  actually draws on top. Painted once, at the very top of `paint_node`
  (before the `match &node.kind` that draws the node's own content),
  the same "zero special-casing" shape transform composition (M5 Phase
  1) and ripple/hover (M4 Phase 5) already established. Skipped
  entirely when `elevation <= 0.0` (matching level 0's own real
  `0px 0px 0px 0px` values) — not a degenerate zero-blur draw call.

## Approach

1. **`engine-render/src/lib.rs`**: two private functions,
   `key_shadow_geometry(level: f64) -> (f32 offset_y, f32 blur)` and
   `ambient_shadow_geometry(level: f64) -> (f32 offset_y, f32 blur, f32
   spread)`, transcribing Material Web's own verified piecewise-linear
   formula exactly (each term individually commented with which real
   documented level value it reproduces). `paint_node` gains a shadow-
   painting step before its `match`: if `elevation > 0.0`, paints the
   ambient layer then the key layer (ambient first, matching real
   `box-shadow` stacking order — later shadows paint on top), each via
   `Scene::fill_blurred_rounded_rect` with `std_dev = blur / 2.0`, the
   node's own local `(0, 0, w, h)` rect shifted by `offset_y` and (for
   ambient) inflated by `spread`, using its own `corner_radius`.
2. **New `engine-render` pixel-readback test**: an elevated `Rect`
   paints real shadow pixels below/around it (not just the rect's own
   fill), and a `Rect` with `elevation == 0.0` paints no shadow at all
   (byte-for-byte identical to a plain fill, the same "provably a
   no-op" standard every additive M5/M7 feature has been held to).
3. **New `engine-core` unit tests** (if the geometry helpers are more
   naturally tested in isolation): the piecewise formula reproduces
   Material Web's own documented value at each of the 6 real integer
   levels, for both layers — the same "match the real spec" rigor M7
   Phase 1's own `Emphasized` landmark test used.

## Files to touch

- `crates/engine-render/src/lib.rs` — shadow geometry helpers,
  `paint_node` wiring, new pixel test.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- Full pre-existing pixel-readback suite must stay green unmodified —
  every existing node has `elevation.current == 0.0` by default
  (`PaintProperties::new`'s own signature), so this must be a true
  no-op for all of them.
- `maturin develop && python -m pytest tests/ -v` plus all examples run
  (no `engine-py` changes expected, but confirmed, not assumed).
