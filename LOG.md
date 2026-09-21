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

## Status

**M52 Phase 1 of 6 is complete.** The mechanism is proven end to end on
the simplest real themed factory. Per this session's own standing
discipline, committing locally now (a phase boundary, not yet a closed
milestone) -- push deferred until the full milestone closes, matching
the established "push after a full milestone" convention. Up next:
Phase 2, the ~19 remaining fixed/simple `PaintProperties`-only
factories.
