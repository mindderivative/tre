# LOG — M52: Live Re-Theme for `Window`'s Imperative MD3 Catalog

- User: "Scope the live re-theme for Window's imperative MD3 catalog"
  -- the "live token-linkage" mechanism M51's own writeup explicitly
  named and deferred, flagged as highest-risk/highest-value since the
  very first investigation in this whole session, long before M49
  existed. Entered Plan Mode before implementing.
- Dispatched a dedicated Explore agent for an exhaustive, per-factory
  audit of all 58 `add_*` factories in `window_factory.rs`/`window_
  virtual_canvas.rs`, mirroring M50's own precedent for this exact
  scale of investigation. Key findings: 46 of 58 factories resolve a
  theme-derived value; the file's 4 shared `resolve_*_colors` helpers
  and `ThemeState` itself are confirmed pure Rust, no GIL type anywhere
  -- meaning a retheme mechanism could be built as plain Rust closures,
  no `Py<PyAny>`/GC obligation needed at all. Node topology varies
  real: 30 fixed-count, 8 conditional-but-bounded, 8 genuinely `Vec`-
  driven. `add_split_button`/`add_button_group` are structurally
  unusual -- both call `self.add_button(...)` internally then overwrite
  shape-morph fields via a second, independent theme lookup. **A
  materially larger gap than this milestone's own naming implied,
  found by the audit:** 9 factories set theme-derived colors on their
  own `NodeKind` payload field rather than `PaintProperties`; 5 of
  those (RadioButton/Switch/LinearProgress/CircularProgress/
  TimePickerDial) are confirmed completely untouched by `Window.
  set_theme` today, not covered by even the existing narrow tint push.
  Two confirmed pre-existing gaps (`add_tooltip`'s hardcoded color,
  `build_menu`'s hardcoded panel shape/elevation) named but deliberately
  left unfixed -- this milestone makes already-resolved values live,
  not newly resolved.
- Also confirmed directly: `Animated::new(value)` snaps `current` and
  clears any in-flight animation -- identical to how `patch_node` (M51)
  already overwrites a re-themed node's whole `PaintProperties`. This
  milestone's own retheme hooks use the same "snap, don't ease"
  convention for full consistency across the declarative/imperative
  split.
- Design: `RetitheHook = Box<dyn Fn(&ThemeState, &mut Tree)>`, a new
  `PyWindow.retheme_hooks: RefCell<Vec<RetitheHook>>` side table
  mirroring the existing `materializers`/`canvas_draws` precedent
  exactly, but as plain Rust closures (no GC obligation, unlike those
  two). Each themed factory builds its hook via a small, named,
  independently-testable builder function (not an inline closure) --
  this is what makes exact-`Color`/`f64`-value Rust unit tests possible
  for the *internal* correctness of each hook, closing the "no Python
  getter for color" limitation this suite has worked around with "does
  not raise" since M44, for at least the mechanism's own core logic.
- Phase 1 (mechanism + `add_button` proof of concept, approved plan):
  - `RetitheHook` type + `retheme_hooks` field, initialized fresh-empty
    in both `PyWindow::new` and `from_view` (deliberately not shared
    with a `View`'s own theme -- a `View`-built tree never calls an
    `add_*` factory, so there's nothing to inherit).
  - Wired into `Window.set_theme`: hooks replay right after the two
    existing tint pushes, using the same freshly-updated `ThemeState`.
    **Deliberate decision: `set_all_interaction_tints`/`set_all_
    component_tints` stay exactly as they are, untouched** -- purely
    additive, zero regression risk, still the only mechanism for a
    plain node that opted into `InteractionState` directly.
  - `button_retheme_hook(container, label, variant, height) ->
    RetitheHook` in `window_factory.rs`, wired into `add_button`.
- A real, honest test-authoring mistake caught by actually running the
  test, not by inspection: an initial `button_retheme_hook` unit test
  asserted the label color must *differ* between two different theme
  seeds -- it failed, because MD3's tone-mapping can legitimately
  resolve `on_primary` to the identical white for two different hues
  that are both dark enough. Fixed to assert the exact resolved value
  against each theme's own real `role(...)` call instead of assuming
  inequality -- a more correct, more precise test than the original.
- 5 new Rust unit tests (GIL-free): 2 for the generic `RetitheHook`
  mechanism (`window::tests`), 3 for `button_retheme_hook` itself
  (`window_factory::tests`) -- exact color values across two themes,
  a `components:` override changing corner_radius live and resetting
  when omitted, and a stale-node safe-no-op case. 4 new pytest tests
  (`tests/test_theme.py`): a `components:` override live on an
  already-built button; reset-to-default on a later call with no
  override; a color-only change does not raise; a removed button's
  hook is a safe no-op.
- `BUILD_TRACKER.md`: new M52 milestone section, marked 🚧 in progress
  (Phase 1 of 6), Top Metrics row, "In progress" note replacing the
  prior "Up next" pointer. **A real parser error caught immediately by
  running the generator, not shipped:** the new step bullets used ASCII
  `--` instead of the required em-dash `—` right before the status icon
  -- the tracker's own `ITEM_RE` needs the literal em-dash there (`--`
  is fine everywhere else in a note's own prose, an established
  convention already used throughout this file). Fixed, regenerated
  cleanly on the second attempt.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (`engine-py` 25, up from 20, +5;
  every other suite unchanged), `maturin develop --release`, `pytest
  tests/` (713 passed, up from 709, +4, 1 skipped unchanged), all 84
  examples (zero new files, zero failures), showcase demo. Tracker
  generator: 52 milestones/151 phases/265 items/2 known gaps/19 fixed
  gaps. Artifact republished to the existing URL.

## Phase 2 — Fixed/Simple `PaintProperties`-Only Factories

- Wired all 19 remaining factories from Phase 2's own scope: `add_
  card`, `add_divider`, `add_tooltip`, `add_dialog`, `add_toolbar`,
  `add_search_view`, `add_popover`, `add_link`, `add_status_bar`,
  `add_node_graph`, `add_graph_node`, `add_list_item`, `add_accordion_
  header`, `add_tree_node`, `add_date_picker_day`, `add_time_input_
  field`, `add_period_selector`, `add_spin_box`, `add_loading_
  indicator`. Each factory's own `*_retheme_hook` builder function
  lives in one new, dedicated preamble section in `window_factory.rs`
  (a deliberate, purely organizational choice for this milestone's own
  ~45-factory batch -- distinct from Phase 1's own `resolve_button_
  colors`-adjacent placement, which would have meant jumping to each
  factory's own scattered location in a 7,900-line file for every one
  of the remaining ~45).
- Several real, non-trivial shapes handled correctly, not glossed
  over: `add_toolbar`'s hook reproduces conditional *default*
  fallbacks (branch on `is_floating`/`vertical`/the caller's own
  original `width`/`height`), not fixed constants; `add_date_picker_
  day`'s hook reproduces the exact same 4-outcome `selected`/`today`/
  `outside_month` branch the factory itself resolves; `add_list_item`/
  `add_spin_box` are this milestone's first `IconState.tint`-touching
  hooks (a plain, non-`Animated` field, written directly); `add_
  accordion_header`/`add_tree_node`'s hooks are careful never to touch
  their own chevron's `paint.transform` (the real, currently-set
  expand/collapse flip state, unrelated to theming); `add_tooltip`'s
  hook correctly does *less* than its siblings (only `corner_radius` --
  its color is a real, confirmed, pre-existing M49-era gap, deliberately
  left unfixed, named directly in the hook's own doc comment, not
  silently "completed" as part of this milestone's narrower scope).
  `add_time_input_field`'s hook reproduces a real, pre-existing minor
  inconsistency as-is (`text_tint` reads `on_surface()` unconditionally,
  no `is_set()` gate, unlike `add_text_field`/`add_code_editor`) rather
  than silently fixing it.
- `add_list_item`/`add_accordion_header`/`add_tree_node`/`add_period_
  selector`/`add_spin_box` each needed a small, real refactor first:
  their own construction code built intermediate `NodeId`s inside
  `if`/`else` branches or closures without retaining them in an
  outer-scope binding the hook-wiring code could later capture -- e.g.
  `add_period_selector`'s own `build_option` closure returned only the
  option's `NodeId`, discarding its label's; widened to return
  `(NodeId, NodeId)`.
- 11 new pytest tests extending each factory's own M50-era construction-
  time fixture with a second `window.set_theme(...)` call, proving the
  same value changes live on the same already-built `Node`, plus one
  combined "does not raise" test across every remaining color-only
  factory with no Python-readable shape/elevation field at all.
- **A real, honest finding caught by running the tests, not assumed:**
  an initial `add_card` test overrode only the bare `"card"` key's own
  `elevation`, which never took effect. The shipped `default_theme.
  yaml` already sets `card.elevated: {elevation: 1}` as a separate,
  more specific key -- the 2-tier lookup always checks the variant-
  specific key first, regardless of which theme layer originally set
  it, so a bare-key override for a field a more specific key already
  defines can never reach that variant. Fixed by overriding `"card.
  elevated"` explicitly, matching this suite's own established M50
  test convention -- this is real, correct, load-bearing behavior of
  the 2-tier lookup itself, not a bug in the new retheme mechanism.
- `BUILD_TRACKER.md`: Phase 2 section added, Top Metrics row updated
  (33%, Phase 2 of 6), "In progress" note updated. Regenerated cleanly.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (`engine-py` 25, unchanged from
  Phase 1 -- Phase 2 needed no new Rust-level tests, relying on Phase
  1's own generic-mechanism coverage plus pytest for these factories'
  integration proof), `maturin develop --release`, `pytest tests/`
  (725 passed, up from 713, +12, 1 skipped unchanged), all 84 examples,
  showcase demo. Tracker generator: 52 milestones/152 phases/266
  items/2 known gaps/19 fixed gaps.

## Phase 3 — Buttons & FAB Family

- Wired the 8 remaining Buttons & FAB factories: `add_icon_button`,
  `add_fab`, `add_extended_fab`, `add_chip`, `add_badge`, `add_
  segmented_button`, then `add_split_button`/`add_button_group` last,
  with the extra care the earlier audit flagged.
- `add_segmented_button` needed the most structural care of the
  "single-factory" cases: captured each segment's own `(segment,
  label, check, is_selected)` tuple into a `Vec` at construction time
  -- `is_selected` is real, app-owned state (Design Principle 6),
  never re-derived by the hook -- and reproduced `corner_radii_
  override`'s own first/last branching using the segment's *index
  within that captured Vec*, matching the factory's own real logic
  exactly.
- **A real, emergent design property discovered while wiring `add_
  split_button`/`add_button_group`, not anticipated in the plan's own
  text:** both factories call `self.add_button(...)` internally to
  build their leading/child buttons -- that internal call *already*
  registers its own plain `button_retheme_hook` for each one, correctly
  re-resolving background/corner_radius/elevation/border live, for
  free. The new `split_button_retheme_hook`/`button_group_retheme_hook`
  therefore only needed to layer the *additional* shape-morph geometry
  on top (`paint.shape`/`interactive_shape` for split button, `paint.
  shape`/`press_interactive_shape` for button group -- disjoint fields
  the plain button hook never touches), not redundantly re-run color
  resolution a second time. Two hooks firing for the same `NodeId`,
  writing disjoint fields, turned out to be a real, deliberate
  consequence of the hook-vec design itself -- composability that
  wasn't explicitly designed in, but fell out correctly because each
  hook only ever writes the fields it's responsible for.
- **A real bug caught by the compiler, not shipped, not even reaching
  a test run:** the first draft of `segmented_button_retheme_hook`/
  `button_group_retheme_hook` iterated their own captured `Vec<NodeId>`/
  `Vec<(NodeId, NodeId, Option<NodeId>, bool)>` via `.into_iter()` --
  but `RetitheHook = Box<dyn Fn(&ThemeState, &mut Tree)>` is `Fn`, not
  `FnOnce`, and must be callable multiple times (once per real
  `set_theme()` call). Consuming the captured `Vec` on its first
  invocation would have made a *second* `set_theme()` call panic --
  `cargo check` refused to compile it at all, catching the bug before
  any test could even run. Fixed by iterating over `&segments`/
  `&dividers` (by reference) instead.
- 9 new pytest tests extending each factory's own M50-era construction-
  time fixture with a second `window.set_theme(...)` call -- including
  two that directly prove the emergent split-button/button-group
  composition property (the leading button's/each child's own
  *inherited* plain-button retheme survives and applies correctly after
  a second `set_theme()` call, read back via `Node.get`) and one
  combined "does not raise" test for the two tightened-shape overrides
  (no Python-facing readback for `interactive_shape`/`press_
  interactive_shape`, matching this suite's own established limit).
  All 9 passed on the first run.
- `BUILD_TRACKER.md`: Phase 3 section added, Top Metrics row updated
  (50%, Phase 3 of 6), "In progress" note updated. Regenerated cleanly.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (unchanged -- no new Rust-level
  tests needed this phase either), `maturin develop --release`,
  `pytest tests/` (734 passed, up from 725, +9, 1 skipped unchanged),
  all 84 examples, showcase demo. Tracker generator: 52 milestones/153
  phases/267 items/2 known gaps/19 fixed gaps.

## Status

**M52 Phases 1-3 of 6 are complete.** The mechanism has now been proven
across every real topology class the original audit identified except
the genuinely variable/`Vec`-driven multi-node case (Phase 4) and the
non-`PaintProperties` stateful components (Phase 5) -- fixed/
conditional node counts, `PaintProperties`-only and `IconState.tint`-
touching hooks, transform-adjacent nodes, a hook that legitimately does
less than its siblings, per-index-branching state captured at
construction time, and (the real surprise of this phase) hooks that
compose correctly with an *inherited* hook from an internally-reused
factory. Per this session's own standing discipline, committing locally
at this phase boundary too -- push still deferred until the full
milestone closes. Up next: Phase 4, the ~9 conditional/variable multi-
node Containers & Navigation factories (`add_snackbar`, `add_side_
sheet`, `add_navigation_drawer`, `add_top_app_bar`, `add_navigation_
rail`, `add_tabs`, `add_search_bar`, `add_pagination`, `add_menu_item`).
