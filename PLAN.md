# Plan: M25 Phase 1 — Real Animation Completion Callback Firing (§5, §9)

Corresponds to `BUILD_TRACKER.md` M25 Phase 1: diagnose and fix why a
real animation's `on_complete` callback never fires in this dev
environment, despite `engine-core`'s own real, unit-tested completion
mechanism.

## Investigation before writing code

- Confirmed real and reproducible: `examples/animation_completion.py`
  and `examples/container_transform.py` both assert their own
  `on_complete` callback fires exactly once during a bounded
  `App.run(max_frames=60)` and both get zero calls instead.
- Confirmed via a real git worktree at the M21-closing commit
  (`bf908c5`, before any M22/M23/M24 work) that this fails identically
  on that pre-existing baseline — not a regression from anything built
  this session.
- `engine-core::animation.rs`'s own unit tests for zero-duration
  completion (the exact real semantics `animation_completion.py`
  relies on: `duration_ms=0` means `elapsed >= duration` is true from
  the very first tick) reportedly pass — so the real bug is most
  likely in `engine-py::App::run`'s own per-frame wiring between
  `Tree::tick_all`'s completion drain and the registered Python
  callback, not in `engine-core`'s own `Animated<T>`/`CompletionHandle`
  mechanism itself. This phase's own job is to confirm that
  hypothesis directly (read the real code, don't assume) and find the
  real, exact break point.
- To investigate: read `engine-py::app.rs`'s real per-frame loop (the
  same `App::run` closure already read/modified for M22's own
  `sync_image_textures` wiring) end to end — `Tree::tick_all` call,
  what it returns, `run_completions`'s own real implementation, and
  how a `Node.animate(..., on_complete=...)` call actually registers a
  callback in the first place (`CompletionRegistry`, `dispatch.rs`).
  Also check whether this is genuinely environment-specific (e.g. a
  headless/no-real-display condition causing frames or ticks to be
  skipped) or a real, universal logic bug — re-run the two failing
  examples with tracing/`RUST_LOG` enabled if needed to see whether
  `tick_all` and the completion drain are even being reached at all.

## What will change

TBD — depends on where the real investigation finds the actual break
point. Likely one of: `App::run`'s own loop not calling
`run_completions` with the real drained `Vec<CompletionHandle>`;
`Node.animate`'s own Python-facing registration not actually storing
the callback where `run_completions` looks for it; or `Tree::tick_all`
itself not returning a genuinely non-empty completion list under
real, live frame timing (as opposed to the direct, single-call unit
tests in `animation.rs`).

## Testing

- Reproduce the failure first, confirm the fix resolves it via the
  real example scripts (not just a new unit test in isolation, since
  the bug is specifically about the real, live `App.run` wiring).
- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run every example script, confirming `animation_completion.py` and
  `container_transform.py` now both pass.
