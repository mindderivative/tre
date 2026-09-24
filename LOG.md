# LOG — M74: Declarative `kind: Icon`

- User-directed via `AskUserQuestion`, while scoping Tesserae's own
  component-fragment catalog: roughly two-thirds of the real MD3
  catalog was blocked from being expressible as a fragment at all,
  confirmed directly (`kind: Icon` failed with `unknown variant
  "Icon"`) -- `engine-spec`'s `NodeKindSpec` supported only 7 of
  `engine-core`'s real 21 primitive kinds. Chose to fix this at the
  source rather than scope fragment work down to the ~10 icon-free
  widgets.

## What shipped

1. `spec.rs` -- `NodeKindSpec::Icon`; `WidgetSpec.icon: Option<
   IconSpec>`; new `IconSpec { name: String }`, mirroring `ImageSpec`'s
   own shape (simpler -- no `fit:` concept for a glyph).
2. `build.rs` -- new `SpecError::UnknownIcon { id, name }`; the real
   `NodeKindSpec::Icon` match arm, resolving `icon.name` against
   `engine_md3::icons::path_for` (the identical vocabulary `Window.
   add_icon` already uses imperatively, confirmed by direct read
   before mirroring it), parsing the real curated SVG path data,
   resolving the glyph's tint via the existing `required_background`
   helper -- reusing `style.background`, the same precedent `kind:
   Text` already established, not a new, parallel color field.
3. One real, unrelated compile break fixed: `cascade.rs`'s own
   `#[cfg(test)]`-only `WidgetSpec` fixture literal needed the new
   `icon: None` field.
4. 4 new Rust unit tests (`build.rs`). Real bug caught and fixed while
   writing them, not shipped: the test YAML's own `"#1C1B1FFF"` hex
   color collided with a single-hash `r#"..."#` raw string delimiter
   (`expected ';', found '1C1B1FFF'`) -- fixed with `r##"..."##`, the
   identical real fix this same file's own pre-existing `VIEW` test
   constant already needed for the same reason.
5. 4 new pytest tests (`tests/test_declarative_icon.py`, new file) --
   the real Python binding surface end to end.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-spec` 89, up from
  85, +4; every other crate unchanged); `maturin develop --release`;
  `pytest tests/` 862 passed, 2 skipped, up from 858, +4;
  `examples/animate_rect.py` (the CI-representative smoke example) and
  `demo/showcase.py` both ran clean, exit 0.

## Status

**M74 is complete, both phases.** Declarative `kind: Icon` is real,
tested, and closes the single largest real blocker to Tesserae's own
component-fragment catalog work. Committed locally on the `0.3.1`
branch, not `main`; push deferred pending explicit user confirmation,
per standing policy.

Real, deliberate scope boundary: only `Icon` was added this milestone
-- `RadioButton`/`Switch`/`CircularProgress`/`LinearProgress`/
`LoadingIndicator`/`Link` and the rest of the real-but-declaratively-
unreachable `NodeKind` variants stay real, un-scoped future candidates.

Next: reinstall into `tesserae/.venv`, then resume item 1 of the
user's own 3-item ordering -- the real component-fragment catalog.
