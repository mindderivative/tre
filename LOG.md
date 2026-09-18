# LOG — M30 Phase 5 Step 1: Navigation Rail

- Verified real MD3 Navigation Rail tokens via WebFetch against
  `_md-comp-navigation-rail.scss`. First fetch summary misattributed
  the active-indicator's own `secondary_container` color to the
  rail's container — caught by a second, more targeted fetch asking
  for the real `container-color` line verbatim (real value:
  `surface`). Real values: 80dp width, `level0` elevation, `corner-
  none` shape; indicator `secondary_container` 56x32dp `corner-full`;
  icon 24dp; label Label Medium.
- Traced Label Medium's real numeric weights through
  `_md-sys-typescale.scss` (`label-medium-weight` = `weight-medium`,
  `-weight-prominent` = `weight-bold`) into `_md-ref-typeface.scss`
  (`weight-medium` = 500, `weight-bold` = 700) — a real, genuine
  weight difference between active/inactive labels, not just a color
  change.
- Decided architecture: plain composition (Segmented Button/Filter
  Chip's own real dividing line — group-exclusive state is app-owned),
  returns `Vec<Node>` (Segmented Button's own exact return shape).
- Implemented `add_navigation_rail` in
  `crates/engine-py/src/window_factory.rs`.
- Wrote `tests/test_navigation_rail.py` — the independent-click test
  failed on first run: clicking an item's own center landed on the
  decorative indicator pill (an interior Rect) instead of the item
  itself, since a plain Rect always independently claims a hit and
  `Tree::hit_test_at` never bubbles a hit up to an ancestor.
- Investigated `hit_test_at` directly, confirmed the real gap:
  Text/Icon already have a hardcoded per-NodeKind hit-test exemption
  (Phase 1) but a Rect can't get the same treatment globally (it's
  the real click target for Button/Card/Chip/etc). Added a new,
  minimal, purely additive `Node.hit_testable: bool` field (default
  `true` via `Tree::insert`'s single construction site) and
  `Tree::set_hit_testable`, checked in `hit_test_at` before the
  existing NodeKind match. Wired `tree.set_hit_testable(indicator,
  false)` into `add_navigation_rail`.
- Added a direct `engine-core` unit test
  (`hit_testable_false_makes_a_node_defer_to_its_own_ancestor`)
  proving the exact hit-target change before/after the flag flips —
  passed on the first run after the fix.
- Fixed a real clippy `collapsible_if` finding along the way (the
  `selected` range-check nested if).
- Re-ran `tests/test_navigation_rail.py` after the fix — all 8 passed
  (previously 1 failing).
- Added `.pyi` stub for `add_navigation_rail`.
- Wrote `examples/navigation_rail.py`, ran clean. Fixed a real mypy
  --strict finding (`make_selector` missing a return type
  annotation) before it was clean.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (engine-core 149 up from
  148), `maturin develop --release`, `pytest tests/` (335 passed, 1
  skipped), all 47 examples clean (confirming zero regressions from
  the hit_testable change across every existing hit-testable node),
  showcase demo clean.
- Updated `BUILD_TRACKER.md` (Phase 5 heading now 🚧, Step 1 closed),
  regenerated and republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
