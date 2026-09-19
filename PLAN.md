# PLAN — M39 Phase 2 Step 2: Time Picker Dial (closes Phase 2)

## Goal
Give the catalog a real MD3 Time Picker's own analog circular drag
control -- named since M35's own scoping as "the single largest real
gap: a genuinely new drag-to-angle engine capability, no precedent in
either TRE or pyCopper."

## Steps
1. Real investigation first (delegated to an Explore subagent, then
   personally spot-checked): confirmed every existing drag primitive
   (`Splitter`/`Slider`'s own track-position math, `Carousel`'s own
   horizontal delta, `ScrollView`'s own thumb travel) is linear -- no
   `atan2`, no circular hit-testing distinct from a plain bounding
   box, anywhere in `engine-core`/`engine-render`. Traced the full
   real call path a Splitter/Slider drag takes (raw winit event ->
   `engine-py::app.rs`'s one dispatch chokepoint -> `Tree::dispatch`'s
   `PointerPressed`/`PointerMoved`/`PointerReleased` arms ->
   `update_drag` -> a kind-specific `update_*_drag` -> the real public
   setter) to reuse the identical shape for the dial, not invent a
   parallel one.
2. New `NodeKind::TimePickerDial(TimePickerDialState { hour: u8,
   minute: u8, mode: TimePickerDialMode, face_tint: Color, hand_tint:
   Color })` (`node.rs`) -- `hour`/`minute` are plain, driven-directly
   fields (the same real "never eased, like a scrollbar being
   dragged" precedent `CarouselState::scroll_x` already establishes),
   not `Animated<f64>`.
3. `Tree::update_time_picker_dial_drag` (`tree.rs`): converts a
   pointer position into an angle from the node's own real box center
   via `atan2`, using the *identical* real convention `engine-render`'s
   own `CircularProgress` paint arm already established (12 o'clock =
   zero, sweeping clockwise) -- confirmed both directions agree by
   hand-tracing all four cardinal points. Hour mode snaps to 1 of 12
   real positions (`round(fraction * 12) % 12`), preserving whichever
   half of the day `hour` was already in (no AM/PM toggle chrome in
   this v1); minute mode snaps to the nearest real 5-minute increment.
   Wired into `update_drag`'s match, `dispatch`'s `PointerPressed`
   (starts the drag, reusing the ordinary rectangular hit-test) and
   `PointerReleased` (produces `DispatchOutcome::Changed`, mirroring
   `Slider`'s own real "a drag ending is a meaningful edit" outcome).
   New `Tree::set_time_picker_dial_time`/`set_time_picker_dial_mode`
   public setters for programmatic moves.
4. `engine-render`'s own new `NodeKind::TimePickerDial` paint arm:
   filled face, 12 real tick-dot positions (the honest v1 stand-in for
   real MD3's own painted digit labels -- drawing real text needs
   `engine-render`'s text-shaping pipeline, real added plumbing this
   v1 skips), an hour hand and a longer minute hand (both real `Line`
   strokes from center), a real selector dot at whichever hand `mode`
   currently makes draggable, and a small center hub.
5. `Window.add_time_picker_dial(hour, minute, size, x, y) -> Node`
   (`engine-py`), theming `face_tint`/`hand_tint` from
   `surface_container_highest`/`primary` the identical real way
   `add_linear_progress` already themes its own two colors. New
   dedicated `Node.set_time_picker_dial_time`/`get_time_picker_dial_
   time`/`set_time_picker_dial_mode`/`get_time_picker_dial_mode`
   methods, mirroring `set_carousel_index`/`get_carousel_index`'s own
   established "a plain, non-`Animated<f64>` field needs its own
   dedicated getter/setter, not the generic `Node.animate()`/`Node.
   get()` dispatch" pattern. Mode exposed as a `"hour"`/`"minute"`
   string, the identical vocabulary convention `parse_content_fit`
   already establishes for a small closed Rust enum.
6. Real tests: 7 new `tree.rs` unit tests (state clamping, the direct
   setter's own clamping, mode switching, a real hand-traced drag test
   at all four clock quadrants, an AM/PM-preservation test, a real
   17-minute angle proving genuine 5-minute snapping rather than
   coincidence, and release-produces-`Changed`). **Found and fixed a
   real bug in my own first draft of these tests** -- see `LOG.md`.
   New `tests/test_time_picker_dial.py` (12 tests) and `examples/
   time_picker_dial.py`, both explicitly checked for filename
   collisions first and confirmed genuinely distinct from the
   pre-existing `test_time_picker.py`/`time_picker.py` (M30 Phase 7
   Step 2's own real, separate *digital* Time Input variant, whose own
   doc comment had already named this analog dial as the deferred
   gap this step now closes).
7. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 77 examples, showcase demo.
8. `BUILD_TRACKER.md` Phase 2 flipped fully to done (both steps, the
   phase heading, and the milestone status line/Top Metrics row all
   updated together, unlike Step 1 alone which only touched its own
   bullet) -- **M39 Phase 2 as a whole is now complete.** Artifact
   regenerated (39/127/218, unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 208 passed, +7 from this step; `pytest
tests/`: 581 passed/1 skipped, up from 569, +12 new tests; all 77
examples + showcase demo clean). **M39 Phase 2 -- Loading Indicator +
Time Picker Dial -- is now fully complete (both steps). Phases 3-5 of
M39 remain: Shape-Morphed Border Inset Fix, Terminal Cell Text
Attributes, `Tree::tick_all` Active-Set Optimization.**
