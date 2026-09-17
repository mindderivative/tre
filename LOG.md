# Log: M14 Phase 1 — Real Checkbox (§5, §7.3)

Corresponds to `BUILD_TRACKER.md` M14 Phase 1. `NodeKind::Checkbox
(CheckboxState)` with real paint and real click-to-toggle interaction.

## Investigation before writing code

ARCHITECTURE.md §5's own struct sketch shows only `check_progress:
Animated<f64>` for `CheckboxState`, but §7.3's own text separately
names `CheckboxState.checked` as a real field — the struct needed both.
Design Principle 6 ("selection/checked-state... depend on what the
app's data means") confirms the engine must not auto-toggle `checked`
on click — that's app-owned; the "click-to-toggle interaction" this
phase's own scoping named is the already-fully-generic `Click`
dispatch + `Node.enable_interaction()` ripple mechanism, confirmed to
already work for any `NodeKind` with zero new dispatch code.
`Tree::tick_all` only ticked `node.paint`/`node.interaction` before
this phase — no kind-specific payload was ticked centrally
(`SplitterState.position`/`VirtualListState.scroll_offset` are driven
directly, never eased) — `check_progress` needed one new arm.
`AccessStates` only carried `disabled` before this phase; that
module's own doc comment already named `CheckboxState.checked` as the
reason. `accesskit` 0.25.0 (pinned) has a real `Toggled { False, True,
Mixed }` + `Node::set_toggled`, with `From<bool> for Toggled`.
`Node.animate()`/`.get()` only dispatched against `PaintProperties`'
own universal fields before this phase — the "two-level dispatch"
ARCHITECTURE.md §8 describes wasn't built yet; this is its first arm.

## What happened

`AccessStates` deliberately gains no new field — `Tree::build_access_
update` reads `checked` directly from `NodeKind::Checkbox`, avoiding a
second copy of the same fact that could drift out of sync.
`CheckboxState::new(checked)` seeds `check_progress` already matching
`checked` (`1.0`/`0.0`), so a checkbox created already-checked shows
its own real initial state with no spurious animation. `tick_all`
gains one new arm ticking `check_progress`. `build_access_update`
gains the real, automatic `checked` -> `Toggled` derivation
ARCHITECTURE.md promised. `paint_node` gains a `Checkbox` arm: the box
paints exactly like a `Rect` (same rounded-rect fill), then a real
checkmark `BezPath` strokes on top, its own opacity driven directly by
`check_progress` -- a plain white mark, real but not yet theme-aware
(the same "wire theme later" precedent ripple's own hardcoded tint had
before M7 Phase 3). `Node.animate("check_progress", ...)`/`.get(
"check_progress")` reach `CheckboxState.check_progress`; new `Node.
set_checked(bool)` is the plain, non-animated write to `checked`. New
`Window.add_checkbox(checked=False, ...)` mirrors `add_rect`'s own
real shape.

New `engine-core` tests: `tick_all` genuinely animates `check_progress`
toward a real target over real ticks (mid-flight, then settled at the
target); `build_access_update` reports the real `Toggled` state. New
`engine-render/tests/checkbox_paint.rs`: an unchecked box paints no
visible checkmark at all; a checked box paints a real, fully-opaque
white checkmark exactly where the real tick path draws it, and nowhere
else.

**Real finding, caught by pytest, not the implementation:** the first
draft of `test_checkbox.py` called `.animate("check_progress", ...)`
then immediately `.get("check_progress")`, expecting the new value —
but `Animated<T>` (every field `animate()` dispatches to) only updates
its own real `current` once a real tick runs (`Tree::tick_all`, driven
by `App.run`'s own per-frame loop); calling `.animate()` then
`.get()` with no tick in between reads the pre-animation value by
design, the same real behavior every other `animate()`-dispatched
field already has — not a Checkbox-specific bug, confirmed via direct
read of `Animated::animate_to`, and no existing pytest test in this
suite reads a value back this way either. Fixed by testing what pytest
*can* prove without a live render loop (both calls reach the real
`Checkbox` state without raising) and leaving the "did a real tick
genuinely animate it" claim to the pixel test, which already proves it
via a real render.

New `tests/test_checkbox.py` (7 tests) + new `examples/checkbox.py`: a
real, live click-to-toggle checkbox with its own real animated
checkmark.

Full `cargo test --workspace --release` (`engine-core` 88, up from 86,
plus 2 new `engine-render` pixel tests)/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean. `maturin
develop --release` + full `pytest tests/` (113 passed, up from 106, 1
skipped) and all twenty examples (nineteen existing + new `checkbox.
py`) confirmed clean.
