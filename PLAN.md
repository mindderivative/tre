# Plan: M4 Phase 2 — Assistive-Technology Action Dispatch (§10, §4)

Phase 1 (all 3 steps, committed `c761d1c`/`1d6086b`) built real pointer/
keyboard dispatch end to end: `winit` events reach `Tree::dispatch`, and
a real click calls a real registered Python callback. Two real gaps
were named explicitly in Phase 1's own `LOG.md`/`BUILD_TRACKER.md`
entries and left open:

1. `accesskit`'s `ActionRequested`/`AccessibilityDeactivated` events are
   still received and ignored in `engine-platform`'s `user_event`
   handler — a real, live screen reader (Orca on Linux, NVDA on
   Windows, VoiceOver on macOS) literally cannot activate a button
   through AT-SPI/UIA/NSAccessibility today, despite M3 step 7's real
   accesskit wiring existing since early in this project. §10's own
   text is explicit about this: "platform-driven focus requests (a
   screen reader focusing a node directly) dispatch `accesskit::
   Action::Focus`... both routed through the same `AppHandler`/
   `InputEvent` inversion already wired for pointer events."
2. Keyboard-triggered activation (Tab then Enter/Space) has no
   Python-facing entry point to test from — `Window` only exposes
   `click()` (pointer press+release).

Both are squarely within M4's own named scope (§4, §10) and don't
depend on anything unbuilt elsewhere (§11.9 transform composition,
§11.8 virtualization viewport, `NodeKind::Canvas` are each real, but
belong to different, later milestones — not started here).

## Step 1 — `Tree` gains direct activation/focus primitives (`engine-core`)

**Real API fact, verified directly against the pinned `accesskit =
"0.25.0"`'s own source before writing anything:** `ActionRequest {
action: Action, target_tree: TreeId, target_node: accesskit::NodeId,
data: Option<ActionData> }`. `target_node` is an opaque `accesskit::
NodeId` — converting it back to `engine_core::NodeId` needs the
reverse of `to_access_id`'s existing `id.data().as_ffi()` encoding.
Verified directly in `slotmap = "1.1.1"`'s own source: `KeyData::
as_ffi`/`from_ffi` are a documented, guaranteed-reversible round trip
("passing it to `from_ffi` will return a key equal to the original"),
and `new_key_type!` generates `impl From<KeyData> for NodeId` — so
`from_access_id` is a real, safe conversion, not a guess. A stale/
foreign id round-trips to some `NodeId` value that simply fails `Tree`'s
own generation check (returns `None`/behaves as "not found") rather
than resolving to the wrong node — the same generational-safety
property this whole project already relies on elsewhere (§5).

`to_access_id`/a new `from_access_id` become `pub` (both were private
free functions in `tree.rs`; `from_access_id` is new).

**`Tree::activate(node) -> DispatchOutcome`**: the direct, non-`InputEvent`
counterpart to a mouse click's own `Activated` outcome — validates
`node` exists in this `Tree`, returns `Activated(node)` if so, `None`
otherwise. Deliberately does **not** check `access.actions` first — the
real mouse-click path (`dispatch`'s `PointerPressed`/`PointerReleased`
handling) doesn't gate on it either (hit-testing alone decides *which*
node; whether anything is actually registered to react is `engine-py`'s
`click_handlers` lookup's own job) — keeping both activation paths
symmetric, not inventing a stricter rule for one than the other.

**`Tree::set_focus_to(node, focus_ring_opacity, duration, now)`**: the
direct-target counterpart to `move_focus`'s own tab-order computation —
factors the existing "animate the old node's `focus_ring` out, the new
one's in, opt-in-only per Design Principle 6" logic out of `move_focus`
into a small private helper, reused by both. This is what `Action::
Focus` (§10: "platform-driven focus requests... dispatch `accesskit::
Action::Focus`") actually needs: jump straight to a specific node, not
compute a tab-order neighbor.

## Step 2 — Real `accesskit_winit` wiring (`engine-platform`)

`run_windowed_multi` gains a sixth parameter, `on_access_action:
FnMut(WindowId, accesskit::ActionRequest)` — matching `on_input`'s own
"translate the platform event, hand the raw data up, never touch a
`Tree`" shape exactly (`engine-platform` doesn't know `engine_core::
NodeId` exists; the conversion is the caller's job, since the caller
already depends on `engine-core` for everything else). Fires on
`accesskit_winit::WindowEvent::ActionRequested(request)`, replacing the
comment that's named this exact gap since step 7. `run_windowed`'s
wrapper passes a no-op, matching every other closure it doesn't need.

**Real, checked-before-assuming constraint, matching step 2's own
precedent:** no live AT-SPI/UIA/NSAccessibility client is available in
this dev/CI environment to drive a genuine end-to-end screen-reader
proof (unlike M3 step 7's original wiring, which *was* checked against
a real, interactively-running AT-SPI bus at the time) — a synthetic
`accesskit::ActionRequest` value is plain data, so the real, honest
proof here is at the translation/dispatch-logic level: construct one
directly and verify `from_access_id` round-trips correctly and
`Tree::activate`/`set_focus_to` produce the right outcome, the same
"validate the pure logic standalone, state what a live integration
would still need" pattern already used for `translate_pointer_button`/
`translate_key`.

## Step 3 — `engine-py` wires the new callback + a keyboard test entry point

`App::run()` gets a new `on_access_action` closure: converts
`request.target_node` via `engine_core::from_access_id`, matches
`request.action` (`Action::Click` → `tree.activate(node)`, `Action::
Focus` → `tree.set_focus_to(node, ...)` via `dispatch::
interaction_config()`'s existing values), and feeds the outcome through
the *same* `dispatch::run_activation` already shared between `on_input`
and `Window.click()` — one more real call site, still one copy of the
click-handling logic.

`Window.press_key(key: str, shift: bool = False)` (new, mirrors
`click()`'s own shape): translates a small Python string vocabulary
(`"tab"`/`"enter"`/`"space"`/`"escape"`) into `engine_core::Key`,
dispatches a `KeyPressed` event through the same `interaction_config()`/
`run_activation` path. Closes the stated gap: Tab-then-Enter activation
is now directly testable from Python with no live window.

## Verification

`engine-core`: new unit tests for `from_access_id`/`to_access_id`
round-tripping (including a stale-id-fails-safely case), `Tree::
activate` (valid node → `Activated`, unknown `NodeId` → `None`),
`Tree::set_focus_to` (focus moves to the exact target, `focus_ring`
transitions the same opt-in-only way `move_focus`'s own test already
proved). `engine-platform`: no new unit tests needed beyond wiring
(the translation direction is trivial data plumbing, unlike `on_input`'s
own real `WindowEvent → InputEvent` logic) — covered by the workspace
still compiling with the new required parameter threaded through every
call site. `engine-py`: new pytest coverage for `press_key` (Tab moves
focus among interactive nodes, then `Enter`/`Space` activates the
focused one) and for the `on_access_action` path (a synthetic
`accesskit::ActionRequest`-shaped call reaching a registered
`click_handler` — exercised at whatever level Step 2's own real
constraint leaves testable).

`engine-py`'s `on_access_action` closure body itself is *not* separately
pytest-covered beyond compiling and type-checking correctly: it's a
thin conversion (`from_access_id`) plus a match onto the same two
`Tree` methods (`activate`/`set_focus_to`) already fully unit-tested at
the `engine-core` level, and the same `dispatch::run_activation` already
covered by `test_click_dispatch.py`. Stated plainly rather than
manufacturing a test around a synthetic `accesskit::ActionRequest` value
that would only re-prove what the `engine-core` tests already prove —
a genuine end-to-end proof needs a live AT-SPI/UIA/NSAccessibility
client, which this environment doesn't have (see Step 2's own
real-constraint note).

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo fmt --check`, `maturin develop` + the full pytest
suite, plus both real windowed tests and both example scripts
unchanged.
