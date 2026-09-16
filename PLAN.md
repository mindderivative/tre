# Plan: M4 Phase 1, Steps 2-3 — Wire Real Input to `winit`/`engine-py`

Step 1 (committed, `c761d1c`) built the core mechanism entirely inside
`engine-core`: `InputEvent`/`AppHandler`/`DispatchOutcome` (`input.rs`)
and `Tree::hit_test`/`update_hover`/`move_focus`/`dispatch` (`tree.rs`),
proven by 32 unit tests with no `winit`/`pyo3` involved at all. This
plan covers the two steps that make it reachable from a real window and
a real Python app: translating actual `winit` events into `InputEvent`
(`engine-platform`), then acting on the result (`engine-py`).

## Step 2 — Real `winit` event translation (`engine-platform`)

**Real APIs verified directly first, not assumed** (`winit = "0.30.13"`'s
own source, `crates.io` checkout):
- `WindowEvent::CursorMoved { position: PhysicalPosition<f64>, .. }`,
  `MouseInput { state: ElementState, button: MouseButton, .. }`,
  `KeyboardInput { event: KeyEvent, .. }`, `ModifiersChanged(Modifiers)`.
- `MouseButton` has **six** real variants (`Left`/`Right`/`Middle`/
  `Back`/`Forward`/`Other(u16)`) — `engine_core::PointerButton`'s doc
  comment previously (wrongly, unverified) claimed a 1:1 three-variant
  match; corrected in this step. `Back`/`Forward`/`Other` translate to
  no `InputEvent` at all (a browser-navigation convention with no MD3
  desktop meaning yet) — narrowed, not silently mishandled.
- `KeyEvent.logical_key: winit::keyboard::Key`, matched against
  `Key::Named(NamedKey::{Tab,Enter,Space,Escape})` — every other key
  produces no `InputEvent` (§10's own stated minimal vocabulary).
- Shift state isn't carried on `KeyEvent` itself — `ModifiersChanged`
  is a separate event. `PerWindow` gains a `modifiers: ModifiersState`
  field, updated on `ModifiersChanged`, read (via `.shift_key()`) when
  translating a `KeyboardInput`.

**`run_windowed_multi` gains a new `on_input: FnMut(WindowId,
engine_core::InputEvent)` closure parameter**, matching `on_frame`/
`build_access_update`'s own existing shape exactly — `engine-platform`
translates the raw event and hands it up; it never touches a `Tree`
itself (it doesn't have one — generic over whatever the caller does
with the translated event, the same real inversion `on_frame` already
uses for rendering). `run_windowed` (the byte-compatible single-window
wrapper, kept for `rect_window.rs`/`access_button.rs`) passes a no-op
`|_, _| {}` — neither existing caller needs input events, and this
keeps `run_windowed`'s own signature untouched, matching its own doc
comment's stated contract.

**Real proof**: a new `harness = false` test (matching `multi_window.rs`'s
own precedent — a real `winit::EventLoop` needs a real process main
thread) that opens a window and asserts genuine `InputEvent`s arrive
for synthetic... **actually verified against a real constraint first**:
`winit` doesn't expose a portable way to *inject* synthetic OS-level
input events into its own event loop from test code (there is no
`window.send_event(WindowEvent::CursorMoved{..})`-shaped public API) —
checked directly before assuming this was testable the same way
`multi_window.rs` tests window lifecycle. The real, honest proof for
this step is therefore at the translation-function level: extract the
`WindowEvent -> InputEvent` mapping into standalone, `winit`-typed-but-
window-independent functions (`translate_pointer_button`,
`translate_key`) and unit-test *those* directly against real `winit`
event/key values — proving the translation logic is correct without
needing a live event loop to feed it, the same "validate the core
logic standalone first" discipline Design Principle 5 already uses
everywhere else in this codebase. The full "does it reach a live
window's callback" wiring is exercised for real in Step 3's own
end-to-end proof once `engine-py`'s `App.run()` has something
meaningful to do with an activation.

## Step 3 — `engine-py` implements the meaning-dependent half

`Node.set_on_click(callback)` finally exists — `node.rs`'s own forward-
reference comment ("needs `PyWindow`'s cyclic-GC participation... until
something actually stores one") is resolved: `PyWindow` already gained
real `__traverse__`/`__clear__` at M3 step 15 Stage C for its
`materializers` map. This step adds a second `HashMap<NodeId, Py<PyAny>>`
field, `click_handlers`, visited by the same `__traverse__` alongside
`materializers` (one class, two independently-populated callback maps,
both real GC roots).

`App::run()`'s existing `on_input` closure (new, mirroring its own
`on_frame`/`build_access_update` closures already there) looks up the
window's `WindowRuntime`, calls `tree.borrow_mut().dispatch(root,
event, &config, now)`, and on `DispatchOutcome::Activated(node)`, looks
up `click_handlers.get(&node)` on that window's `PyWindow` and calls it
via `Python::attach`/`call0` if present — matching §9's own stated
policy ("unhandled exceptions from a callback are caught, logged, and
non-fatal") rather than propagating a panic into the render loop.
`InteractionConfig`'s real MD3 numeric values (hover/focus-ring
opacity, ripple radius/opacity/duration) are hardcoded as named
constants in `engine-py` for now — `engine-md3` doesn't yet expose
named interaction presets the way it does `motion::STANDARD`; revisit
when it does, not manufactured ahead of that need.

**Real proof**: a pytest exercising the full path without a live
window — build a `Window`, `add_rect`, `set_on_click`, then call the
new `Node`-level test-only entry point (`Tree::dispatch` reachable
through a thin, real, non-mocked wrapper) with a synthetic
`InputEvent::PointerPressed`/`PointerReleased` pair over the rect's own
known bounds, and assert the registered Python callback actually ran
(a mutated list/counter the callback closes over) — the same "prove the
mechanism with a direct call, defer only the OS event source" pattern
this whole project has used at every prior interaction step. A GC-cycle
test mirroring M3 step 15 Stage C's own (`test_window_participates_in_
cyclic_gc...`) proves `click_handlers` is visited by `__traverse__` too.

## Verification

`cargo build --workspace` (the new `on_input` parameter is a real,
compiling signature change reaching every existing caller);
`translate_pointer_button`/`translate_key` unit tests in
`engine-platform`; `cargo test --workspace`; `cargo clippy --workspace
--all-targets -- -D warnings`; `cargo fmt --check`; `maturin develop` +
the new pytest coverage (click dispatch + GC cycle), plus the full
existing pytest suite still green.
