# PLAN — M70: Rename Declarative `flex_direction`'s `Row`/`Column` to `Horizontal`/`Vertical`

*(Replaces the prior M69 plan in this file — M69 is complete, pushed,
live. This is a new, unrelated request, opened as a PR per the user's
own explicit instruction rather than pushed directly to `main`.)*

## Goal
`engine-spec`'s declarative `style: {flex_direction: ...}` accepted
`Row`/`Column` — MD3/CSS-standard vocabulary, but one that collides
with the different, established meaning "row"/"column" already carry
in spreadsheet/datasheet tools, a real, common desktop-app background
for someone designing a UI. User-directed: rename to `Horizontal`/
`Vertical`, which name the same real axis with no possible ambiguity.

## Real investigation
Confirmed `align_items`/`justify_content` have zero row/column
vocabulary (`Start`/`End`/`Center`/etc. only) — the real, complete
scope is `FlexDirectionSpec` alone. Confirmed this value is
declarative-YAML-only: `Node.set_layout` (imperative Python API) never
exposed `flex_direction` at all. Confirmed every other `Row`/`Column`
in the codebase is `taffy::FlexDirection`'s own third-party vocabulary,
correctly left untouched. Full real usage sites found via repo-wide
grep: the enum + its one match arm in `engine-spec`, 14 real example/
demo YAML files, `tests/test_component.py`, and
`docs/guide/declarative-views.md`.

## Design (1 milestone, 2 phases)
1. Rename the enum, update every real call site.
2. Tests, verification, PR.

## Status

**Complete, both phases.**

`FlexDirectionSpec::Row`/`Column` renamed to `::Horizontal`/
`::Vertical` — a deliberate, hard rename, not an alias, matching both
the user's own "I want ... to use" framing and this project's
established "no back-compat shims for their own sake" discipline.
`build.rs::layout_style`'s match arm updated, with a comment noting
`taffy::FlexDirection` itself stays `Row`/`Column` underneath. All 14
real example/demo YAML files, `tests/test_component.py`, and
`docs/guide/declarative-views.md` updated to the new values.

Tests: 2 new Rust unit tests — one proving both new values parse to
the correct enum variant, one proving the old `Row` value now
correctly fails to parse (real regression coverage for the breaking
rename, not silently still accepted).

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-spec` 85, up from
83, +2; every other crate unchanged); `maturin develop --release`;
`pytest tests/` 831 passed, 2 skipped, unchanged — every real example
YAML re-parsed and ran clean under its new values; `demo/showcase.py`
all 5 phases, exit 0 (its own declarative panel uses the renamed
value); `mkdocs build --strict` clean, 0 warnings. `BUILD_TRACKER.md`
updated (Top Metrics, full Milestone 70 section, Up-next refreshed),
tracker regenerated (21 milestones/61 phases/161 items/3 known gaps/25
fixed gaps), artifact republished.

Committed on a dedicated branch (`flex-direction-horizontal-vertical`),
not `main` directly, per the user's own explicit "Add a PR" request.
Opening the PR now.

Next: nothing else currently scoped beyond this PR awaiting review.
