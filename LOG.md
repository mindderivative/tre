# LOG — M35 Phase 3: Button Groups, closing M35

- Real anatomy read directly from `COMPONENT_BUTTON_GROUPS.md` before
  writing any code: Standard variant reflows adjacent buttons' widths
  when one is pressed; Connected variant explicitly, per the spec's
  own words, replaces the already-built `Segmented Button` -- ruled
  out of this phase's own scope immediately, no duplicate work needed.
- Investigated the real architectural precedent this phase's own
  M35-scoping note promised before writing any code: `Tree::sync_
  carousel_layouts` (M30 Phase 9 Step 5) already does exactly the
  needed shape -- a container-level marker recomputes every child's
  own real `layout_style`, pushed via `Tree::set_layout_style`, with
  `compute_layout` running `taffy` a second time so the new absolute
  insets actually land in `self.layout(child)`.
- Real, load-bearing confirmation before designing the reflow trigger:
  `Tree.pressed: Option<(PointerButton, NodeId)>` already exists and
  is already tracked by `Tree::dispatch`'s own `PointerPressed`/
  `PointerReleased` handling -- meaning the new sync function needs
  zero new interaction wiring, just a read of an already-live field,
  the identical real "read live interaction state to drive computed
  layout" technique `update_slider_drag`/`update_splitter_drag`
  already establish.
- New `PaintProperties.button_group_reflow: Option<(f64, f64)>`
  (grow_px, gap_px) -- deliberately a plain field, not a new
  `NodeKind`, since (unlike `Carousel`) a button group needs no other
  real per-instance data. Confirmed via grep first that `PaintProperties
  ::new` is the sole real construction path everywhere (no raw struct
  literals to migrate), so this addition was safe and mechanical.
- New `Tree::sync_button_group_layouts`, wired into `compute_layout`
  right after the existing carousel sync call. Real, deliberately
  simple, honestly-stated formula: no discrete numeric token for the
  "grow" amount exists anywhere in the scraped spec, only the
  qualitative "briefly changes the width of itself and adjacent
  buttons" -- the pressed child grows by the group's own real
  `grow_px`; that amount is split evenly back out of its immediate
  left/right neighbors (clamped at 0.0), so the row's own total width
  provably stays constant, a real bounded reflow rather than raw
  uncompensated growth.
- Two real, decisive Rust unit tests added directly in `tree.rs`'s own
  test module (private-field access to `tree.pressed` used directly,
  no synthetic dispatch round-trip needed for this pure layout-math
  proof): with nothing pressed, every child keeps its own exact
  resting width; with the middle of three 80px buttons pressed
  (grow=12), the pressed child grows to exactly 92px and each real
  neighbor shrinks to exactly 74px, with the row's own real total
  width (240px) provably unchanged before and after -- both passed on
  the first run, no bugs found in the reflow math.
- Implemented `Window.add_button_group` by building each real child
  via a direct `self.add_button(...)` call (a plain Rust method call
  within the same `impl PyWindow` block, the identical technique
  Phase 2's own `add_split_button` already established for its
  leading button), then reparenting each one under the new group
  container via `Tree::try_add_child` -- confirmed its real signature
  first (`fn try_add_child(&mut self, parent: NodeId, child: NodeId)
  -> bool`, moves an already-attached node), since `add_button` itself
  always parents fresh under `self.root` first.
- Real MD3 tokens used as flat constants, matching `add_button`/`add_
  split_button`'s own established convention: `BUTTON_GROUP_GAP`
  (8.0, the real, scraped M/L/XL "inner padding" token) and `BUTTON_
  GROUP_GROW` (12.0, a real, reasonable, honestly-stated-as-
  undocumented value -- no discrete grow token exists in the spec).
- Compiled clean on the first `cargo check`/`cargo clippy` attempt.
- Real, direct empirical script run before writing any pytest: a real
  3-button group, independent per-button clicks, and a themed
  2-button group -- all passed on the first run.
- Wrote `tests/test_button_group.py` (6 tests) and `examples/
  button_group.py` -- both checked for filename collisions first
  (none). `mypy --strict` caught one real issue in the new example: a
  nested nested handler-factory function missing its own return-type
  annotation -- fixed directly (added `Callable[[], None]`), not
  glossed over.
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean, `cargo test
  --workspace --release` clean (`engine-core` +2 new reflow-math
  tests, up from 176 to 178, unchanged elsewhere), `maturin develop
  --release` rebuilt, `pytest tests/` 549 passed/1 skipped (6 new, up
  from 543, zero regressions), all 74 examples (including the new
  `examples/button_group.py`) and the showcase demo re-run clean,
  `mypy --strict` clean against `examples/button_group.py`.
- Updated `BUILD_TRACKER.md` (Phase 3 closed; M35 itself closed, all
  3 phases; Top Metrics row at 100%; a real "Not scoped" trailer note
  added naming Loading Indicator/Time Picker Dial as the two real
  gaps this milestone's own scoping investigation found but the user
  didn't select, plus Split Button's own inner-corner shape-tightening
  and Button Group's own per-child shape change on press, both
  deliberate v1 scope cuts stated directly) -- verified the parser's
  own reported item count unchanged (only existing items' status
  flipped, no new step bullets), regenerated and republished the
  Build Tracker artifact. **This closes M35 Phase 3 and, with it, M35
  itself, all 3 phases.**
