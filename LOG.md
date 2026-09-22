# LOG — M53: Real Context Menus + Clipboard for `TextField`/`CodeEditor`

- User: "Scope the click/hover/context menu handlers. There is a real
  need for context menus and handlers in the CodeEditor and TextField."
  Dispatched a dedicated Explore agent for an exhaustive read of the
  dispatch chain, `node.rs`, `window_factory.rs`, `window_input.rs`,
  `app.rs`, and every example, before designing anything.
- **Real, scope-reshaping finding:** every generic interaction primitive
  (`Node.set_on_click`/`set_on_hover_enter`/`set_on_hover_exit`/
  `enable_interaction`/`set_context_menu`) already works on `TextField`/
  `CodeEditor` with zero `NodeKind` exclusion anywhere -- confirmed via
  direct read, not assumed. A real right-click on any node already
  opens whatever context menu was attached, live, in the real winit
  event loop -- confirmed via direct read of `Tree::dispatch`'s
  `PointerReleased` → `SecondaryActivated` → `open_context_menu` →
  `App::run`'s own unconditional call to it on every real input event.
  There is no "handlers don't work on text fields" problem.
- **The real, concrete gaps, each confirmed by direct source read:**
  (1) right-click never focused a `TextField`/`Terminal` -- a context
  menu opened by right-click would act on whatever was last *left*-
  clicked, not the field just right-clicked; (2) there is no real,
  callable path to the actual OS clipboard at all outside `App::run`'s
  own raw winit key-event handling -- `Window.copy`/`cut`/`paste` are
  explicitly, deliberately hermetic, confirmed via their own doc
  comments, never touching the real OS clipboard; (3) no `select_all`
  exists anywhere, Rust or Python, confirmed via grep; (4) no factory
  in the 56-entry catalog wires a context menu at construction,
  confirmed zero call sites; (5) `add_code_editor` adds nothing
  interaction-wise beyond plain `TextField`; (6) neither factory calls
  `enable_interaction()`, so click-to-focus already happens but
  produces no visible focus ring/hover at all -- deliberately not
  proposed as an automatic factory-level change, since every other
  composite `add_*` in this catalog deliberately does not auto-call it.
- Entered Plan Mode with this real, grounded scope before implementing.

## Phase 1 — `engine-core`: Right-Click Focus + `select_all`

- Widened the click-to-focus gate at `tree.rs:3615` from `button ==
  PointerButton::Primary` to `matches!(button, PointerButton::Primary |
  PointerButton::Secondary)` -- a real, minimal fix reusing `set_focus_
  to` verbatim, not a second mechanism. Applies to both `TextField` and
  `Terminal` (the existing gate's own two `NodeKind`s) -- a real
  terminal context menu benefits identically and for the same reason;
  excluding it would need more code, not less, for no real benefit.
- New `Tree::select_all_text_field(&mut self, field: NodeId) -> bool`,
  beside `set_text_field_cursor`/`extend_text_field_selection` --
  `selection_anchor = Some(0)`, `cursor = content.len()`, matching
  every real desktop text field's own Ctrl+A convention (cursor lands
  at the end, not the start). A genuinely new primitive, not
  composable from Python today -- `Node.get` has no way to read a
  field's own content length, so an app cannot build "select all"
  itself from the existing exposed pieces alone.
- 5 new Rust unit tests, all GIL-free, all passing on the first run:
  a right-click on a `TextField` now moves real focus there (mirroring
  the existing Primary-click test byte-for-byte); `select_all_text_
  field` selects from real start to real end on ordinary content, on
  empty content (a real, deliberately-checked edge case), and on
  multi-byte UTF-8 content (confirms the cursor lands at the genuine
  byte length, not a truncated/miscounted one); a non-`TextField`
  target is a true no-op, matching every sibling method's own
  established contract.
- 1 new pytest test (`tests/test_text_field.py`): `Window.right_click
  (field)` now moves real focus there too, mirroring the existing
  `click`-focuses test.
- `BUILD_TRACKER.md`: new M53 milestone section, marked 🚧 in progress
  (Phase 1 of 3), Top Metrics row, "In progress" note. Regenerated
  cleanly on the first attempt.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (`engine-core` 224, up from 219,
  +5; every other suite unchanged), `maturin develop --release`,
  `pytest tests/` (748 passed, up from 747, +1, 1 skipped unchanged),
  all 84 examples (zero new files, zero failures -- confirms nothing
  in the existing catalog depended on right-click *not* focusing a
  field), showcase demo. Tracker generator: 53 milestones/157
  phases/274 items/2 known gaps/19 fixed gaps.

## Status

**M53 Phase 1 of 3 is complete.** The real `engine-core` mechanism is
proven: right-click now focuses a `TextField`/`Terminal` before opening
its context menu, and `select_all` is a real, genuinely new primitive.
Per this session's own standing discipline, committing locally now --
push deferred until the full milestone closes. Up next: Phase 2, the
real OS clipboard API (`engine-py`) -- refactor `app.rs`'s inline
Copy/Cut/Paste logic into shared helpers, reused by three new `Window`
pymethods plus `Window.select_all()`.
