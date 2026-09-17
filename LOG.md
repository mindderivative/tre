# Log: M9 Phase 3 — Real Automatic Teardown for Container Transform (§7.6), closing M9 entirely

Corresponds to `BUILD_TRACKER.md` M9 Phase 3, the milestone's own final
phase. `Window.begin_container_transform` gains an optional
`on_complete` parameter, wired onto the destination's own driven
`transform` animation via the new completion mechanism — a real app can
now pass a callback that calls `end_container_transform` and have
teardown fire automatically, closing the exact gap `container_
transform.rs`'s own doc comment named as confirmed-still-unwired.

## Investigation before writing code

- `engine_md3::container_transform::begin` is pure Rust with no `pyo3`
  awareness (§4) — it can accept a plain `Option<CompletionHandle>`
  (already a real dependency of `engine-md3` via `engine-core`) but can
  never itself mint one or call a Python callback; only `engine-py` can
  do either.
- `begin`'s own real body already drives all four properties with one
  shared `now`/`config.duration`/`config.curve` (confirmed via direct
  re-read) — attaching the one real handle to just the `transform`
  animation is sufficient, since all four complete on the exact same
  tick.
- `Node::animate`'s own `animate_field` helper (M9 Phase 2) is
  `engine-py`-only — `container_transform::begin` needed its own,
  simpler, `engine-core`-only branch (`animate_to` vs. `animate_to_
  with_completion` based on a plain `Option<CompletionHandle>`).

## What happened

`container_transform::begin` gains `on_complete: Option<CompletionHandle
>`, wired onto the destination's own `transform` animation only.
`Window.begin_container_transform` gains `on_complete: Option<
Py<PyAny>>`, registered into `self.completions` the same way `Node.
animate` already does, and passed down as the new handle.

Two stale doc comments corrected along the way (this project's own
established discipline): `container_transform.rs`'s own module doc
comment, which explicitly named this exact gap as "confirmed-still-
unwired... separate, larger, unscoped work," now states it's real;
`CompletionHandle`'s own doc comment in `animation.rs`, which said "no
allocator/registry exists yet," now names the real one (M9 Phase 2's
`CompletionRegistry`).

New `engine-md3` tests: a real `on_complete` handle given to `begin`
genuinely reaches `Tree::tick_all`'s own real drain once the transition
finishes, not before; `on_complete: None` never reports a completion —
the real regression counterpart. New pytest test: `Window.
begin_container_transform(..., on_complete=callback)` accepted without
raising (the same FFI-smoke-test scope M9 Phase 2's own tests used —
`App.run()` needs a real display to prove live firing). Updated
`examples/container_transform.py`: `on_complete` now calls `end_
container_transform` automatically, replacing the manual call at the
end of the script — the real, live, closing proof this milestone's own
investigation set out to enable.

Full `cargo test --workspace --release` clean (`engine-md3` gains 2
tests: 7 → 9 — every prior test passed unmodified), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean. `maturin develop --release` + full `pytest tests/` (83 passed,
up from 82, 1 skipped) and all sixteen examples confirmed clean.

M9 — Animation Completion Callbacks is now complete: all 3 phases done.
`CompletionHandle`/`ActiveAnimation.on_complete` were real, exported,
genuinely unused types before this milestone — now wired end to end,
from `Animated::tick`'s own real per-tick detection through `Tree::
tick_all`'s real drain, `engine-py`'s own Python-facing registry, and
into container transform's own real automatic teardown.
