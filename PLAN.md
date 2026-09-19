# PLAN — M35 Phase 2: Split Button

## Goal
Continue M35 (per Phase 1's own closure) with Phase 2: a real MD3
Split Button -- a leading button plus a separate trailing menu-icon
button that rotates 180° when its own menu opens/closes.

## Steps
1. Real anatomy verified from the local MD3 spec mirror
   (`COMPONENT_SPLIT_BUTTONS.md`) before writing any code: leading
   button + trailing menu button (always `expand_more`), real xsmall
   tokens (`between-space` 2dp, `trailing-button.icon.size` 22dp),
   "the menu button rotates inwards 180° when opened and closed."
2. **Real correction found before writing any code, not after:**
   checked whether the existing `PaintProperties.transform`
   (`Animated<Affine>`) mechanism could drive the rotation, per the
   original M35 scoping note's own assumption -- found `Interpolate
   for Affine` is a plain componentwise lerp, explicitly documented as
   wrong for rotation; kurbo's real `Affine::svd()` is `pub(crate)`,
   not exported. Corrected the earlier BUILD_TRACKER.md claim rather
   than silently building against it.
3. Real fix: new `IconState.rotation: Animated<f64>` field -- a
   scalar, mirroring `CheckboxState.check_progress`/`RadioButtonState.
   select_progress`/`SwitchState.toggle_progress`'s own exact shape.
   `IconState` lost its `#[derive(Clone, Debug, PartialEq)]`
   (`Animated<T>` has none), mirroring `NodeKind`'s own identical
   precedent for `Splitter`; confirmed via grep nothing actually
   clones/prints/compares an `IconState` directly.
4. New `IconState::new(path, tint)` constructor; migrated all 23 real
   struct-literal construction sites (22 `window_factory.rs`, 1
   `tree.rs`) via a scripted mechanical patch + `cargo check`'s own
   error list as the final worklist (the M33P2/M34P1 technique).
5. New `Tree::tick_all` arm for `IconState.rotation` (mirrors
   `select_progress`/`toggle_progress`); new `Node.animate("rotation",
   ...)` arm in `engine-py::node.rs`, resolving only on `Icon`.
6. `engine-render`'s `NodeKind::Icon` paint arm composes a fresh
   `Affine::rotate` from `state.rotation.current` every frame, in
   local node space around the icon's own center, before the existing
   viewBox-to-local `icon_transform`.
7. Real, decisive pixel-diff test (`icon_paint.rs`): a 180° rotation
   paints the node's own right half instead of left (reusing the
   existing asymmetric test icon) -- passed on the first run.
8. `Window.add_split_button(label, width, height, variant, x, y) ->
   (leading, trailing, trailing_icon)` -- `leading` reuses `self.
   add_button(...)` directly (a plain Rust call within the same impl
   block); `trailing`/`trailing_icon` built manually (mirrors `add_
   top_app_bar`'s own icon-button construction) so the real icon node
   is exposed for the app to animate directly. No wrapping container
   -- real MD3 anatomy has none for Split Button.
9. Real, honest v1 scope limit: inner-corner hover/press shape-
   tightening not implemented -- both buttons paint fully rounded
   always, a deliberate simplification stated directly, not glossed
   over.
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, real empirical script before pytest, `tests/
    test_split_button.py` (6 tests), `examples/split_button.py`, full
    pytest suite, all examples, showcase demo, mypy --strict.
11. `BUILD_TRACKER.md` (Phase 2 closed, earlier "zero new capability"
    claim corrected honestly), artifact republish, memory update,
    commit (holding push -- Phase 3 remains).

## Status
Phase 2 complete. Full verification chain green (`pytest tests/` 543
passed/1 skipped, 6 new, zero regressions; all 73 examples + showcase
demo clean; mypy --strict clean). **M35 is not yet closed -- Phase 3
(Button Groups) remains.**
