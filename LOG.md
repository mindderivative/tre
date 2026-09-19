# LOG — M35 Phase 2: Split Button

- Real anatomy read directly from `COMPONENT_SPLIT_BUTTONS.md` before
  writing any code: leading button + trailing menu button (always
  `expand_more`), real xsmall tokens (between-space 2dp, trailing-
  button icon size 22dp), "the menu button rotates inwards 180° when
  opened and closed" (standard, non-expressive motion scheme).
- **Real, load-bearing correction, found and fixed before writing any
  Split Button code, not after:** the M35 scoping note (written last
  phase) assumed the trailing icon's rotation could reuse the existing
  `PaintProperties.transform` (`Animated<Affine>`) mechanism at zero
  new engine-core cost. Checked directly and found this false:
  `Interpolate for Affine` (`animation.rs:34-59`) is a plain
  componentwise coefficient lerp, its own doc comment already stating
  it's wrong for rotation ("would look like a non-circular morph");
  kurbo's real `Affine::svd()` (rotation-aware) is `pub(crate)`, not
  exported -- confirmed via direct source read. Corrected the earlier
  BUILD_TRACKER.md claim honestly rather than letting it stand.
- Real fix: new `IconState.rotation: Animated<f64>` -- a plain scalar
  degrees value, avoiding the whole Affine-interpolation correctness
  problem entirely (a scalar lerp is exact), mirroring the identical
  "scalar progress value drives real paint geometry" shape
  `CheckboxState.check_progress`/`RadioButtonState.select_progress`/
  `SwitchState.toggle_progress` already establish.
- `IconState` could no longer derive `Clone`/`Debug`/`PartialEq` once
  it carried an `Animated<f64>` (`Animated<T>` implements none of
  those) -- found the exact real precedent already in this codebase:
  `NodeKind`'s own doc comment states the identical fact for
  `Splitter`, confirming via grep first that nothing actually clones/
  prints/compares an `IconState` value directly before removing the
  derive.
- New `IconState::new(path, tint)` constructor. Migrated all 23 real
  `IconState { path, tint }` struct-literal sites (22 in
  `window_factory.rs`, 1 in `tree.rs`) via a Python regex script --
  22 matched cleanly on the first pass; the 23rd (`tint: Color::
  from_rgba8(r, g, b, a)`) had internal commas the regex's `[^\n,]+`
  capture group split on, silently leaving it unmatched -- caught by
  `cargo check`'s own next error, fixed by hand. The identical
  "compiler's own exhaustive error list as the final real worklist"
  technique M33 Phase 2/M34 Phase 1 already established.
- New `Tree::tick_all` arm for `IconState.rotation` (mirrors
  `select_progress`/`toggle_progress`, `tree.rs`); new `Node.
  animate("rotation", ...)` match arm (`engine-py::node.rs`),
  resolving only on `NodeKind::Icon`.
- `engine-render`'s `NodeKind::Icon` paint arm now composes a fresh
  `Affine::rotate(state.rotation.current.to_radians())` every frame,
  in local node space around the icon's own real center (`w/2, h/2`),
  before the existing viewBox-to-local `icon_transform` -- so the icon
  visually spins in place regardless of its own internal viewBox
  geometry.
- Real, decisive pixel-diff test added to `icon_paint.rs`, reusing the
  file's own existing asymmetric left-half test icon: a real 180°
  rotation paints the node's own *right* half instead of the left --
  a real, meaningful geometric proof, not "doesn't panic." Passed on
  the first run.
- Implemented `Window.add_split_button` by calling `self.add_button
  (...)` directly for the leading button (a plain Rust method call
  within the same `impl PyWindow` block -- `#[pymethods]` doesn't
  block ordinary same-crate calls), reusing all of its own already-
  verified color/token logic rather than reimplementing it. Built the
  trailing button/icon manually, mirroring `add_top_app_bar`'s own
  icon-button construction pattern, so the real icon `Node` itself
  (not a wrapper) is exposed for the app to animate directly.
- Real, deliberate design: `add_split_button` returns `(leading,
  trailing, trailing_icon)` with no wrapping container node at all --
  checked the spec's own anatomy diagram first: it lists exactly
  "Leading button, Icon, Label text, Trailing button," unlike
  `Toolbar`/`Button Group`, both of which do have a real container
  element. Design Principle 6's own "engine provides the mechanism,
  app decides the real state change" split: the engine never
  opens/closes a menu or rotates the icon on its own.
- Real, honest v1 scope limit stated directly: the inner corners' own
  real hover/press shape-tightening is not implemented -- both
  buttons paint fully rounded always, a deliberate simplification of
  MD3's own asymmetric-corner anatomy this component's real function
  doesn't strictly need.
- Compiled clean on the first `cargo check`/`cargo clippy` attempt
  after the mechanical migration (one straggler fixed by hand, above).
- Real, direct empirical script run before writing any pytest: real
  split button construction, independent leading/trailing clicks, and
  real rotation animation round-tripping 0°→180°→0° -- all passed on
  the first run.
- Wrote `tests/test_split_button.py` (6 tests) and `examples/
  split_button.py` -- both checked for filename collisions first
  (none).
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean, `cargo test
  --workspace --release` clean (`engine-render` +1 new rotation
  pixel-diff test, unchanged elsewhere -- additive only), `maturin
  develop --release` rebuilt, `pytest tests/` 543 passed/1 skipped
  (6 new, up from 537, zero regressions), all 73 examples (including
  the new `examples/split_button.py`) and the showcase demo re-run
  clean, `mypy --strict` clean against `examples/split_button.py`.
- Updated `BUILD_TRACKER.md` (Phase 2 closed; Top Metrics row updated
  to 67%/2-of-3; corrected the earlier scoping note's own "zero new
  engine-core capability" claim to state the real finding honestly)
  -- verified the parser's own reported item count unchanged (only an
  existing item's status flipped), regenerated and republished the
  Build Tracker artifact. **This closes M35 Phase 2 only -- M35 itself
  stays open, Phase 3 (Button Groups) remains.**
