# Log: M4 Phase 3, Step 2 — `Window.add_splitter` (§11.5)

Corresponds to `BUILD_TRACKER.md` M4 Phase 3, step 2 of 2. Step 1 (`f6d4aec`) built the real drag mechanism entirely inside `engine-core`/`engine-render`: `Tree::dispatch` now starts, live-tracks, and ends a real splitter drag from real pointer events. That step's own `LOG.md` named the one real gap it left open explicitly: `PyWindow` had no method to create a `NodeKind::Splitter` from Python at all, so the real mechanism had nothing to act on from a real app. This step closes it.

## What happened

`Window.add_splitter(background, width, height, initial_position=0.5)` — mirrors `add_rect`'s own exact parameter shape (`background` first, matching §11.5's own text: "a real splitter typically just wants a background for its own grip/handle," since `SplitterState` carries no separate appearance data) plus one new keyword parameter, `initial_position` (0.0..=1.0 along the split axis, defaulting to an even split). Inserts a `NodeKind::Splitter(SplitterState { position: Animated::new(initial_position) })` and appends it to the window's own root row, the same append-only way `add_rect` already works.

No new mechanism needed: the window's root is already a flex row (`PyWindow::new`'s own existing setup), which is exactly the flex parent `Tree::splitter_geometry` expects a splitter to sit inside, directly between two real siblings. Calling `add_rect` (left pane), then `add_splitter`, then `add_rect` (right pane) — in that order — produces exactly the resizable-pane layout §11.5's own architecture text describes, with no separate "pane container" concept invented for this.

## Verification

5 new pytest tests in `test_splitter.py`: `add_splitter` returns a `Node`; a full left/splitter/right layout builds without error; the default `initial_position` (omitted) doesn't raise; a custom `initial_position` is accepted; clicking a splitter via `Window.click()` (press+release with no `PointerMoved` in between, so `Tree::update_drag` never runs) doesn't crash — real click-without-drag behavior, not an edge case being carved around. All passed on the first run.

A new example script, `examples/resizable_panes.py`, opens a real window with a working left/splitter/right layout and runs for real frames, exiting cleanly (headless-CI-safe, matching every other example in this workspace) — the honest, real proof of what's automatable: construction and rendering through the real pipeline. Stated explicitly in the script's own docstring: it does *not* automatically prove an actual mouse drag resizes the panes on screen, since that needs a real human pointer (or the synthetic `InputEvent` sequences `engine-core`'s own unit tests and `engine-render`'s `splitter_drag_dispatch.rs` pixel test already drive without one) — a human running this script interactively can drag the splitter and see it for real.

```
$ cargo test --workspace              # all green, unchanged (this step touched only engine-py + tests/examples)
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                   # clean

$ maturin develop
$ python -m pytest tests/ -v
37 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/resizable_panes.py   # exited cleanly after 180 frames
$ python examples/animate_rect.py      # exited cleanly after 60 frames (unaffected)
$ python examples/two_windows.py       # exited cleanly after 60 frames, both windows (unaffected)
```

## Next

M4 Phase 3 is now fully complete (both steps). No further M4 phase is yet scoped. Real, stated-not-silent gaps unchanged from step 1: "drag-to-rearrange docking" (moving a whole panel between zones, not resizing) remains a separate, untouched, larger feature; two-phase press/hold/release ripple timing and `HoverEnter`/`HoverExit` firing through `engine-spec`'s handler path (§7.3) are both still real, separate, untouched pieces of M4's overall named scope.
