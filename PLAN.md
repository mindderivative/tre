# Plan: M9 Phase 3 — Real Automatic Teardown for Container Transform (§7.6, closing its own stated gap and M9 entirely)

Corresponds to `BUILD_TRACKER.md` M9 Phase 3's own scoping: `Window.
begin_container_transform` gains an optional `on_complete` parameter,
wired onto the destination's own driven `transform` animation via the
new completion mechanism — a real app can now pass a callback that
calls `end_container_transform` and have teardown fire automatically,
closing the exact gap `container_transform.rs`'s own doc comment named
as confirmed-still-unwired.

## Investigation before writing code

- `engine_md3::container_transform::begin(tree, trigger, destination,
  config, now)` is a pure Rust function with no `pyo3` awareness (§4:
  `engine-md3` never depends on `pyo3`) — it can accept a plain
  `Option<CompletionHandle>` (already `engine-core`, already a real
  dependency of `engine-md3`) but can never itself mint one or call a
  Python callback; only `engine-py` can do either.
- `begin`'s own real body already drives all four properties
  (`transform`/`corner_radius`/`background`/`elevation`) with one
  shared `now`/`config.duration`/`config.curve` — confirmed via direct
  re-read — so attaching the one real handle to just the `transform`
  animation (matching this phase's own scoping text) is sufficient:
  all four complete on the exact same tick.
- `Node::animate`'s own new `animate_field` helper (M9 Phase 2) is
  `engine-py`-only (lives in `node.rs`, takes a `Py<PyAny>`-registering
  closure) — `container_transform::begin` needs its own, simpler,
  `engine-core`-only branch (`animate_to` vs. `animate_to_with_
  completion` based on a plain `Option<CompletionHandle>`), since it
  has no `SharedCompletions`/`Py<PyAny>` to work with at all.
- `Window.begin_container_transform` (`window.rs`) already mirrors
  `Node.animate`'s own real construction shape closely enough (both
  build a `ContainerTransformConfig`-equivalent and call into
  `engine_md3`) that registering a handle there, the same way `Node.
  animate` now does, and passing it down as a new function parameter
  is the natural, minimal extension — no new registry, no new GC
  obligation (`PyWindow.completions` already exists and is already
  traversed, M9 Phase 2).

## Design

- `engine_md3::container_transform::ContainerTransformConfig` stays
  unchanged; `begin` gains a new `on_complete: Option<CompletionHandle>`
  parameter, passed through to the `transform` animation's own
  `animate_to`/`animate_to_with_completion` choice (the other three
  properties keep plain `animate_to`, matching this phase's own stated
  "any one of them completing means the transition is genuinely done"
  reasoning — only one handle is needed, not four).
- `Window.begin_container_transform` gains `on_complete: Option<
  Py<PyAny>>` (`#[pyo3(signature = (trigger, destination, duration_ms=
  300, content_stagger_ms=90, on_complete=None))]`), registers it into
  `self.completions` the same way `Node.animate` now does, and passes
  the resulting handle into `engine_md3::begin_container_transform`.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New `engine-md3` test:
  `begin` with a real `Some(handle)` attaches it to the destination's
  own `transform` animation (`ActiveAnimation.on_complete`), and
  ticking to completion reports it via `Tree::tick_all`'s own real
  drain — proving the wiring reaches all the way through, not just that
  the parameter compiles; `begin` with `None` behaves byte-for-byte as
  before this phase (no completion ever reported), a true regression
  check.
- `maturin develop --release` + `pytest tests/` + all examples. New
  pytest test: `Window.begin_container_transform(..., on_complete=
  callback)` doesn't raise (the same FFI-smoke-test scope M9 Phase 2's
  own tests used, for the same real reason — `App.run()` needs a real
  display to prove live firing). Updated `examples/
  container_transform.py`: passes a real `on_complete` that calls
  `end_container_transform` automatically, replacing the manual call
  at the end of the script — the real, live, closing proof this
  milestone's own investigation set out to enable.
