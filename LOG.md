# Log: M30 Phase 1 Step 3 — FAB and Extended FAB

## Real MD3 data, verified rather than assumed

`FAB`'s own color-variant system was checked directly against
Material Web's real component token source before any code was
written — `tokens/versions/v0_192/_md-comp-fab-surface.scss` (the real
default: `container-color` → `surface-container-high`, `icon-color` →
`primary`, `container-shape` → `corner-large`, 56×56px, 24px icon) and
`_md-comp-fab-primary.scss` (`container-color` → `primary-container`,
`icon-color` → `on-primary-container`). Secondary/Tertiary follow the
identical `<name>-container`/`on-<name>-container` pattern MD3 uses
everywhere else in the spec (`Button`'s own Filled Tonal variant
included), not independently re-verified per variant.

`FAB`'s three real sizes each carry their own independently-specified
shape token — Small (40dp/`corner-medium` 12dp), Default (56dp/
`corner-large` 16dp), Large (96dp/`corner-extra-large` 28dp). The
three real ratios (12/40, 16/56, 28/96) are close but not identical —
a single proportional formula would have been a fabricated
approximation, not real fidelity, so `fab_shape` is a real lookup over
the three canonical sizes, matching `resolve_button_colors`'s own
"validate real vocabulary, raise `ValueError` on unknown" pattern
rather than accepting an arbitrary float.

`Extended FAB`'s real padding was also verified directly:
`fab/internal/_fab.scss`'s own real CSS comment states `padding-inline:
20px` with no icon slotted vs. `padding-inline: 16px 20px` with one —
both now real constants (`EXTENDED_FAB_LEADING_PADDING_WITH_ICON`/
`_NO_ICON`), not a single value applied to both cases.

## Anatomy

`FAB`: `Rect` container + one centered `Icon` child, `Icon Button`'s
own anatomy shape with `FAB`'s own real size/shape/color system.
`Extended FAB`: `Rect` container with real `padding`/`gap` (taffy's
own flex primitives, not manual per-child positioning) holding an
optional `Icon` child plus a `Text` label — real MD3 label-only
Extended FAB supported by making `icon` `Option<&str>`.

## Real, small refactor along the way

`add_icon`'s own curated-icon-name-to-`BezPath` lookup had already
been duplicated once (`add_icon_button`, Step 2); `add_fab`/
`add_extended_fab` needing it a third and fourth time crossed this
project's own established "2+ call sites, worth a shared helper"
threshold — factored into `resolve_icon_path`, all four real call
sites now share it, including `add_icon` itself (updated in place,
not left duplicated).

## Proactive hit-test regression coverage

`Extended FAB` is the first real composite with both an `Icon` and a
`Text` child on the same container at once — `test_fab.py`'s own
click-dispatch test for it is the real combined proof that `Tree::
hit_test_at`'s two earlier fixes (`NodeKind::Text` from Step 1,
`NodeKind::Icon` from Step 2) both correctly defer to their shared
container, not just independently.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 39 binaries, all green. `maturin develop
--release` rebuilt. `pytest tests/`: 227 passed, 1 skipped (20 new in
`test_fab.py`, zero regressions). All 33 examples and the showcase
demo re-run clean. `mypy --strict` clean against `examples/fab.py`.
