# LOG — M39 Phase 2 Step 2: Time Picker Dial (closes Phase 2)

- User's own explicit instruction: "Start" continued into M39 Phase
  2's own second step, per the earlier `AskUserQuestion` answer for
  this exact item: "Build it now, honest approximation."
- Real investigation first, delegated to an Explore subagent, then
  personally spot-checked against the live source before writing any
  code: confirmed no existing drag primitive in this codebase does
  angle-based math (`Splitter`/`Slider`/`Carousel`/`ScrollView` are
  all linear -- zero real hits for `atan2` anywhere in `engine-core`/
  `engine-render`), and confirmed circular hit-testing only exists for
  `NodeKind::Canvas` via `CustomHitTest::Circle` (M5 Phase 3) -- not
  reused here; the dial deliberately uses the ordinary rectangular
  hit-test (its own whole bounding box), the identical real simplicity
  `Slider`'s own whole-track-width hit region already uses rather than
  a pixel-exact thumb hit box, stated as a real, deliberate v1 choice
  in `TimePickerDialState`'s own doc comment.
- Traced the real, full call path a `Splitter`/`Slider` drag takes,
  start to finish, before writing anything: a raw winit event ->
  `engine-py::app.rs`'s single real dispatch chokepoint -> `Tree::
  dispatch`'s `PointerPressed`/`PointerMoved`/`PointerReleased` arms ->
  `update_drag` -> a kind-specific `update_*_drag` -> the real public
  setter. The dial reuses this exact shape end to end -- one real
  mechanism, extended, not a second one invented in parallel.
- New `NodeKind::TimePickerDial(TimePickerDialState)` (`node.rs`) --
  `hour: u8` (real 24-hour value, `0..=23`), `minute: u8` (`0..=59`),
  `mode: TimePickerDialMode` (`Hour`/`Minute`, selects which hand a
  drag moves), `face_tint`/`hand_tint: Color`. `hour`/`minute` are
  plain, driven-directly fields, not `Animated<f64>` -- the identical
  real "driven directly, like a scrollbar being dragged, never eased"
  precedent `CarouselState::scroll_x`'s own doc comment already
  establishes: a clock hand snapping instantly to wherever the
  pointer is IS the correct real behavior.
- **Real, stated v1 simplifications** (all in `TimePickerDialState`'s
  own doc comment): no digit labels around the face (plain tick marks
  stand in -- painting real text needs `engine-render`'s own
  text-shaping pipeline, real added plumbing this v1 skips); no AM/PM
  toggle or digital-input dialog chrome (this is the real circular
  drag *primitive* MD3's own Time Picker dialog is built from, not the
  whole dialog -- `Window.add_time_input_field`/`add_period_selector`,
  M30 Phase 7 Step 2's own separate digital variant, already exist and
  are unaffected).
- `Tree::update_time_picker_dial_drag` (`tree.rs`): `atan2(dy, dx)`
  from the node's own real box center, rotated so 12 o'clock is the
  real zero point and wrapped into `0.0..TAU` via `rem_euclid` (not
  plain `%`, which would leave a real negative remainder just
  counter-clockwise of 12). This is byte-for-byte the same real angle
  convention `engine-render`'s own pre-existing `CircularProgress`
  paint arm already established (`-PI/2` start, `+angle` clockwise) --
  confirmed both directions agree by hand-tracing all four cardinal
  points against where they actually render. Hour mode: `round(
  fraction * 12) % 12`, then re-adds whichever 12-hour period `hour`
  was already in (`+12` if it was already PM) -- dragging the hour
  hand alone must never silently flip AM/PM, since this widget has no
  toggle of its own. Minute mode: `round(fraction * 60)` snapped to
  the nearest multiple of 5 via `((raw + 2) / 5 * 5) % 60` (integer
  rounding-to-nearest-5, hand-verified against several boundary
  values).
- Wired into the same three real chokepoints `Splitter`/`Slider`
  already use: `update_drag`'s match gains a `TimePickerDial` arm;
  `dispatch`'s `PointerPressed` arm widens its "start a drag" check to
  include `TimePickerDial`; `PointerReleased`'s "a drag ending is a
  real, meaningful edit" `Changed` outcome widens the same way. New
  `Tree::set_time_picker_dial_time`/`set_time_picker_dial_mode` public
  setters for a programmatic (non-drag) move, mirroring `set_slider_
  position`'s own shape.
- `engine-render`'s new `NodeKind::TimePickerDial` paint arm: filled
  face (`face_tint`), 12 real tick-dot positions at 40% opacity (the
  honest stand-in for real digit labels), an hour hand and a longer
  minute hand (real `Line` strokes from center, `hand_tint`), a real
  selector dot at whichever hand `mode` currently makes draggable
  (MD3's own real "which hand is active" indicator), and a small
  center hub -- the same real anatomy a physical analog clock face
  has.
- `Window.add_time_picker_dial(hour, minute, size, x, y) -> Node`
  (`engine-py::window_factory.rs`): themes `face_tint`/`hand_tint`
  from `surface_container_highest`/`primary`, the identical real
  pattern `add_linear_progress` already established for its own two
  colors (`track_tint`/`indicator_tint`). `size` defaults to `256.0` --
  a practical, legible default, explicitly **not** cited as a verified
  MD3 dp token (this phase's own research located the dial's real
  anatomy and interaction model, not a confirmed default diameter from
  an authoritative source, so this doc comment says so plainly rather
  than presenting an unverified number as fact).
- New dedicated `Node.set_time_picker_dial_time`/`get_time_picker_
  dial_time`/`set_time_picker_dial_mode`/`get_time_picker_dial_mode`
  (`engine-py::node.rs`) -- `hour`/`minute` are plain values, not
  `Animated<f64>`, so they can't go through the generic `Node.animate(
  )`/`Node.get()` f64-only dispatch; mirrors `set_carousel_index`/
  `get_carousel_index`'s own already-established real precedent for
  exactly this situation. `mode` is a `"hour"`/`"minute"` string, the
  identical vocabulary convention `parse_content_fit`/`parse_dock_
  side` already establish for a small, closed Rust enum exposed to
  Python (no dedicated pyo3-native enum type built for just two
  variants).
- **Real bug found and fixed in my own first draft of the drag tests
  -- the same "verify, don't assume" discipline this whole project has
  already applied repeatedly to its own code, not just the engine's:**
  the first four-quadrant hour-drag test dispatched only a single
  `PointerPressed` at each target point and asserted the resulting
  hour directly. It happened to "pass" for the very first case (12
  o'clock -> hour 0) purely by coincidence -- `TimePickerDialState::
  new(0, 0)`'s own default hour already IS 0, so an inert press proved
  nothing. The second case failed loudly (expected 3, got 0), which is
  what actually surfaced the real bug: `PointerPressed` alone only
  ever *starts* a drag (`self.dragging = Some(node)`) -- the value
  itself only moves on the real `PointerMoved` that follows, the
  identical real shape `slider_scene`'s own existing drag tests
  already use and which I initially failed to mirror. Fixed by
  redesigning every drag test to press at a neutral point first, then
  move to the real target point before asserting.
- Also found while writing the same tests: `rect_contains`'s own real
  `Rect::contains` (kurbo's standard convention) is exclusive on the
  box's max edge -- a test point exactly on the dial's own right/
  bottom edge (`x == width` or `y == height`) misses the hit-test
  entirely and never starts a drag at all. Fixed by moving every such
  test point fractionally inside the box (`199.0` rather than the
  exact `200.0` edge) rather than exactly on it.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`
  (one real doc-comment lint fix needed along the way: a new paragraph
  directly following a bulleted list without a blank `///` separator
  reads as an unindented list continuation, `clippy::doc_lazy_
  continuation`); `cargo fmt` + `cargo fmt --check`; `cargo test
  --workspace --release` (`engine-core`: 208 passed, +7 from this
  step); `maturin develop --release`; `pytest tests/` (581 passed, 1
  skipped, up from 569, +12 new tests in `tests/test_time_picker_dial.
  py`, explicitly checked for filename collisions first and confirmed
  distinct from the pre-existing digital-variant `test_time_picker.
  py`); all 77 examples including the new `examples/time_picker_dial.
  py` (constructs two dials, moves one via the direct setter and the
  other via mode-switch-then-setter, round-trips both through the real
  getters, runs a real 60-frame live loop); showcase demo.
- `python/tre/_core.pyi` given a full, paired `add_time_picker_dial`
  factory stub plus all four new `Node` method stubs.
- `BUILD_TRACKER.md` Phase 2's own heading, Step 2's own bullet, the
  milestone status line ("Phase 2 of 5 done", up from "Phase 1"), and
  the Top Metrics row (40%, up from 20%) all updated together --
  unlike Step 1 alone, which only ever touched its own bullet, this
  closes the whole real phase. Parser re-confirmed balanced (39
  milestones, 127 phases, 218 items, unchanged); artifact regenerated
  and republished. **M39 Phase 2 is now fully complete. Phases 3-5
  remain: Shape-Morphed Border Inset Fix, Terminal Cell Text
  Attributes, `Tree::tick_all` Active-Set Optimization.**
