# Log: M4 Phase 3 — Real Pointer-Drag Dispatch (§11.5, §11.4)

Corresponds to `PLAN.md`/`BUILD_TRACKER.md` M4 Phase 3. Phases 1-2 (`c761d1c`/`1d6086b`/`b3b179a`) built real pointer/keyboard dispatch and real assistive-technology action dispatch end to end. One real, repeatedly-named gap remained from every earlier splitter/docking step: `Tree::set_splitter_position` has existed since M3 step 15 Stage A as a direct, programmatic API, and every later mention of it said the same thing — real splitter-drag input needs real pointer dispatch, which didn't exist yet. It does now (M4 Phase 1), so this phase wires it up.

## What happened

`Tree` gained a `dragging: Option<NodeId>` field alongside the existing `pressed`/`hovered` fields, same lifecycle shape. The "find a splitter's own flanking siblings, its parent's flex axis, and their current combined extent" logic that used to be inlined in `set_splitter_position` was factored into a private `splitter_geometry` helper, so a second real caller could reuse it without duplicating it.

`Tree::update_drag(point, now)` (new, private): converts `point`'s coordinate along the dragged splitter's own parent flex axis into a 0.0..=1.0 fraction — relative to the left sibling's own current absolute start position and the flanking siblings' combined extent, which stays constant during a drag since the two siblings only ever trade extent between each other — then calls the *existing* `set_splitter_position` with it. §11.5's own text describes exactly this: "on drag, `position`'s tick handler mutates its two adjacent siblings' `layout_style`... used both standalone... and by docking (§11.4) for zone resizing — one mechanism, two call sites." This is that mechanism made real, reusing `set_splitter_position` verbatim rather than reimplementing the resize math for the drag case.

`Tree::dispatch` changes: `PointerPressed` on a `NodeKind::Splitter` with the primary button sets `self.dragging = Some(node)`, reusing the same hit-test call `dispatch` already makes for click-tracking and ripple-spawn — not a second hit-test. `PointerMoved` calls `update_drag` whenever a drag is active, in addition to its existing `update_hover` call — both run on every move, since dragging one thing doesn't mean hover elsewhere stops mattering. `PointerReleased` with the primary button clears `self.dragging` unconditionally — a real mouse-up always ends a drag wherever it happens, not conditioned on still hitting the splitter (the pointer can leave a splitter's own thin hit region mid-drag, and a real drag must keep tracking it until release, matching real OS drag semantics — this "capture" behavior falls out for free here since `update_drag`'s own geometry computation never depends on hit-testing at all, only on `self.dragging`'s own stored identity).

Because docking's own zone-resize already reuses `Tree::set_splitter_position` verbatim (M3 step 15 Stage B's own design), dragging a dock-zone boundary is real now too, with zero docking-specific code touched in this phase — the same "one mechanism, multiple real call sites" story this whole project has followed since step 13's overlay work.

## Verification

4 new `engine-core` unit tests, each isolating one claim: dragging a splitter live-follows the cursor across *two* separate drag positions (not just a one-shot snap, sampling mid-drag at x=130 then again at x=160 to prove continuous tracking, not a single before/after check); releasing genuinely ends the drag, so a further pointer move afterward leaves the pane sizes untouched; pressing on a plain, non-splitter node never starts a drag (dragging from an ordinary button must never move an unrelated splitter sharing its parent); a non-primary-button press on a splitter never starts one either. All passed on the first run. `engine-core` now at 41 unit tests.

A new `engine-render` pixel-readback test, `splitter_drag_dispatch.rs`, mirrors `splitter_drag.rs`'s own exact scene and render harness but drives the whole sequence through `Tree::dispatch`'s real `PointerPressed`/`PointerMoved`/`PointerReleased` events instead of a direct `set_splitter_position` call — the concrete, on-screen proof that real dispatch, not just the underlying mechanism, moves a real rendered pane boundary, including a genuine mid-drag (pre-release) sample and a post-release "further moves don't keep dragging" check. Passed on the first run.

This phase touched only `engine-core` (the mechanism) and added one `engine-render` test — no changes were needed anywhere in `engine-platform` or `engine-py`, since `Tree::dispatch` is the exact same function `App::run()`'s existing `on_input` closure already calls for every real `winit`-translated pointer event. The whole real, mouse-driven drag path already existed the moment `Tree::dispatch` gained it.

```
$ cargo test --workspace              # all green: engine-core 41 (was 37), rest unchanged
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                   # clean
$ maturin develop && python -m pytest tests/ -v   # 32 passed, 1 skipped benchmark, unaffected
```

## Known scope narrowing (stated, not silent)

- **No Python-facing way to create a `NodeKind::Splitter` exists yet** — `PyWindow` exposes `add_rect`/`add_virtual_list`, but no `add_splitter`. The drag mechanism is real and proven end to end at the `engine-core`/`engine-render` level (and would work immediately through `App.run()`'s existing real `winit` wiring the moment a splitter node exists), but a Python app author can't actually build a resizable-pane UI today without that method. Real, additive, narrow follow-up whenever that's needed — not manufactured ahead of this phase's own stated scope (the drag *mechanism*, not a full Python-facing pane-layout API).
- **"Drag-to-rearrange docking"** (picking up a whole dock *panel* and moving it to a different zone) remains untouched — a genuinely different, larger feature (drop-target detection, panel reparenting mid-drag) than the resizing this phase built.
- **Two-phase press/hold/release ripple timing** and **`HoverEnter`/`HoverExit` firing through `engine-spec`'s handler path** (§7.3's own text) are both still real, separate, untouched pieces of M4's overall named scope.

## Next

M4 Phase 3 is complete. No further M4 phase is yet scoped. The most concrete, immediately-useful next step if M4 continues: `Window.add_splitter` in `engine-py`, which would make this phase's already-real drag mechanism actually reachable from a Python app for the first time.
