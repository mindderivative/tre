# Demo: Phase 13 Step 13.1 -- `treTime` (`tre.Clock`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_1
../../.venv/bin/python demo.py
```

**What this proves.** The first section of the Phase 13 plan (timing,
tweening, animation, math, shader, shadow, editable-text systems):
`tre.Clock`, a thin Python wrapper over `tre_engine::FrameClock` -- the
engine's own real, `std::time::Instant`-backed monotonic frame timer,
already used natively in `main_loop_demo.rs` but never exposed to
Python.

Deliberately not a new timing implementation: `FrameClock::tick()`
already correctly returns `0.0` on its first call (no prior frame to
measure a delta from) and a real delta afterward. This step adds exactly
one new capability to the engine itself -- `FrameClock::elapsed()`,
total real seconds since construction, independent of `tick()`'s own
per-frame cadence -- because a future `treTween`/`treAnimation` caller
needs a real "t" to sample a tween or timeline against, not just a
per-frame delta.

Both properties are proven against real `time.sleep` durations, not
just "it returns a float": `tick()` reports a real ~50ms delta after a
real 50ms sleep; `elapsed()` keeps growing across two real sleeps
totalling ~80ms even while an immediately-repeated `tick()` reports a
near-zero delta -- proving `elapsed()` genuinely tracks time since
construction, not time since the last `tick()`.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-engine` 157 -> 158
  tests: a new `frame_clock_elapsed_accumulates_independently_of_tick`
  test, matching the existing `frame_clock_reports_real_elapsed_time_
  between_ticks` test's own real-sleep-based verification style).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes.

**Not yet done:** `treTween` (Phase 13 Step 13.2) and `treAnimation`
(Step 13.3), both of which will consume this `Clock` to drive real
tween/timeline sampling -- this step is the timing foundation only, per
the approved Phase 13 plan's own dependency ordering
(`treTime` -> `treTween` -> `treAnimation`).
