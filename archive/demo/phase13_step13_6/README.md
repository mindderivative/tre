# Demo: Phase 13 Step 13.6 -- Vertex Animation (`tre.Svg.morph`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_6
../../.venv/bin/python demo.py
```

**What this proves.** Q12's "vertex animations" -- turned out to be
*largely already possible* at the engine level: `tre_svg::morph`/
`morph_into` already interpolates two equal-length point lists via a
real SIMD primitive (`tre_math::lerp_points_batch`), and
`tre_engine::Svg`'s own `positions` field (added Step 12.8) is a plain,
unvalidated public field a caller can already swap frame-to-frame. This
step closes the one real gap: exposing that capability to Python, as
`tre.Svg.morph(from_, to, t) -> Svg`, interpolating two same-topology
`Svg` meshes' own already-tessellated vertex positions while keeping
`from_`'s own triangle indices unchanged (morphing changes *positions*,
never mesh connectivity).

**Not a rename of `tre_svg::morph_into`**: that function operates on
`tre_svg::Polygon`'s raw, un-triangulated contour points -- an earlier
pipeline stage than `Svg`'s own already-tessellated `positions`. `Svg.
morph` calls `tre_math::lerp_points_batch` directly instead -- the
identical real SIMD primitive `morph_into` itself uses internally, just
applied to the right data shape for this real use case.

**No new `treAnimation` class needed for this.** `Svg.morph` is pure,
stateless sampling, mirroring `Tween::sample`'s own contract exactly --
driving it frame-by-frame is just composing it with the already-shipped
`tre.Tween`/`tre.Easing` machinery from Step 13.2: `t =
progress_tween.sample(elapsed)`, then `frame_svg = tre.Svg.morph(a, b,
t)`. This demo proves that composition drives a real, monotonically
shrinking animation across five sampled frames, not just the raw
morph math in isolation.

Every assertion checks an exact or provably-real property:

- `morph(big, small, t=0.0)` renders **byte-identical** to `big` itself,
  and `t=1.0` renders byte-identical to `small` -- real endpoint
  fidelity, not an approximation.
- `morph(big, small, t=0.5)` produces a genuinely intermediate size: a
  pixel inside the original `big` square but outside the real linearly-
  interpolated midpoint footprint reads background, proving the mesh
  actually shrank, not just "some blend of colors."
- Morphing between meshes with different vertex counts (a 4-point
  square and a 3-point triangle) raises a real `ValueError` naming the
  exact mismatch -- no silent corruption.
- Driving `t` through a real `Tween(0.0, 1.0, duration, Linear)`
  produces a real, monotonically shrinking sequence of on-screen widths
  across five frames.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes.

**Real, disclosed scope limit carried forward from `tre_svg::morph`
itself**: only equal-topology morphing is supported (matching
`tre_svg::morph`'s own real "equal vertex counts, no auto-resampling"
constraint) -- morphing between visually similar but differently-
triangulated meshes is not supported without a real resampling step,
out of scope here.
