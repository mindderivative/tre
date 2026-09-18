# Log: M29 — Render Loop Dirty-Tracking

Closed the one item M28 deliberately left open: `engine-platform`'s
render loop redrew every frame unconditionally, regardless of whether
anything actually changed.

## Phase 1 — the dirty flag

`Tree::nodes` is private, so every real mutation is already forced
through one of `Tree`'s own public `&mut self` methods. Enumerated the
full list precisely (grepping both single-line and multi-line
signatures, since a naive single-line grep silently missed `insert`/
`dispatch`/`open_overlay`/`set_virtual_list_window` — a real, near-miss
found before it became a real gap): 29 methods, not the ~25 estimated
while scoping.

Wrote a small Python script to insert `self.dirty = true;` as the
first statement of each target method's body, using exact paren-depth
matching to find where each signature's `{` actually starts (handling
multi-line parameter lists correctly). Mechanical insertion across 29
methods by hand would have been the real risk here — a script that
finds the exact same brace precisely every time removes that risk
entirely, verified afterward by spot-checking a few multi-line
signatures (`insert`, `dispatch`) landed the insertion in the right
place.

`tick_all` needed its own conditional handling — it runs every frame
regardless, so marking it dirty unconditionally would defeat Phase 1's
whole purpose. Instead: `if any_active { self.dirty = true; }` at its
return point, reusing the value it already computes.

New regression test proves `take_dirty()` after each real mutation
category (structural, paint-property via `get_mut`, interaction via
`interaction_mut`, animation-tick via `tick_all`) and clean between
read-only calls.

`App::run`'s per-frame closure (`app.rs`) then reads `tree.take_dirty()`
once, right after `tick_all`, and returns early — skipping
`compute_layout`/GPU texture sync/scene encoding/submit/present
entirely — when nothing real happened. Checked whether window resize
needed special handling here (the scoping doc flagged it): it doesn't,
because this codebase has no resize support at all yet, confirmed via
grep — a real, pre-existing, separate gap, not something this phase
needed to preserve.

## Phase 2 — true idle

Widened `run_windowed_multi`'s `on_frame` to return `bool` (still
animating). `PerWindow` gained an `animating` field; `RedrawRequested`
now sets the event loop's `ControlFlow` to `Poll` while any open window
last reported animating, `Wait` once every window has settled
(`ControlFlow` is event-loop-wide, not per-window). Every real
input-handling arm in `window_event` now calls `request_redraw()`
explicitly, since nothing else wakes a waiting loop.

Audited the one non-`WindowEvent` redraw trigger the scoping doc named:
AccessKit's `ActionRequested` (a screen-reader-driven `Action::Focus`/
`Action::Click`), delivered via `user_event`, had no redraw wiring at
all — added one.

**Real bug, caught only by actually running the whole example suite
against this change, not assumed:** several examples hung indefinitely.
Root cause: a window opened with `max_frames: Some(_)` — the pattern
essentially every example and test in this workspace uses to run for a
bounded number of frames and exit — relied on continuous polling to
ever reach its own frame count. Once idle windows stopped
auto-polling, a static example with no animation and no real human
interacting with it never got another redraw request at all, so it sat
at frame 1 forever. Fixed by treating `max_frames: Some(_)` as always-
animating for `ControlFlow` purposes, independent of what `on_frame`
itself reports. Re-ran the full example suite after the fix — all 29
pass, this time genuinely (not just "didn't time out because the
overall script timeout was generous").

Empirically verified the actual idle-CPU claim, not just the code path:
a scratch script opening an unbounded, non-animating window settled to
under 1.5% CPU over several real seconds with a real display attached.

## Verification

Full `cargo check`/clippy `-D warnings`/fmt clean. `cargo test
--workspace --release` clean, `engine-core` 146 (up from 145).
`maturin develop --release` + pytest (187 passed, 1 pre-existing skip),
all 29 examples, and the showcase demo (real click/keyboard/drag
interaction throughout, not a static scene) all clean with the real
display.

M29 — Render Loop Dirty-Tracking is now complete. Both phases shipped
together, not split into a separate go/no-go the way the original
scoping proposed — Phase 1's own low-risk mechanical instrumentation
gave enough confidence, and Phase 2's one real risk (a missed redraw
path) was caught and fixed by actually running everything, the same
discipline this project applies throughout.
