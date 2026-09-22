# LOG — M52: Live Re-Theme for `Window`'s Imperative MD3 Catalog

- User: "Scope the live re-theme for Window's imperative MD3 catalog"
  -- the "live token-linkage" mechanism M51's own writeup explicitly
  named and deferred, flagged as highest-risk/highest-value since the
  very first investigation in this whole session, long before M49
  existed. Entered Plan Mode before implementing.
- Dispatched a dedicated Explore agent for an exhaustive, per-factory
  audit of all 58 `add_*` factories, mirroring M50's own precedent for
  this exact scale of investigation. Key findings: 46 of 58 factories
  resolve a theme-derived value; the file's 4 shared `resolve_*_colors`
  helpers and `ThemeState` itself are pure Rust, no GIL type anywhere;
  node topology varies from fixed to genuinely `Vec`-driven; `add_
  split_button`/`add_button_group` are structurally unusual (call
  `add_button` internally then overwrite shape-morph fields
  independently); 9 factories set theme-derived colors on their own
  `NodeKind` payload field, 5 of those confirmed completely untouched
  by `Window.set_theme` even before this milestone.
- Design: `RetitheHook = Box<dyn Fn(&ThemeState, &mut Tree)>`, a new
  `PyWindow.retheme_hooks: RefCell<Vec<RetitheHook>>` side table
  mirroring the existing `materializers`/`canvas_draws` precedent,
  replayed at the end of `Window.set_theme` alongside the two existing
  (unchanged) tint pushes -- purely additive. Each themed factory
  builds its hook via a small, named, independently-testable function.

## Phase 1 — Mechanism + `add_button` Proof of Concept

`RetitheHook` type + `retheme_hooks` field + wiring into `Window.
set_theme`, `add_button` wired. 5 new Rust unit tests. A real test-
authoring mistake caught by running the test: MD3's `on_primary` role
can legitimately resolve to the identical white for two different dark
seeds, so an initial "must differ" assertion was wrong -- fixed to
assert exact resolved values instead. 4 new pytest tests. Committed
eceedae.

## Phase 2 — Fixed/Simple `PaintProperties`-Only Factories

All 19 remaining fixed/simple factories wired: `add_card`, `add_
divider`, `add_tooltip`, `add_dialog`, `add_toolbar`, `add_search_
view`, `add_popover`, `add_link`, `add_status_bar`, `add_node_graph`,
`add_graph_node`, `add_list_item`, `add_accordion_header`, `add_tree_
node`, `add_date_picker_day`, `add_time_input_field`, `add_period_
selector`, `add_spin_box`, `add_loading_indicator`. Each factory's own
`*_retheme_hook` builder lives in one new, dedicated preamble section
(a deliberate organizational choice for this milestone's own ~45-
factory batch). A real, honest finding caught by running the tests:
the shipped `default_theme.yaml` sets both a bare `card` key and a more
specific `card.elevated` key -- a test overriding only the bare key's
elevation was silently shadowed by the untouched, more specific default
entry, real and correct 2-tier-lookup behavior, not a mechanism bug.
11 new pytest tests. Committed a1614ef.

## Phase 3 — Buttons & FAB Family

All 8 remaining factories wired: `add_icon_button`, `add_fab`, `add_
extended_fab`, `add_chip`, `add_badge`, `add_segmented_button`, then
`add_split_button`/`add_button_group` last. A real, emergent design
property discovered while wiring the two nested-reuse cases, not
anticipated in the plan's own text: both call `self.add_button(...)`
internally, and that call already registers its own plain button hook
-- the new hooks only needed to layer the additional shape-morph
geometry on top (disjoint fields), not redundantly re-resolve colors.
A real bug caught by the compiler, not shipped: an initial draft
consumed captured `Vec`s via `.into_iter()`, which `RetitheHook`'s own
`Fn` (not `FnOnce`) bound rejects outright, since a hook must be
callable on every real `set_theme()` call, not just once. 9 new
pytest tests, all passing on the first run. Committed da8b8c2.

## Phase 4 — Conditional/Variable Multi-Node Containers & Navigation

All 9 remaining factories wired: `add_snackbar`, `add_side_sheet`,
`add_navigation_drawer`, `add_top_app_bar`, `add_navigation_rail`,
`add_tabs`, `add_search_bar`, `add_pagination`, `add_menu_item`. Two
real, pre-existing inconsistencies confirmed and reproduced faithfully,
named directly in each hook's own doc comment: `add_search_bar`'s icon-
button containers use a hardcoded literal corner radius, never the
`"icon_button"` key; `add_pagination`'s prev/next icon buttons share
the `"pagination"` key instead -- a third distinct convention among
this catalog's four icon-button-shaped call sites. `add_menu_item`'s
hook correctly has nothing to do for shape/elevation (`build_menu`'s
own panel still hardcodes those, a confirmed pre-existing gap named
again, not fixed here). A real parser error caught by running the
tracker generator: an unclosed outer paren in a long, multi-factory
Step 1 note. 11 new pytest tests, all passing on the first run.
Committed 97c061c.

## Phase 5 — Non-`PaintProperties` Stateful Components

Real investigation before writing any code: direct read of `Tree::
set_all_component_tints` confirmed `add_checkbox`/`add_slider`/`add_
text_field`/`add_code_editor` need zero new Rust code -- their own
themed fields are exactly what that pre-existing, untouched mechanism
already re-tints on every `set_theme()` call. Wired the 5 factories
confirmed to be real, previously-undocumented gaps instead: `add_
radio_button` (2 roles), `add_switch` (5 roles, the most of any
component in this catalog), `add_linear_progress`, `add_circular_
progress`, `add_time_picker_dial`. 2 new exact-value Rust unit tests
(radio button's 2 roles, switch's all 5), both passing on the first
run. 2 new pytest tests. A real signature mismatch caught by running
the first draft: `add_text_field`/`add_code_editor` both require a
`background` tuple, not a `value=` keyword. Committed 8aab6fb.

## Phase 6 — Docs/Example/Verification Wrap-Up

`python/tre/_core.pyi`'s `Window.set_theme` docstring corrected -- it
still claimed "there is no live re-theming of an already-built node's
own corner radius or elevation," true before this milestone, false
after Phase 5 closed. No signature change, but a real documentation-
accuracy issue nonetheless, caught and fixed rather than left stale.
`examples/theme_customization.py` extended with a `window.set_theme
(custom_theme=...)` call after the button is already built (reusing
the M51-era `theme_customization_retheme.yaml` fixture, widened with a
real `components: {button.filled: {corner_radius: 20, elevation: 6}}`
section) -- proves the exact same already-built button's own `corner_
radius`/`elevation` change live, the `Window`-side mirror of what M51
already proved for `view.set_theme(...)`.

Full final chain green: `cargo check --workspace --all-targets`/
`cargo clippy --workspace --all-targets -- -D warnings`/`cargo fmt
--check` clean; `cargo test --workspace --release` (`engine-py` 27,
unchanged from Phase 5); `maturin develop --release`; `pytest tests/`
(747 passed, unchanged from Phase 5, 1 skipped unchanged); all 84
examples (zero new files, one extended, zero failures); `demo/
showcase.py` (all 5 phases, exit 0). Tracker generator: 52
milestones/156 phases/272 items/2 known gaps/19 fixed gaps. Artifact
republished to the existing URL.

## Status

**M52 -- Live Re-Theme for `Window`'s Imperative MD3 Catalog -- is now
fully complete, all 6 phases.** The "live token-linkage" mechanism
named and deliberately deferred since M49 is closed. All 46 in-scope
factories (of 58 total in the catalog) now re-theme their own already-
built node(s) live on `Window.set_theme(...)`, not just at construction
time -- both real MD3 color roles and shape/elevation `components:`
overrides. The `RetitheHook` mechanism proved correct across every real
topology class this catalog contains: fixed/conditional/genuinely-
variable node counts, `PaintProperties`-only and `IconState.tint`/
custom-`NodeKind`-payload-touching hooks, transform-adjacent nodes that
must never be clobbered, hooks that legitimately do less than their
siblings (pre-existing gaps named and reproduced as-is, not silently
"fixed"), per-item app-owned state captured at construction time and
never re-derived, hooks that compose correctly with an inherited hook
from an internally-reused factory, and knowing when a factory needs no
new hook at all because an existing mechanism already covers it
correctly. As a direct byproduct, 5 confirmed, previously-undocumented
gaps were closed too (`RadioButton`/`Switch`/`LinearProgress`/
`CircularProgress`/`TimePickerDial`).

This closes the "theme as a YAML file" roadmap end to end, across both
real node-creation surfaces this engine has: construction-time
resolution (M49/M50) plus genuine live re-resolution (M51 declarative,
M52 imperative). The user's original customization directive from
early this session -- "I want all properties usable and exposed for a
users use... Even changing the MD3 theme should be customizable by a
user" -- is now fully realized for theming specifically, across every
real component this catalog has.

Per this session's own standing "push after a full milestone closes"
convention, a `git push` is now appropriate -- 6 local commits across
this milestone's own 6 phases, none yet pushed.
