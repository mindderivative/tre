# Demo: Phase 13 Step 13.3 -- `treAnimation` (`tre.Timeline`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_3
../../.venv/bin/python demo.py
```

**What this proves.** The third section of the Phase 13 plan: a new
`tre-animation` crate providing `Timeline`, the real sequencer built on
top of `treTween`'s pure math (Step 13.2). `Timeline` itself has no
PyO3 dependency -- it identifies animation targets by an opaque `u64`,
mirroring `tre_engine::AnimationId`'s own real, already-established
pattern -- so `tre-python`'s own `PyTimeline` supplies the missing half:
mapping each `u64` to a real Python object + attribute name, applying
every sampled value back via Python's own `setattr`. This works
generically for **any** `#[pyo3(get, set)]` field on any shape class
with no per-shape dispatch code, and directly reuses this session's own
Step 12.9 `scale_x`/`scale_y`/`rotation`/`opacity` exposure as real
animation targets for the first time since they were added.

Four real properties proven, each against an exact hand-computed value:

- `timeline.animate(rect, "x", to=100.0, duration=1.0)` then
  `advance(0.25)` twice must leave `rect.x` at exactly `25.0` then
  `50.0` -- and `advance()` past the duration must hold at exactly
  `100.0` and report `still_running=False`, never extrapolating past
  the tween's own end.
- Two concurrent animations on the *same* shape (`scale_x` and
  `rotation`) advance independently and correctly in one `Timeline`.
- `prune_finished()` drops only the animation that has actually
  finished, leaving a still-running one untouched.
- **Real render integration**: an animated rectangle's `x` is advanced
  to a known midpoint, then actually inserted into a `ShapeRegistry` and
  rendered through `HeadlessRenderer` -- the resulting pixels are
  checked at both the shape's *new* position (must be filled) and its
  *original* position (must be background), proving the animation
  genuinely drives the real rendering pipeline, not just an isolated
  Python attribute.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (new `tre-animation` crate: 5
  tests covering single/concurrent/finished-entry sampling and
  `prune_finished`'s own exact removed-id reporting).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion (including the real-render integration check) passes.

**Real, disclosed scope limits**: `Timeline` only animates `f32`-typed
attributes (matching `treTween`'s own `Lerp<f32>` instantiation exposed
to Python so far) -- a `Vec2`-shaped property (e.g. animating position
as one `(x, y)` pair instead of two separate `x`/`y` timeline entries)
isn't wired into `PyTimeline::animate` yet, though `tre-tween`'s own
`Tween<glam::Vec2>` already exists and could back it. No looping/yoyo/
repeat-count exists yet (`tre_animation::Timeline` schedules each entry
once); real, straightforward future work once a caller needs it.
