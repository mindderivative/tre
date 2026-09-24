# PLAN — M74: Declarative `kind: Icon`

*(Replaces the prior M73 plan in this file — M73 is complete, committed.
User-directed via `AskUserQuestion`, while scoping Tesserae's own
component-fragment catalog.)*

## Goal

`engine-spec`'s `NodeKindSpec` supported only 7 of `engine-core`'s real
21 primitive `NodeKind` variants — confirmed directly, `kind: Icon`
failed with `unknown variant "Icon"`. This blocked roughly two-thirds
of the real MD3 catalog from being expressible as a Tesserae component
fragment at all (any composition with an icon glyph). Add declarative
`kind: Icon` support, mirroring `kind: Image`'s own real, already-
shipped precedent (M22 Phase 2).

## Status

**Complete, both phases.**

New `IconSpec { name: String }` sibling block (`spec.rs`), simpler than
`ImageSpec` (no `fit:` concept). `NodeKindSpec` gained `Icon`;
`WidgetSpec` gained `icon: Option<IconSpec>`. New `SpecError::
UnknownIcon`. `build.rs`'s new match arm resolves `icon.name` via
`engine_md3::icons::path_for` (the identical vocabulary `Window.
add_icon` already uses imperatively), parses the real curated SVG data,
resolves the glyph's tint via the existing `required_background`
helper — reusing `style.background`, the same precedent `kind: Text`
already established, not a new field.

4 new Rust unit tests (`build.rs`) — a real bug caught while writing
them: the test YAML's own hex color collided with a single-hash raw
string delimiter, fixed with `r##"..."##` (the identical fix this same
file's own pre-existing `VIEW` test constant already needed). 4 new
pytest tests (`tests/test_declarative_icon.py`, new file).

Full verification chain: `cargo check`/`clippy -D warnings`/
`fmt --check` clean; `cargo test --workspace --release` (`engine-spec`
89, up from 85, +4; every other crate unchanged); `maturin develop
--release`; `pytest tests/` 862 passed, 2 skipped, up from 858, +4;
`examples/animate_rect.py` (CI-representative smoke example) and
`demo/showcase.py` both ran clean, exit 0. `BUILD_TRACKER.md` updated,
tracker artifact regenerated (25 milestones/71 phases/194 items/3
known gaps/25 fixed gaps) and republished. Committing locally on the
`0.3.1` branch now.

Real, deliberate scope boundary: only `Icon` was added — `RadioButton`/
`Switch`/`CircularProgress`/`LinearProgress`/`LoadingIndicator`/`Link`
and the rest stay real, un-scoped future candidates, added only if/when
a real Tesserae fragment genuinely needs one.

Next: reinstall into `tesserae/.venv`, then resume item 1 of the
user's own 3-item ordering — the real component-fragment catalog, now
that icon-using widgets are actually buildable.
