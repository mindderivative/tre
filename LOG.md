# Log: M7 Phase 2 — Elevation → Real Shadow Rendering (§7.2)

Corresponds to `BUILD_TRACKER.md` M7 Phase 2. `node.paint.elevation.
current` was a real, animatable, Python-settable `PaintProperties`
field that `paint_node` never read — confirmed no node has ever
actually painted a shadow, despite `build_shadow_scene` (M3 step 8)
already proving `vello_hybrid::Scene::fill_blurred_rounded_rect` works
against this pinned version.

## Investigation before writing code

- **`engine-render` cannot depend on `engine-md3`** — confirmed
  directly from `ARCHITECTURE.md` §4's own dependency diagram: the only
  edge into `engine-render` is from `engine-core`. This matches the
  already-established real precedent for ripple/hover (M4 Phase 5),
  whose state-layer math is computed directly inside `paint_node`, in
  `engine-render` itself, not delegated to `engine-md3`. Elevation
  follows the identical shape.
- **The exact MD3 elevation shadow values had to be verified against a
  real, authoritative source, not recalled or guessed.** A first web
  source gave values explicitly marked "alpha-stage ... may change
  before stable release" — rejected. Found and fetched the real,
  current, authoritative source: Material Web's own `elevation/
  internal/_elevation.scss` (the actual production CSS Google ships),
  which documents exact per-level values in its own comments and
  encodes them as a continuous piecewise-linear formula in a real
  `--_level` custom property — exactly the shape needed for
  `PaintProperties.elevation` being a real `Animated<f64>`, not a
  discrete 0-5 enum. Two stacked shadow layers confirmed: a "key" shadow
  (opacity 0.3, no spread) and an "ambient" shadow (opacity 0.15, with
  spread). Hand-verified the transcribed formula reproduces the
  documented value at all 6 integer levels for both layers before
  trusting it (worked through the arithmetic for every level, not just
  spot-checked one).
- **The CSS blur-radius → Gaussian standard-deviation conversion also
  needed real verification**: confirmed against the W3C CSS Backgrounds
  and Borders Module Level 3 spec directly — "a Gaussian blur with a
  standard deviation equal to half the blur radius." `std_dev =
  blur_px / 2.0`.
- **The shadow color doesn't need to wait for Phase 3's dynamic-color
  wiring.** MD3's `shadow` color role is computed from the neutral
  palette's own tone-0 (blackest) position — confirmed via search,
  constant black regardless of the active theme's seed color, unlike
  `on-surface` (ripple/hover's own still-open gap, deferred to Phase 3
  on purpose). Real black, scaled by each layer's own real 0.3/0.15
  opacity, is the *correct* MD3 answer today, not a placeholder.
- **Applies uniformly to every `NodeKind`** — `elevation` is a
  universal `PaintProperties` field; a shadow only needs a node's own
  bounds/corner-radius, independent of what it draws on top. Painted
  once, at the top of `paint_node`, before the node's own content —
  skipped entirely at `elevation <= 0.0` (matching level 0's own real
  all-zero values), not a degenerate zero-blur draw call.

## What happened

`engine-render/src/lib.rs`: `key_shadow_geometry`/`ambient_shadow_
geometry` (private functions transcribing Material Web's own verified
piecewise-linear formula exactly, each term commented with which
documented level value it reproduces), `blur_to_std_dev` (the real W3C
conversion), `shadow_color` (real black at a given opacity). `paint_node`
gains a shadow-painting step before its own `match`: ambient layer then
key layer (real `box-shadow` stacking order), each via `Scene::
fill_blurred_rounded_rect`, skipped entirely at `elevation <= 0.0`.

New `engine-render/tests/elevation_shadow.rs`: `elevation == 0.0` paints
no shadow at all (byte-for-byte no-op, the same standard every additive
M5-era feature has been held to — the full pre-existing pixel suite
also passed unmodified, confirming this in practice); a real,
fractional elevation paints real shadow pixels below the node — not
plain background, not the node's own opaque fill color — while a point
far from the node stays plain background. Both passed on the first run.

New `examples/elevation.py`: five real cards animating in at MD3's real
elevation levels 0-5 — `Node.animate("elevation", ...)` already
existed before this phase (the setter predates it); what's new is that
it now actually paints something.

Full `cargo test --workspace --release` clean (`engine-render` gains 2
new tests), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean. `maturin develop --release` + full
`pytest tests/` (78 passed, 1 skipped, unchanged — no `engine-py` code
touched) and all eleven examples (ten existing + new `elevation.py`)
confirmed clean.
