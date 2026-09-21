# LOG — M49: Theme as a YAML File (Color Overrides + Per-Kind Default Styles)

- User: "Yes scope M49" -- following M48's own writeup naming M49 as
  the next roadmap piece ("theme YAML carrying both a token/palette
  section and per-kind default styles, layered beneath the existing
  Stylesheet cascade").
- Entered Plan Mode. Dispatched an Explore agent to answer the open
  question: does theme YAML reach the *real* MD3 component catalog
  (`window_factory.rs`'s 56 `add_*` factories) or only the narrow
  declarative 7-kind surface? Findings: corner-radius/elevation are a
  real dead end -- every factory hardcodes them as a Rust `const` or a
  required Python argument, confirmed via grep, never theme-driven.
  Color is NOT a dead end -- `window_factory.rs`'s `resolve_*_colors`
  helpers and the declarative `resolve_color` path both already
  resolve roles through the exact same `ColorScheme::role` lookup, so
  a single override layer applied wherever a `ColorScheme` is selected
  reaches the entire real catalog, not just 7 kinds. This materially
  improved the plan over its original "theme YAML only affects Rect/
  Container" framing.
- Implementation (4 phases, approved plan):
  - Phase 1: `ColorScheme::set_role`/`apply_overrides`
    (`engine-md3/src/color.rs`) -- a mirrored setter of the existing
    `role()` getter plus an override-applying method reusing the same
    hex/CSS parser used elsewhere. Wired into `Window.set_theme`'s new
    `custom_theme` param and `View::new`'s new `default_theme`/
    `custom_theme` params.
  - Phase 2: `StyleSpec.elevation` (a real parity gap -- `PaintProperties
    .elevation` existed since day one, never exposed to YAML). New
    `engine_md3::shape` module -- real, tested MD3 shape/elevation
    scale constants, replacing ~40 scattered doc-comment-only mentions
    across `window_factory.rs` with one source of truth (data only,
    not yet consumed by those call sites -- named as explicit
    out-of-scope follow-up).
  - Phase 3: `cascade.rs`'s `resolve_style` split into
    `resolve_style_within_sheet` (unchanged external behavior) +
    `resolve_style_layered(spec, default_theme, custom_theme, sheet)`
    -- each layer resolved through its own independent cascade before
    merging in source-priority order, inline last. Deliberately NOT
    concatenating all three sheets' rules into one combined list
    (would let a default theme's `id:` rule beat an app's own
    `baseline` stylesheet rule, backwards from the user's own stated
    model). Threaded through `build_tree`/`patch_node`/
    `load_styled_view`/`Reconciler::load`/`Reconciler::reconcile`,
    mirroring exactly how `sheet` was already threaded.
  - Phase 4: `View.__new__(default_theme=, custom_theme=)` --
    omitting `default_theme` loads the engine's own shipped
    `crates/engine-py/assets/default_theme.yaml`, embedded at compile
    time via `include_str!` (never a runtime filesystem lookup).
    `Window.set_theme(custom_theme=)` confirmed to need no
    `default_theme` equivalent (its corner-radius/elevation never
    consult any theme regardless).
- **Real design decision worth recording:** seed precedence is
  DIFFERENT direction between `View`/`Window` on purpose, not an
  oversight -- `View`'s `theme_seed` is optional, so an explicit value
  there is itself the "don't defer to the file" signal; `Window.
  set_theme`'s `seed` is required, so the theme file's own seed wins
  when present, since there's no way to omit a required argument to
  express deference. Documented directly in both doc comments.
- No real test-authoring mistakes this time -- all 15 new pytest tests
  passed on the first real run (verified via an ad hoc smoke-test
  script before writing the formal suite, catching nothing new).
- `python/tre/_core.pyi` updated (`View.__init__`, `Window.set_theme`).
  New `tests/test_theme.py` (15 tests). New `examples/
  theme_customization.py` + 3 YAML fixtures, proving all four real
  cascade tiers plus color overrides reaching both the declarative and
  imperative catalog in one script.
- `BUILD_TRACKER.md`: new M49 milestone section (4 phases, 11 steps),
  Top Metrics row, "Just closed" prepended, "Up next" pointed forward
  to M50. Regenerated cleanly on the first attempt (no parser-format
  mistakes this time, unlike M47/M48's own real bugs found there).
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
  `cargo test --workspace --release` (`engine-md3` 19 up from 14 +5,
  `engine-spec` 60 up from 52 +8, every other suite unchanged),
  `maturin develop --release`, `pytest tests/` (663 passed, up from
  648, +15, 1 skipped unchanged), all 84 examples (+1), showcase demo.
  Tracker generator: 49 milestones/144 phases/249 items/2 known
  gaps/19 fixed gaps. Artifact republished to the existing URL.

## Status

**M49 -- Theme as a YAML File -- is now fully complete, all 4 phases.**
The second concrete piece of the user's broader customization request.
M50 (live re-theme) remains sketched-but-unscoped in the approved plan,
needing its own dedicated plan-mode pass now that M49's theme-as-data
shape exists to re-resolve from. Per the standing "push after a full
milestone closes" convention, a `git push` is now appropriate.
