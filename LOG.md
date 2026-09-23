# LOG — M60: Styling API Breadth I: Border Kwargs Across the Catalog

- The first, most mechanical of the 3 genuinely separate pieces the
  "Scope the following Known Gaps" investigation found bundled in one
  "styling API breadth" bullet (see M61/M62). `add_rect`'s own existing
  `border_color`/`border_width` kwargs were the only place in the whole
  58-entry catalog a caller could set a border at all.
- Dispatched to a background agent (general-purpose, isolated git
  worktree) given its sheer mechanical repetition -- the same real
  4-line conditional-overwrite pattern applied to ~32 near-identical
  factories, no custom retheme-hook logic needed per factory (unlike
  M52's own catalog work) -- freeing the main session to work on M61's
  design-heavier token-reference-substitution milestone in parallel
  rather than idling on mechanical edits. Given the identical
  verification-chain requirements as every other milestone this
  session, plus an explicit "do NOT touch BUILD_TRACKER.md/commit
  anything" boundary so the main session stayed the one place tracker/
  commit state changes.

## What shipped (both phases)

1. `border_color`/`border_width` optional kwargs added to 32 factories
   beyond the pre-existing `add_rect` -- `add_button`, `add_icon_button`,
   `add_fab`, `add_extended_fab`, `add_segmented_button`, `add_chip`,
   `add_menu_item`, `add_badge`, `add_card`, `add_divider`,
   `add_tooltip`, `add_dialog`, `add_snackbar`, `add_side_sheet`,
   `add_navigation_rail`, `add_navigation_drawer`, `add_top_app_bar`,
   `add_toolbar`, `add_split_button`, `add_tabs`, `add_search_bar`,
   `add_search_view`, `add_list_item`, `add_accordion_header`,
   `add_tree_node`, `add_date_picker_day`, `add_period_selector`,
   `add_popover`, `add_pagination`, `add_status_bar`, `add_node_graph`,
   `add_graph_node` -- each verified by the agent against its own real
   `tree.insert(NodeKind::...)` call site, not guessed from name.
   Non-Rect factories confirmed excluded by code, not name:
   Text/Container/ScrollView/Carousel-backed factories, factories with
   their own dedicated `NodeKind` (`Checkbox`/`Slider`/`TextField`/etc.),
   `add_splitter` (technically also stroked by `paint_node`, but outside
   this milestone's own stated `NodeKind::Rect`-only scope).
   Multi-node-return handling, real judgment calls documented in code:
   `Vec<Node>` returns (`add_segmented_button`/`add_navigation_rail`/
   `add_tabs`) apply the border uniformly -- no single "first" element
   is distinguishable among peers; tuple returns (`add_pagination`/
   `add_snackbar`) apply it only to the first/primary element, per the
   plan's own stated convention (verified with a test); `add_side_
   sheet`/`add_navigation_drawer` (scrim-wraps-panel, modal vs.
   standard) apply the border to whichever real `NodeKind::Rect` insert
   actually produces the node returned to Python in each branch;
   `add_split_button` forwards the kwargs into its own internal
   `add_button` call rather than duplicating the logic.
2. 6 new pytest tests in `tests/test_live_style.py`: `add_card`
   (simple, proves the override wins over the theme-derived border),
   `add_chip`, `add_badge` (both dot and labeled shapes), `add_snackbar`
   (tuple, first-element-only), `add_tabs` (`Vec`, uniform),
   `add_pagination` (tuple, explicitly asserting `next` stays
   unbordered while `previous` isn't -- proving the convention). All use
   `.get("border_width")` readback per `test_add_rect_accepts_border_
   kwargs`'s own established pattern. `python/tre/_core.pyi` updated for
   all 32 factories, cross-checked programmatically against the real
   Rust source for an exact 1:1 match.

## Merging the agent's worktree onto `main`

The agent's own worktree branched from a commit before M57/M58/M59/M61
landed on `main` -- its own uncommitted changeset (3 files:
`window_factory.rs`/`_core.pyi`/`test_live_style.py`) was extracted as
a diff and applied onto current `main` via `git apply -3` (a real
3-way merge using the shared merge-base blob, not a blind patch). 2 real
conflicts surfaced in `window_factory.rs`, both in factories M58 had
*also* touched in the meantime:
- `add_tooltip`: M58 added real theme-aware `container_color`/
  `label_color` resolution (replacing a hardcoded `Md3Baseline::
  INVERSE_SURFACE` constant); M60 added the same function's own
  `border_color`/`border_width` kwargs. Merged by hand: kept M58's real
  `container_color` variable feeding `PaintProperties::new(...)`, and
  M60's conditional `border_color`/`border_width` overwrite on top of
  it -- neither change silently reverted the other.
- `add_pagination`: M58 unified `previous`/`next` to consult the
  `"icon_button"` theme key instead of `"pagination"` (a new
  `icon_button_corner_radius` local); M60 added `border_color`/
  `border_width` params to the shared `build_icon_button` helper and
  both call sites. Merged by hand: both call sites now pass M58's own
  `icon_button_corner_radius` (not M60's stale pre-M58 `corner_radius`)
  plus M60's new border params -- `next` correctly stays unbordered
  (`None`, `None`) per the agent's own stated "border kwargs style
  `previous` only" convention.
The full verification chain was then re-run from scratch against the
merged result, not just trusted from the agent's own pre-merge run.

- `BUILD_TRACKER.md`: full Milestone 60 section, Top Metrics row at
  100%, Just-closed/Up-next refreshed to reflect M60 closing after M61.
  Tracker regenerated (13 milestones/46 phases/114 items/2 known gaps/
  24 fixed gaps), artifact republished.
- Full chain green (post-merge): `cargo check`/`clippy -D warnings`/
  `fmt --check` clean; `cargo test --workspace --release` (227
  engine-core, 21 engine-md3, 74 engine-spec, matching the M61
  baseline exactly -- zero regressions introduced by the merge);
  `maturin develop --release`; `pytest tests/` (824 passed, up from
  818, +6, 2 skipped unchanged); every file in `examples/` ran clean;
  `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M60 is complete, both phases.** The real "which node gets the
border on a multi-node-return factory" judgment calls the agent made
are documented directly in the code and cross-checked against a real
test for the one genuinely ambiguous case (`add_pagination`'s `next`
staying unbordered). The merge onto a `main` that had advanced
underneath the agent's worktree is itself a real, worth-recording
event: background-agent work in an isolated worktree needs an explicit
review-and-merge step before it's real, not just "the agent said tests
passed." Committing locally now; push deferred pending explicit user
confirmation. Next: M62 (styling API breadth III: typography theming,
now widened via `AskUserQuestion` to include real `TextState.
line_height` plumbing, not just theme data -- see `PLAN.md`).
