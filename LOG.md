# Log: M9 Phase 2 — Python-Facing `on_complete` Callbacks

Corresponds to `BUILD_TRACKER.md` M9 Phase 2. `Node.animate(property,
to, duration_ms, on_complete=None)` mints a real `CompletionHandle` and
stores the Python callback keyed by it; `App::run`'s own per-frame loop
drains `Tree::tick_all`'s new completions and invokes each matching
stored Python callback exactly once.

## Investigation before writing code

- `dispatch.rs`'s own `HandlerMap`/`run_dispatch_outcome`/
  `call_handler` (M4 Phase 1 step 3/M4 Phase 6) is the exact real
  template mirrored: a shared `Rc<RefCell<HashMap<..., Py<PyAny>>>>`,
  cloned into every `Node`/`PyWindow`/`View`, plus a "look up, clone
  out, drop the borrow, *then* call" helper (avoids a re-entrant
  `RefCell` borrow panic if a callback itself registers a new one).
  Completion callbacks differ in one real way: one-shot (`HashMap::
  remove`, not `get`) — a real CSS `transitionend`/JS Promise-style
  single fire, not a repeating subscription.
- `Node.theme: SharedTheme` (M7 Phase 3) is the most recent real
  precedent for "a new shared registry threaded through every `Node`
  construction site" — `window.rs` (4 sites) shares the real one;
  `view.rs` (3 sites) gets a fresh, private instance each, since `View`
  has no real per-frame render loop to ever drain a completion through
  (confirmed via its own module doc comment) — the same real, stated
  scope limit `theme` already accepted for `View`.
- `App::run`'s own `on_frame` closure already captures `py: Python<'_>`
  from the enclosing `run(&self, py: Python<'_>, ...)` method (confirmed
  by reading the `on_input` closure's own body) — no new parameter
  threading needed.
- `Node::animate`'s own real dispatch is six separate match arms, each
  calling a *different* `Animated<T>` field's own `.animate_to(...)`
  (`T` differs per arm). A small, private, generic `animate_field<T:
  Interpolate + Clone>` helper replaces each arm's own direct call,
  choosing `animate_to_with_completion` vs. plain `animate_to` based on
  whether a handle was registered — avoiding six copies of that branch.
- Registering the `CompletionHandle` inside whichever single arm
  actually matches (not once up front) avoids leaking an orphaned,
  never-invoked callback into the registry on an unknown-property error
  — `on_complete: Option<Py<PyAny>>` is moved into exactly one arm at
  match time, which Rust allows.
- `completions` holds real `Py<PyAny>` values — the same cyclic-GC
  obligation `handlers` already has. `PyWindow`'s own `__traverse__`/
  `__clear__` cover it (the same underlying shared `Rc<RefCell<...>>`
  `Node` also holds a clone of); `Node` itself needs no separate
  `__traverse__`, matching `handlers`' own existing precedent.
- **Real constraint confirmed while planning verification:** `App.run()`
  blocks and opens a real window/display — `test_engine_py.py`'s own
  module doc comment already states it's deliberately never exercised
  in pytest, only in real examples. This meant the pytest-level
  coverage for `on_complete` stays an FFI smoke test (accepted without
  raising) plus a real cyclic-GC regression test; the real, live,
  end-to-end "does it actually fire" proof is the new example.

## What happened

New `dispatch::CompletionRegistry { next_id: u64, callbacks: HashMap<
CompletionHandle, Py<PyAny>> }` with `register(&mut self, callback) ->
CompletionHandle`; `type SharedCompletions = Rc<RefCell<
CompletionRegistry>>`. New `dispatch::run_completions(completions,
completed, py)` — removes and calls each real callback, printing (not
raising) any real Python exception the same way `call_handler` already
does.

`PyWindow` gains `completions: SharedCompletions` (fresh in `::new`),
covered by its own `__traverse__`/`__clear__`. `Node` gains the same
field, threaded through all 7 construction sites. `Node::animate` gains
`on_complete: Option<Py<PyAny>>`; a new private `animate_field` helper
replaces each of the six arms' own direct `.animate_to` call.
`App::run`'s `WindowSetup`/`WindowRuntime` gain `completions:
SharedCompletions`; the `on_frame` closure's `tick_all` call now binds
the real completion `Vec` and passes it to `run_completions`.

New tests (`test_engine_py.py`): `on_complete` accepted without raising;
omitting it still works (explicit regression); a real cyclic-GC test
(mirroring `test_virtual_list.py`'s own analogous `materializers` test)
proves a `Window`-capturing `on_complete` callback is genuinely
collected once unreachable, not leaked forever. New `examples/
animation_completion.py`: a real `App.run()`-driven animation whose
`on_complete` fires exactly once — found and fixed a real timing issue
while writing it (a 100ms real-duration animation never completed
within a reasonable `max_frames` budget in this headless environment's
own unpredictable software-rendered frame rate; switched to
`duration_ms=0`, which `Animated::tick`'s own real "elapsed >= duration"
contract snaps immediately on the first tick, making completion
deterministic regardless of real wall-clock frame pacing).

Full `cargo test --workspace --release` clean (no `engine-core`/
`engine-render` change this phase, a pure regression check — unchanged
from Phase 1's own counts), `cargo clippy --workspace --all-targets --
-D warnings`, `cargo fmt --check` all clean. `maturin develop --release`
+ full `pytest tests/` (82 passed, up from 79, 1 skipped) and all
sixteen examples (fifteen existing + new `animation_completion.py`)
confirmed clean.
