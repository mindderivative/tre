# Plan: M9 Phase 2 — Python-Facing `on_complete` Callbacks

Corresponds to `BUILD_TRACKER.md` M9 Phase 2's own scoping: `Node.
animate(property, to, duration_ms, on_complete=None)` mints a real
`CompletionHandle` and stores the Python callback keyed by it; `App::
run`'s own per-frame loop drains `Tree::tick_all`'s new completions and
invokes each matching stored Python callback exactly once.

## Investigation before writing code

- `dispatch.rs`'s own `HandlerMap`/`run_dispatch_outcome`/
  `call_handler` (M4 Phase 1 step 3, generalized M4 Phase 6) is the
  exact real template to mirror: a shared `Rc<RefCell<HashMap<...,
  Py<PyAny>>>>`, cloned into every `Node`/`PyWindow`/`View` the same
  way, plus a "look up, clone out, drop the borrow, *then* call" helper
  (avoids a re-entrant `RefCell` borrow panic if the callback itself
  registers a new one — a real, plausible pattern this codebase already
  guards against for click/hover handlers). Completion callbacks follow
  the identical shape, with one real difference: they're one-shot
  (`HashMap::remove`, not `get`) — matching a real CSS `transitionend`/
  JS Promise-style single fire, not a repeating subscription.
- `Node`'s own `theme: SharedTheme` field (M7 Phase 3) is the most
  recent real precedent for "a new shared registry threaded through
  every `Node` construction site" — `window.rs` (4 sites) shares the
  real one; `view.rs` (3 sites) gets a fresh, private instance each,
  since `View` has no real per-frame render loop to drain completions
  through (confirmed via its own module doc comment: "never embedded
  into a live `winit`-driven window") — a completion registered on a
  `View`-created node would sit unreachable forever, the same real,
  stated scope limit `theme` already accepted for `View`.
- `App::run`'s own `on_frame` closure (`app.rs`) already captures `py:
  Python<'_>` from the enclosing `run(&self, py: Python<'_>, ...)`
  method (confirmed by reading the `on_input` closure's own body,
  which already uses `py` the same way) — no new parameter threading
  needed to reach it from `on_frame`.
- `Node::animate`'s own real dispatch (`node.rs`) is six separate match
  arms, each calling a *different* `Animated<T>` field's own
  `.animate_to(...)` (`T` differs per arm: `f64`, `Color`, `Affine`,
  `ShapeKey`) — no existing shared helper unifies them. A small,
  private, generic `animate_field<T: Interpolate + Clone>(field: &mut
  Animated<T>, ..., handle: Option<CompletionHandle>)` (choosing
  `animate_to_with_completion` vs. plain `animate_to` based on
  `handle`) is directly reusable across all six arms, avoiding
  per-arm duplication of that branch.
- Registering the `CompletionHandle` *inside* whichever single arm
  actually matches (not once up front, before knowing `property` is
  valid) avoids leaking an orphaned, never-invoked callback into the
  registry when a caller passes an unknown property name (the existing
  `UnknownProperty` error path) — `on_complete: Option<Py<PyAny>>` is
  moved into exactly one arm at match time, which Rust allows.

## Design

- New `dispatch::CompletionRegistry { next_id: u64, callbacks: HashMap<
  CompletionHandle, Py<PyAny>> }` with `register(&mut self, callback:
  Py<PyAny>) -> CompletionHandle` (mints a fresh, monotonically
  increasing handle); `type SharedCompletions = Rc<RefCell<
  CompletionRegistry>>`.
- New `dispatch::run_completions(completions: &SharedCompletions,
  completed: Vec<CompletionHandle>, py: Python<'_>)` — for each real
  handle, removes (not just looks up) its callback and calls it,
  printing (not raising) any real Python exception the same way
  `call_handler` already does.
- `PyWindow` gains `completions: SharedCompletions`, initialized fresh
  in `::new`. `Node` gains the same field, threaded through all 7
  construction sites (`window.rs` shares `self.completions.clone()`;
  `view.rs` gets a fresh, private instance each).
- `Node::animate` gains `on_complete: Option<Py<PyAny>>` (`#[pyo3(
  signature = (property, to, duration_ms=0, on_complete=None))]`). A
  new private `animate_field` helper (generic over `T: Interpolate +
  Clone`) replaces each arm's own direct `.animate_to(...)` call,
  registering a real handle (moving `on_complete` into whichever one
  arm matches) only when one was actually given.
- `App::run`'s `WindowSetup`/`WindowRuntime` gain `completions:
  SharedCompletions`, extracted from `window.completions.clone()` the
  same way `theme`/`dock`/`context_menus` already are. The `on_frame`
  closure's own `tick_all` call is updated to bind the new `Vec<
  CompletionHandle>` return and pass it to the new `run_completions`.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — this phase is
  entirely `engine-py`, no `engine-core`/`engine-render` change, so the
  full Rust suite is a pure regression check (must stay exactly as it
  was after Phase 1).
- `maturin develop --release` + `pytest tests/` + all examples. New
  pytest tests: a zero-duration `on_complete` fires exactly once, with
  no arguments, on the very next `tick_all`; a still-running animation
  never fires its callback early; an animation with no `on_complete`
  behaves exactly as before this phase (a true no-op regression check);
  a `View`-created node's `on_complete` (no live render loop to drain
  it through) never fires — the real, stated scope limit, proven, not
  just claimed. New `examples/animation_completion.py`: a real
  `Window`-driven animation whose `on_complete` callback prints a
  message once, proving the whole call chain end to end.
