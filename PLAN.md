# PLAN — M63: Typography Theming Migration: `window_factory.rs`'s Catalog

*(Replaces the prior M62 plan in this file — M62 is complete, committed
and pushed. A new, smaller follow-up task: the user pasted M62's own
"Known gaps" bullet verbatim and asked to scope it.)*

## Goal
M62 built `engine_md3::typography` (the real MD3 type scale) and
`ThemeSpec.typography` (theme-file overrides), but left two real,
named gaps open: `ThemeSpec.typography` had no resolver/consumer at
all, and `window_factory.rs`'s ~23 text-creating factories still used
their own independent per-component font constants instead of the new
centralized module.

## Real investigation
Every real `NodeKind::Text(TextState {...})`/`NodeKind::Link(TextState
{...})` construction site was enumerated directly (44 `TextState`
literals total from M62's own mechanical grep, minus `add_text`'s
free-form kwarg-driven one and 3 test-module fixtures = 27 real
factory sites across 23 factories, plus `add_link`'s own `NodeKind::
Link` site the original `Text`-only grep had missed). Each factory's
own already-shipped `(font_size, font_weight)` pair was matched
against the real 15-role type scale -- the shipped numbers themselves
treated as authoritative for role identity (independently verified
against Material Web's own tokens when first written), not re-derived
from a fresh spec reading (which would risk an unrequested visual
change). One genuine numeric tie found (`14sp/500` matches both
`title_small` and `label_large`) -- resolved via a real external
source (material-web's own `--md-sys-typescale-title-small-font`
token, confirming Tabs use `title_small`), not guessed. One real
per-state complication found and preserved: `add_navigation_rail`/
`add_navigation_drawer`'s own active-item label weight (`700.0`) has
no equivalent named MD3 role at all -- kept as its own literal
constant on top of the resolved role's base weight for the inactive
state, not folded into a role that doesn't represent it.

## Design (1 phase)
1. `ThemeState::typography` infrastructure + full catalog migration.

## Status

**Complete, single phase.**

New `ThemeState.typography: HashMap<String, engine_spec::
TypographyOverride>` field + new `ResolvedTypeStyle` struct (`font_
family` widened to an owned `String`, the identical reason `Resolved
ComponentOverride` exists) + new `ThemeState::typography(role) ->
Option<ResolvedTypeStyle>` method -- one real tier simpler than
`shape`/`elevation`'s own 2-tier lookup, since `TypographyOverride`'s
4 fields are already plain literals needing no eager token resolution.
Never needs an external `.unwrap_or(SHIPPED_CONST)` at any call site,
unlike `shape`/`elevation` -- `engine_md3::type_style_named` already
*is* the real un-themed default. `Window.set_theme` widened to
populate it from `ThemeSpec.typography` (default then custom, custom
wins), the identical merge `components:` already establishes.

All 23 factories migrated to their own number-matched role: `label_
large` (`add_button`, `add_extended_fab`, `add_segmented_button`,
`add_chip`, `add_menu_item`, `add_snackbar`'s action, `add_list_item`'s
headline, `add_accordion_header`, `add_tree_node`, `add_period_
selector`, `add_pagination`, `add_navigation_drawer`'s base);
`label_medium` (`add_navigation_rail`'s base); `label_small`
(`add_badge`, `add_status_bar`); `body_small` (`add_tooltip`);
`body_medium` (`add_dialog`'s body, `add_snackbar`'s message, `add_
list_item`'s supporting, `add_popover`'s body); `body_large` (`add_
date_picker_day`, `add_link`); `headline_small` (`add_dialog`'s
headline); `title_large` (`add_top_app_bar`); `title_small` (`add_
tabs`, `add_graph_node`, `add_popover`'s subhead). Every layout height/
centering calc that referenced the old literal directly was updated to
reference the resolved role's own `font_size` too, so a themed size
change reflows layout correctly, not just glyphs. Dead constants
removed once every real (non-test) reference migrated away
(`TOOLTIP_FONT_SIZE`/`_WEIGHT`, `DIALOG_HEADLINE_FONT_SIZE`/`_WEIGHT`,
`DIALOG_BODY_FONT_SIZE`/`_WEIGHT`, `TOP_APP_BAR_HEADLINE_FONT_SIZE`/
`_WEIGHT`, `TAB_LABEL_FONT_SIZE`/`_WEIGHT`, `POPOVER_SUBHEAD_FONT_
SIZE`/`_WEIGHT`, `LINK_FONT_SIZE`/`_WEIGHT`, `BADGE_LABEL_FONT_SIZE`/
`_WEIGHT`, `NAV_RAIL_LABEL_FONT_SIZE`/`_WEIGHT_INACTIVE`); `BUTTON_
LABEL_FONT_WEIGHT` gated `#[cfg(test)]` (3 pre-existing retheme-hook
tests still construct a real fixture `TextState` with it, unrelated to
typography theming); `BUTTON_LABEL_FONT_SIZE` stays ungated, still
used by `add_spin_box`'s own `TextFieldState` (out of scope).

Tests: 3 new `engine-py::window.rs` unit tests (no-override returns
the real shipped default; unrecognized role returns `None`; a real
per-field override wins for set fields, shipped default for the rest
-- the actual proof `ThemeSpec.typography` now has a real consumer).
2 new `tests/test_theme.py` pytest tests (a real `typography:` custom-
theme override reaches `add_button` without raising; an override keyed
by an unrecognized role is confirmed inert, not an error, matching
`components:`'s own real non-validating behavior). `examples/theme_
customization_custom_theme.yaml`/`theme_customization.py` extended
with a real `typography: {label_large: {font_size: 15, font_family:
Roboto}}` override reaching the same already-demonstrated `add_button`.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-py` 30, up from 27,
+3; every other crate unchanged); `maturin develop --release`;
`pytest tests/` 831 passed, up from 829 (M62's own closing baseline)
-- **the 829 un-themed baseline matched exactly before either new
typography test was added, the direct proof this migration changed
zero existing visual/numeric output**; every example ran clean;
`demo/showcase.py` all 5 phases, exit 0. `BUILD_TRACKER.md` updated
(Top Metrics, full Milestone 63 section, the M62-authored "Known
gaps" bullet's two remaining clauses closed and replaced with the two
genuinely narrower ones -- non-text-creating factories out of scope by
definition, 6 unused type-scale roles awaiting a real future consumer),
tracker regenerated (14 milestones/47 phases/119 items/3 known gaps/25
fixed gaps), artifact republished. Committing locally now.

Next: nothing currently scoped.
