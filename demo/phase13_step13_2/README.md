# Demo: Phase 13 Step 13.2 -- `treTween` (`tre.Tween`/`tre.Easing`/`tre.Spring`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_2
../../.venv/bin/python demo.py
```

**What this proves.** The second section of the Phase 13 plan: a new
`tre-tween` crate -- the pure, stateless interpolation math layer
`tre-animation` (Step 13.3) will sequence on top of -- plus its
`tre-python` binding.

- **`tre.Easing`**: the standard Robert Penner curve set (linear,
  quad/cubic/quart/quint in/out/in-out, 13 named curves total), each a
  pure `f32 -> f32` function.
- **`tre.Tween(from_, to, duration, easing=...)`**: generic in Rust over
  anything `Lerp` (`f32`, `glam::Vec2` today); `tre.Tween` accepts
  either a plain number or a real `(x, y)` tuple for `from_`/`to`,
  dispatching to the matching Rust generic instantiation and rejecting a
  shape mismatch with a real `TypeError`. `.sample(elapsed)` clamps
  `elapsed` into `[0, duration]` before applying the easing curve, so a
  caller can pass any real elapsed time -- including past the tween's
  own end -- and always gets a well-defined result (`from_` before the
  start, `to` after the end, never extrapolated).
- **`tre.Spring(stiffness, damping, mass, initial_position=0.0)`**: a
  **real** damped mass-spring-damper integrator (semi-implicit Euler),
  genuinely distinct from `tre_math::spring_decay`'s plain exponential
  smoothing -- `spring_decay`'s own doc comment says it can never
  overshoot; this `Spring` can and does, when underdamped, and settles
  without overshoot when heavily overdamped, matching real spring
  physics at both ends of the parameter space.

Every assertion checks an exact hand-computed value, not just "it
runs": `EaseInQuad` at progress `0.5` of a `0..10` tween must give
exactly `2.5` (`0.5² × 10`); a `(0,0)..(10,20)` `Vec2` tween at the
halfway point of its duration must give exactly `(5, 10)`; an
underdamped `Spring(200, 5, 1)` chasing target `1.0` must overshoot past
`1.05`, while a heavily overdamped `Spring(50, 200, 1)` must not.

**Real math library adoption (Q9)**: `tre-tween` depends on `glam`
(0.29) -- the de facto standard, SIMD-backed Rust math crate for
real-time graphics (Bevy/wgpu ecosystem) -- for its `Vec2` lerp.
Deliberately additive: `tre-math`'s own `Affine2`/SIMD-batch functions,
already proven in `tre-engine`'s hot flatten path, are untouched.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (new `tre-tween` crate: 12
  tests covering every easing curve's `0->0`/`1->1` boundary, tween
  clamping, and both spring regimes).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes.

**Real, disclosed scope limits**: only `f32` and `glam::Vec2` are
exposed as `Lerp` instantiations to Python today (`Vec3`/`Quat`/`Color`
are real, straightforward additions once a real 3D or color-tween
consumer exists -- not built speculatively). `Tween`/`Spring` are pure
math with no timeline/sequencing concept -- that's `tre-animation`'s
job (Step 13.3), not yet started.
