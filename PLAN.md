# PLAN — M58: MD3 Theming Catalog Loose Ends

*(Replaces the prior M57 plan in this file — M57 is complete, committed.
Second of six milestones from the approved M57-M62 plan; see
`/home/phil/.claude/plans/reflective-sleeping-falcon.md` for the full
roadmap.)*

## Goal
Same "scope the following Known Gaps" request as M57. This gap: five
specific, confirmed-via-direct-source-read MD3 theming loose ends in
`window_factory.rs`'s 58-entry catalog, found (but not fixed) during
M52's own investigation.

## Real investigation
`add_tooltip` (color never theme-resolved, only `corner_radius`);
`build_menu`'s panel (hardcoded shape/elevation, no retheme hook at
all); `add_search_bar`'s icon-button containers (hardcoded corner
radius, not `theme.shape("icon_button", ...)`) -- all three safe/
additive. `add_time_input_field`'s `text_tint` (no `is_set()` gate,
unlike `TextField`/`CodeEditor`) and `add_pagination`'s `previous`/
`next` (shared the `"pagination"` key with page-number items instead of
`"icon_button"`) -- both real, approved visual changes, confirmed via
`AskUserQuestion`. **Real subtlety found and handled carefully:**
unifying `add_pagination`'s key needed care not to also add a new
`icon_button:` default to `default_theme.yaml` -- that would have
silently rippled into `add_top_app_bar`/`add_spin_box`/`add_search_
bar`'s own shipped appearance too, since they already consult that same
key with no override today.

## Design (4 phases)
1. Safe/additive: `add_tooltip`, `build_menu`, `add_search_bar`.
2. `add_time_input_field` color gate (approved visual change).
3. `add_pagination` key unification (approved visual change).
4. Tests, docs, verification.

## Status

**Complete, all 4 phases.**

1: `add_tooltip`'s container/label colors resolved via `theme.role
("inverse_surface")`/`role("inverse_on_surface")`, `is_set()`-gated,
mirroring `resolve_button_colors`'s established pattern, at both
construction and in `tooltip_retheme_hook` (widened to also take
`label`). `build_menu`'s panel now resolves `corner_radius`/`elevation`
via `theme.shape("menu", None)`/`theme.elevation("menu", None)`, plus a
brand-new `menu_retheme_hook` (there was none at all before) that also
re-resolves the panel's own `surface_container` color live. `add_
search_bar`'s leading/trailing icon-button containers switched from
the hardcoded `SEARCH_ICON_BUTTON_SIZE / 2.0` literal to `theme.shape
("icon_button", None).unwrap_or(...)`, matching `add_top_app_bar`/`add_
spin_box`; `search_bar_retheme_hook` widened to match.

2: `add_time_input_field`'s `text_tint` resolution wrapped in `if
theme.is_set() { ... }` -- un-themed output changes from pure black
`#000000` to `TextFieldState`'s own real default `#1C1B1F`, matching
`TextField`/`CodeEditor`. The hook itself needed no change (it only
ever runs from `set_theme`, where a theme is always set).

3: `previous`/`next` icon buttons switched to `theme.shape("icon_
button", None)`; page-number items unchanged, still `"pagination"`.
Both construction-time and `pagination_retheme_hook` now keep two
independent corner-radius values (`pagination_corner_radius`/`icon_
button_corner_radius`). `default_theme.yaml`'s existing `pagination:`
entry left with its value unchanged, just a new comment clarifying its
narrowed scope -- deliberately no new `icon_button:` override added.

4: `tests/test_theme.py` -- 2 pre-existing pagination tests updated to
reflect the deliberately-changed behavior (`previous`/`next` no longer
follow a `pagination:` override, do follow `icon_button:`); 6 new
tests covering `add_tooltip`'s color resolution (a real, non-default
seed reaching the path without raising -- no Python-facing color
getter exists anywhere in this suite, the same honest limit already
established for `add_dialog`), `build_menu`'s panel shape/elevation
(construction-time + live-retheme, both directly readable), `add_
search_bar`'s icon-button radius (construction-time + live-retheme),
and `add_pagination`'s new key behavior.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean (zero `engine-core` changes); `cargo test --workspace --release`
unchanged; `maturin develop --release`; `pytest tests/` (794 passed, up
from 789, +5, 2 skipped unchanged); all 88 examples; `demo/showcase.py`
all 5 phases, exit 0. `BUILD_TRACKER.md` updated (Top Metrics, full
Milestone 58 section, the closed gap moved to "Fixed gaps"), tracker
regenerated (13 milestones/45 phases/102 items/3 known gaps/22 fixed
gaps), artifact republished. Committing locally now.

Next: M59 (layout API breadth).
