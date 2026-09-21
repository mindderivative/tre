# LOG — M51: Live Re-Theme for the Declarative Surface (`View.set_theme`)

- User: "Scope M51" -- following M50's own writeup renumbering this as
  the next roadmap item once color (M49) and shape/elevation (M50)
  overrides both reached the whole real MD3 catalog. Entered Plan Mode
  before implementing.
- Real investigation, not assumed: re-read `Reconciler`
  (`engine-spec/src/reconcile.rs`) and `patch_node`
  (`engine-spec/src/build.rs`) directly. `Reconciler` already retains
  the full parsed `WidgetSpec` (`self.spec`) and every widget `id`'s
  `NodeId` (`self.ids`) for a `View`'s entire lifetime, purely to
  support hot-reload. `patch_node` already does exactly "recompute this
  node's `PaintProperties`/`layout_style` from its spec + the active
  theme layers, overwrite in place, leave `id`/`parent`/`children`/
  `access`/`interaction` untouched" -- the entire mechanism a live
  re-theme needs for one node, already built and tested. `reconcile_
  node` only *skips* `patch_node` when the spec is unchanged (a real
  fast path for the common hot-reload case) -- live re-theme is the
  opposite: spec always unchanged, theme changed, so `patch_node`
  needed to run unconditionally instead. This turned what could have
  been a large new subsystem into a small, well-bounded addition.
- Explicitly investigated and deliberately not attempted, named in the
  plan, not silently skipped: live re-theme for `Window`'s own
  imperative catalog. `Window.add_button`/etc. retain no spec at all
  once built -- nothing remembers "this node's container color came
  from resolving 'primary' against variant 'filled'" -- so there is
  nothing analogous to `patch_node` to unconditionally re-run. A real
  fix needs a new per-node theme-derivation tracking mechanism, the
  same "live token-linkage" concept flagged as the highest-risk/
  highest-value piece of the whole customization request since the
  very first investigation in this session, long before M49 existed.
  `Window.set_theme`'s own existing narrow 4-field color re-tint
  (`Tree::set_all_interaction_tints`/`set_all_component_tints`) is
  unchanged by this milestone.
- Implementation (single phase, approved plan):
  - `resolve_theme_layers` (`engine-py/src/view.rs`): `View::new`'s own
    inline ~35-line theme-resolution block factored into a shared
    private helper once `View.set_theme` needed the identical logic --
    the same "two real call sites justify factoring out" precedent
    `resolve_button_colors`/`fill_scrollbar_thumb` already established.
    `View::new` itself confirmed behavior-identical by the full
    pre-existing `test_theme.py` suite re-passing unmodified.
  - `Reconciler::retheme` (`engine-spec/src/reconcile.rs`): walks
    `self.spec` via a new private `retheme_node` recursive helper
    (mirroring `record_ids`'s own walk shape), calling `patch_node`
    unconditionally for every node -- no `node_props_equal` check, since
    the spec is guaranteed unchanged. `&self`, not `&mut self` -- reads
    `self.spec`/`self.ids`, only mutates the passed-in `Tree`. Zero
    changes to `patch_node`/`build_tree` themselves. 2 new unit tests
    passed on the first run.
  - `View.set_theme` (`engine-py/src/view.rs`): calls `resolve_theme_
    layers` then `self.reconciler.retheme(...)`, then updates
    `self.default_theme`/`self.custom_theme`/`self.scheme` so a later
    `poll_reload()` continues using the new theme. **Deliberate
    convention, matching `Window.set_theme`'s own already-shipped
    precedent, not invented fresh here:** each call is a complete,
    fresh theme selection -- omitting `default_theme`/`custom_theme`
    resets to the shipped default, not "keep whatever the previous
    call used."
- A manual Python smoke test (`python3 -c "..."`) run before writing
  formal pytest coverage proved all four core behaviors on the first
  try: shipped default before retheme, override after `set_theme
  (custom_theme=...)`, reset after `set_theme()` with no args, inline
  style still winning after a retheme.
- **A real, verified-not-assumed finding while writing formal pytest
  coverage, caught by actually running the test, not by inspection:** a
  `{{ }}`-bound property does *not* survive a `retheme()` call --
  `patch_node` only recomputes the static style cascade (the identical,
  pre-existing behavior any content-only `poll_reload` already has
  today, not a new limitation this milestone introduces), so a bound
  field reverts to its spec's own static value. The first draft of this
  test asserted the bound value survived; running it showed it reverts
  to the static default instead -- fixed the test to assert the real,
  correct, honest outcome ("reverts sanely to a real value," not
  "crashes" or "goes stale"), matching the plan's own named scope limit.
- `python/tre/_core.pyi` updated with a `View.set_theme` stub, matching
  `Window.set_theme`'s own doc-comment conventions. Extended `examples/
  theme_customization.py` + a new `theme_customization_retheme.yaml`
  fixture: after the window is already showing, `view.set_theme
  (custom_theme=...)` swaps in a second custom theme live, and
  `theme_checkbox` picks up the new value on the exact same, already-
  built node, while `stylesheet_checkbox`/`inline_checkbox` are
  unaffected.
- `BUILD_TRACKER.md`: new M51 milestone section (single phase, 4
  steps), Top Metrics row, "Just closed" prepended, "Up next" updated
  to state nothing is formally scoped yet (M51 closes the declarative-
  surface theme roadmap; live re-theme for `Window`'s imperative
  catalog remains the one named, still-unscoped follow-up). Regenerated
  cleanly on the first attempt.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (`engine-spec` 65, up from 63, +2;
  every other Rust suite unchanged -- no `engine-core`/`engine-render`/
  `engine-md3` logic touched at all), `maturin develop --release`,
  `pytest tests/` (709 passed, up from 702, +7, 1 skipped unchanged),
  all 84 examples (one extended, zero new files, zero failures),
  showcase demo. Tracker generator: 51 milestones/150 phases/262
  items/2 known gaps/19 fixed gaps. Artifact republished to the
  existing URL.

## Status

**M51 -- Live Re-Theme for the Declarative Surface -- is now fully
complete, single phase.** Completes the "theme as a YAML file" roadmap
for the declarative `View` surface end to end: construction-time
resolution (M49/M50) plus genuine live re-resolution (M51), all through
the same `default_theme`/`custom_theme`/`theme_seed` parameters. Live
re-theme for `Window`'s own imperative catalog remains explicitly out
of scope, not yet formally scoped -- the "live token-linkage" mechanism
it would need is real, separate, large follow-up work. Per the standing
"push after a full milestone closes" convention, a `git push` is now
appropriate.
