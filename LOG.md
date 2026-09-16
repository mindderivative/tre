# Log: M4 Phase 2 — Assistive-Technology Action Dispatch (§10, §4)

Corresponds to `PLAN.md`/`BUILD_TRACKER.md` M4 Phase 2. Phase 1 (all 3 steps, `c761d1c`/`1d6086b`) built real pointer/keyboard dispatch end to end. Two real gaps were named explicitly in Phase 1's own docs and left open: `accesskit`'s `ActionRequested` events were received and ignored (a real screen reader literally couldn't activate a button through AT-SPI/UIA/NSAccessibility, despite M3 step 7's wiring existing since early in this project), and keyboard-triggered activation had no Python-facing entry point to test from. Both close here.

## What happened

**`engine-core`**: `to_access_id`/a new `from_access_id` become `pub`. The real API fact this whole phase is built on, verified directly before writing anything: `accesskit::ActionRequest.target_node` is an opaque `accesskit::NodeId` wrapping a `u64`, and `slotmap::KeyData::as_ffi`/`from_ffi` (already what `to_access_id` used one direction of) are a documented, guaranteed-reversible round trip — "passing it to `from_ffi` will return a key equal to the original." A stale or foreign id round-trips to *some* `NodeId` that simply fails this `Tree`'s own generation check later, not undefined behavior — the same generational-safety property §5 already relies on everywhere else, now extended to accessibility-client-supplied ids.

`Tree::activate(node) -> DispatchOutcome` is the direct, non-`InputEvent` counterpart to a mouse click's `Activated` outcome — `accesskit::Action::Click` from a screen reader has no press/release pair or on-screen point, just a named target. Deliberately doesn't check `access.actions` first, matching the real mouse-click path's own symmetry (hit-testing alone decides *which* node; whether anything reacts is the caller's own lookup).

`Tree::set_focus_to(node, ...)` is the direct-target counterpart to `move_focus`'s tab-order computation — what `accesskit::Action::Focus` actually needs (§10: "a screen reader focusing a node directly" dispatches this). The shared "animate the old node's `focus_ring` out, the new one's in, opt-in-only" logic was factored out of `move_focus` into a private `transition_focus` helper, reused by both — not duplicated.

5 new unit tests: the `to_access_id`/`from_access_id` round trip; a stale id (from a since-removed node) failing safely, not silently resolving; `activate` producing `Activated`/`None` correctly; `set_focus_to` jumping directly to a named node (not computing a tab-order neighbor) and animating `focus_ring` correctly on both ends of the transition; `set_focus_to` on an unknown node leaving real focus untouched. All passed on the first run. `engine-core` now at 37 unit tests.

**`engine-platform`**: `run_windowed_multi` gained a sixth parameter, `on_access_action: FnMut(WindowId, accesskit::ActionRequest)`, firing on `accesskit_winit::WindowEvent::ActionRequested` — replacing the comment that's named this exact gap since step 7. Matches `on_input`'s own "hand the raw platform data up, never touch a `Tree`" shape, except the data crosses *unrtranslated* this time: `engine-platform` doesn't know `engine_core::NodeId` exists, and the conversion needs `engine_core::from_access_id`, which only a caller already depending on `engine-core` can call. `AccessibilityDeactivated` needed no new handling — every `TreeUpdate` build already goes through `access_adapter::update_if_active`, which already gates on activation state.

**Real, checked-before-assuming constraint, matching Phase 1 step 2's own precedent**: no live AT-SPI/UIA/NSAccessibility client is available in this dev/CI environment to drive a genuine end-to-end screen-reader proof (unlike M3 step 7's original wiring, which *was* checked against a real, interactively-running AT-SPI bus at the time). The honest, real proof here is at the logic level: `from_access_id`'s round-trip and `activate`/`set_focus_to`'s own behavior are fully unit-tested (above); `engine-platform`'s translation is thin, untranslated pass-through with no logic of its own to unit-test beyond compiling correctly through every call site.

**`engine-py`**: `App::run()` gained a new `on_access_action` closure — converts `request.target_node` via `from_access_id`, matches `request.action` (`Action::Click` → `tree.activate(node)`, `Action::Focus` → `tree.set_focus_to(node, ...)` using `dispatch::interaction_config()`'s existing values), and feeds the outcome through the *same* `dispatch::run_activation` already shared between `on_input` and `Window.click()` — a third real call site for one click-handling mechanism, not a third one.

`Window.press_key(key, shift=False)` (new): `click()`'s own keyboard counterpart, translating a small Python string vocabulary (`"tab"`/`"enter"`/`"space"`/`"escape"` — an unknown value raises `ValueError`) into `engine_core::Key` and dispatching through the identical `interaction_config()`/`run_activation` path. Closes the stated Phase 1 gap directly: Tab-then-Enter/Space activation is now testable from Python with no live window, the same way `click()` already made pointer activation testable.

## Verification

6 new pytest tests in `test_keyboard_dispatch.py`: Tab then Enter activates the focused node; Tab then Space does too; Tab cycles between two nodes and wraps; Shift-Tab moves backward (and wraps to the *last* node when nothing was focused); Enter with nothing focused activates nothing; an unknown key name raises `ValueError`. All passed on the first run.

`engine-py`'s `on_access_action` closure body itself is *not* separately pytest-covered beyond compiling and type-checking correctly — it's a thin conversion plus a match onto the same two `Tree` methods already fully unit-tested at the `engine-core` level, and the same `run_activation` already covered by `test_click_dispatch.py`. A genuine end-to-end proof needs a live AT-SPI/UIA/NSAccessibility client, which this environment doesn't have — stated plainly rather than manufacturing a test that would only re-prove what the `engine-core` tests already prove.

```
$ cargo test --workspace              # all green: engine-core 37, engine-platform 4, rest unchanged
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                   # clean

$ maturin develop
$ python -m pytest tests/ -v
32 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py     # exited cleanly after 60 frames
$ python examples/two_windows.py      # exited cleanly after 60 frames, both windows
```

Both real windowed tests (`multi_window.rs`, `access_button.rs`) still pass unchanged with the new `on_access_action` parameter threaded through every call site.

## Known scope narrowing (stated, not silent)

- **No live AT-SPI/UIA/NSAccessibility end-to-end proof exists in this environment** — the mechanism is real and unit-tested at every layer it can be without one; a genuinely interactive screen reader driving a real click/focus request through the full stack is real, separate follow-up work whenever such an environment is available (matching M3 step 7's own original verification, which *did* have one).
- **`Action::Click`/`Action::Focus` are the only two accesskit actions this step gives real dispatch meaning to** — scrolling, text selection, and custom actions still have no model anywhere in this codebase; not manufactured ahead of a real need.

## Next

M4 Phase 2 is complete. No further M4 phase is yet scoped. `BUILD_TRACKER.md` updated at all three levels. Real remaining gaps named across both phases: §11.9 transform composition (blocking transform-aware hit-testing), a real scrollable viewport for `VirtualList` (§11.8), and `NodeKind::Canvas` (blocking custom hit-testing, §11.10's own second half) — each its own, later milestone-shaped body of work.
