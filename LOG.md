# LOG — M70: Rename Declarative `flex_direction`'s `Row`/`Column` to `Horizontal`/`Vertical`

- User-directed: "I want the designations row and column for flex and
  alignments to use vertical and horizontal. I know column and row are
  the standard vocabulary for MD3 but with datasheets and other excel
  types on a desktop it is confusing and can lead to misunderstanding
  during design of a UI." A real, named usability concern: "row"/
  "column" already carry an established, different meaning in
  spreadsheet/datasheet tools -- a real, common desktop-app background
  for someone designing a UI -- which happens to agree with flexbox's
  own meaning here, but only by coincidence a reader can't be expected
  to already know. Requested as a PR, not a direct push, matching a
  deliberate, reviewable change to the declarative YAML schema's own
  public surface.

## What shipped (single milestone, both phases)

1. Real investigation confirmed the full, exact scope before touching
   anything: `align_items`/`justify_content` (the "and alignments"
   half of the request) have zero row/column vocabulary anywhere --
   their real values are `Start`/`End`/`FlexStart`/`FlexEnd`/`Center`/
   `Baseline`/`Stretch`/`SpaceBetween`/`SpaceAround`/`SpaceEvenly`,
   verified directly against `AlignItemsSpec`/`JustifyContentSpec` --
   so the real, complete scope is `engine-spec`'s own
   `FlexDirectionSpec` enum alone. Confirmed this value is
   declarative-YAML-only: `Node.set_layout` (`engine-py::node.rs`, the
   imperative Python API) exposes `align_items`/`justify_content` as
   free strings but never exposed `flex_direction` at all -- no
   imperative-API surface needed touching. Confirmed every other real
   `Row`/`Column` occurrence in the codebase is `taffy::FlexDirection`
   itself, the vendored layout engine's own third-party vocabulary,
   used internally throughout `window_factory.rs`/`engine-core`/
   `engine-render`'s own tests -- correctly left untouched, not this
   crate's naming to change.
2. `FlexDirectionSpec::Row`/`Column` renamed to `::Horizontal`/
   `::Vertical` in `engine-spec/src/spec.rs`, with a real doc comment
   explaining the naming departure from `taffy`'s own vocabulary and
   the exact axis mapping (`Horizontal` -> main axis left-to-right,
   `Vertical` -> main axis top-to-bottom). A deliberate, hard rename,
   not an alias -- the old values are meant to stop parsing, matching
   this project's own "no back-compat shims for their own sake"
   discipline and the user's own clear "I want ... to use" framing.
3. `build.rs::layout_style`'s match arm updated for the renamed
   variants, with a comment noting `taffy::FlexDirection` itself stays
   `Row`/`Column` underneath, unchanged.
4. All 14 real example/demo YAML files shipping `flex_direction: Row`/
   `Column` updated (13 files under `examples/` plus
   `demo/data_panel.yaml` plus `engine-spec/examples/view.yaml`),
   along with `tests/test_component.py` (3 occurrences) and
   `docs/guide/declarative-views.md` (written earlier this session in
   M68 -- its schema table and 3 real YAML snippets, 4 occurrences
   total).
- Tests: 2 new Rust unit tests in `spec.rs` -- `flex_direction_parses_
  horizontal_and_vertical` (real, direct proof both new values parse
  to the correct enum variant) and
  `flex_direction_no_longer_accepts_the_old_row_column_naming` (real
  regression coverage proving the old `Row` value now fails to parse
  with a clear error, not silently still accepted alongside the new
  names).
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-spec` 85, up from
  83, +2; every other crate's own count unchanged); `maturin develop
  --release`; `pytest tests/` 831 passed, 2 skipped, unchanged -- every
  real example YAML re-parsed and ran clean under its new values, real
  proof the rename didn't silently break anything; `demo/showcase.py`
  all 5 phases, exit 0 (its own declarative panel uses the renamed
  value); `mkdocs build --strict` clean, 0 warnings.

## Status

**M70 is complete, both phases.** A deliberate, hard rename of the
declarative schema's `flex_direction` values, closing a real usability
gap the user named directly from their own desktop-UI-design
experience. Committed on a dedicated branch
(`flex-direction-horizontal-vertical`), not `main`, per the user's own
explicit "Add a PR" request -- opening the PR now.

Next: nothing else currently scoped beyond this PR awaiting review.
