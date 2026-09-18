# Log: M30 Phase 2 Step 3 — Chip

## The real architectural choice: composition, not a new NodeKind

`Radio Button`/`Switch` each got a genuinely new, engine-owned,
animated `NodeKind` this same phase. Chip's Filter variant is also
stateful (selected/unselected) — the surface-level pattern-match would
be "give it a NodeKind too." Checked the real precedent this catalog
already established instead: `Segmented Button` (Phase 1's own last
step) already answered this exact question for a structurally
identical case — a group of toggleable pills whose *selected* meaning
is fundamentally group/app state, not something the engine needs to
own or coordinate. Filter Chip's own selection is the same shape.
Built as a plain composition (`Rect` + optional leading `Icon` +
`Text` + optional trailing `Icon`), the same real line `Button`/`Icon
Button`/`FAB`/`Segmented Button` all already sit on.

## No new hit-test fix needed — confirmed, not assumed

Every one of `Button`, `Icon Button`, and `Extended FAB` needed (or
proactively applied) a `Tree::hit_test_at` fix for their own `Text`/
`Icon` children eating clicks meant for the container. Chip is the
same composite shape. `tests/test_chip.py`'s own click-dispatch test
passed on the first run, confirming those two earlier fixes (Phase 1)
already cover this case completely — nothing new needed this step.

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source (`_md-comp-assist-chip.
scss`/`_md-comp-filter-chip.scss`) directly: 32dp height, `corner-
small` shape (8dp) — a real, easy-to-miss distinction from every other
component in this catalog so far (`Button`/`Icon Button`/`FAB`/
`Segmented Button` all use "Full," fully-rounded shape; Chips
genuinely don't). Icon token is 18dp, smaller than every other
component's 24dp. Assist Chip's own label role is `on_surface`,
confirmed genuinely different from Filter/Input/Suggestion's shared
`on_surface_variant` — not assumed to be the same role reused, or a
typo waiting to happen. Filter Chip's real selected state reuses
`Button`'s own Filled Tonal color pattern exactly (`secondary_
container`/`on_secondary_container`).

## Real MD3 behavior reproduced faithfully

A selected Filter Chip's real checkmark replaces any custom leading
`icon` rather than showing both — real MD3 anatomy, not an engine
simplification. `removable` (a trailing "close" icon) is independent
of `variant`, since any chip can reasonably be made removable in a
real app, not gated to the Input variant specifically.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 42 binaries, all green, unchanged count
(a pure composition needed no new engine-render capability or pixel
test). `maturin develop --release` rebuilt. `pytest tests/`: 263
passed, 1 skipped (12 new in `test_chip.py`, zero regressions). All 37
examples and the showcase demo re-run clean. `mypy --strict` clean
against `examples/chip.py`.
