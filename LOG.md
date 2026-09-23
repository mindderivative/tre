# LOG — M61: Styling API Breadth II: Token-Reference Substitution in `StyleSpec`

- The second of the 3 genuinely separate pieces the "Scope the
  following Known Gaps" investigation found bundled in one "styling API
  breadth" bullet (see M60/M62). `engine_md3::shape` already had real
  named shape/elevation constants; nothing let a theme YAML author
  reference them by name (`corner_radius: small`) instead of a plain
  literal number.
- Investigation's key precedent: `StyleSpec.background: Option<String>`,
  resolved by `resolve_color` (tries a theme role first, falls back to a
  literal parse) -- but `corner_radius`/`elevation` were `f32`, not
  `String`, so this needed a genuinely new `Literal(f64) | TokenRef
  (String)` type, not just new logic.

## What shipped (both phases)

1. `engine_md3::shape` gained `pub fn named(name: &str) -> Option<f64>`
   and `pub fn elevation_named(name: &str) -> Option<f64>` (exact-match
   lookups against the 6 real shape names / 6 real elevation levels,
   `None` for anything unrecognized), re-exported from `engine-md3`'s
   crate root -- 2 new unit tests, cross-checking the real MD3-published
   relative ordering and every real token name. New `ShapeOrElevation
   Spec` enum (`Literal(f64) | TokenRef(String)`, `#[serde(untagged)]`,
   the same real convention M59's own `SpacingSpec` established) with a
   `.resolve(is_elevation: bool) -> Option<f64>` method, applied to both
   `StyleSpec.corner_radius`/`elevation` (widened from `Option<f32>`)
   and `engine_spec::theme::ComponentOverride.corner_radius`/`elevation`.
   Widening away from `Copy` (`TokenRef` holds an owned `String`)
   cascaded into `cascade.rs`'s `merge()` needing `.clone_from(&...)`
   instead of a move, and every existing bare-float test literal across
   `cascade.rs`/`theme.rs`/`window_factory.rs` needing `ShapeOrElevation
   Spec::Literal(...)` wrapping -- all fixed and re-verified.
2. `engine-spec::build.rs` -- new `SpecError::UnknownShapeToken { id,
   field, token }` variant plus a new `resolve_shape_value(spec, field,
   value, default, is_elevation) -> Result<f64, SpecError>` helper,
   wired into `node_kind_and_paint`'s existing `corner_radius`/
   `elevation` resolution -- an unrecognized token name is a real,
   clear `SpecError`, never a silent fallback to `0.0`, the identical
   "fail loudly at the boundary" contract `InvalidColor` already
   established for `background`.
   `engine-py::window.rs` -- **the real design fork this phase turned
   on.** Widening `ThemeState::shape`/`elevation`'s own return type to
   fallible would have rippled into dozens of existing `theme.shape
   (...).unwrap_or(...)` call sites across `window_factory.rs`'s
   58-entry catalog, way beyond this milestone's scope. Resolved instead
   by a new `resolve_components(raw: HashMap<String, ComponentOverride>)
   -> PyResult<HashMap<String, ResolvedComponentOverride>>` function that
   resolves every token *eagerly*, once, inside `Window.set_theme` (a
   real Python `ValueError` naming the offending component key and
   field on the first unrecognized token), converting into a new
   engine-py-local `ResolvedComponentOverride` struct (plain
   `Option<f64>` fields) -- `ThemeState.components`'s own field type
   changed to store the already-resolved form, so `ThemeState::shape`/
   `elevation`'s own public signature (and every one of its call sites)
   stays completely unchanged.
- Tests: 5 new `engine-spec::build.rs` unit tests -- a real
  `corner_radius: small`/`elevation: level_3` string resolves end-to-end
  through real YAML parsing + node building to the exact same constant
  `engine_md3::named`/`elevation_named` return; an unrecognized token on
  either field produces a real `SpecError::UnknownShapeToken` naming the
  widget id, field, and bad token; a plain numeric literal still works
  unchanged. 2 new `tests/test_theme.py` pytest tests -- `Window.
  set_theme`'s own `components: {card: {corner_radius: small, elevation:
  level_2}}` resolves to the real `SHAPE_SMALL`/`ELEVATION_LEVEL_2`
  constants on a real `add_card` node; an unknown component shape token
  raises a real Python `ValueError` naming the offending component key.
  `python/tre/_core.pyi` checked, not changed -- `Window.set_theme`'s own
  Python signature is unchanged, this milestone's whole surface is
  YAML-string-level, not a new Python parameter.
- `examples/theme_customization_custom_theme.yaml` gained a new
  `components: {card: {corner_radius: small, elevation: level_2}}` entry
  alongside the pre-existing literal `button.filled` override;
  `examples/theme_customization.py` extended with a real `window.
  add_card(...)` call asserting the token resolved to `8.0`/`2.0`.
- `BUILD_TRACKER.md`: full Milestone 61 section, Top Metrics row at
  100%, the "token-reference substitution" clause moved from the
  bundled "Known gaps" styling-breadth bullet into its own "Fixed gaps"
  entry. Tracker regenerated (13 milestones/45 phases/111 items/2 known
  gaps/24 fixed gaps), artifact republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-md3` 21, up from
  19, +2; `engine-spec` 74, up from 69, +5; every other suite
  unchanged); `maturin develop --release`; `pytest tests/` (818 passed,
  up from 816, +2, 2 skipped unchanged); every file in `examples/` ran
  clean; `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M61 is complete, both phases.** The real design tension this
milestone had to resolve -- propagating a newly-fallible token type
through two structurally different real consumers (a synchronous,
per-node YAML path in `engine-spec` vs. a once-per-theme-load path in
`engine-py`) while keeping each crate's own error-handling convention
intact and avoiding any signature ripple into `window_factory.rs`'s
dozens of unrelated call sites -- is resolved and compiling cleanly
workspace-wide, with zero regression to any pre-existing test or
example. Committing locally now; push deferred pending explicit user
confirmation. Next: M62 (styling API breadth III: typography theming,
the largest and most design-heavy of the 6). M60 (border kwargs,
dispatched to a background agent in an isolated worktree) is still
running independently -- to be reviewed and merged in when it completes.
