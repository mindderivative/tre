# PLAN — M50: Theme-Driven Shape & Elevation for the Imperative MD3 Catalog

## Goal
User: "Scope the corner-radius/elevation to use the new theme pattern.
I would like the theme backend complete before re-theming everything
else" -- explicitly reprioritizing this ahead of M51/live re-theme
(M49's own originally-scoped "up next"). Scoped via a formal plan
(`EnterPlanMode`/`ExitPlanMode`), grounded in a dedicated Explore
agent's exhaustive audit of all 56 `add_*` factories in
`window_factory.rs`: 28 have a real, themeable corner radius; 14 of
those also have real elevation; the other 28 have neither.

## Design (5 phases)
1. `ThemeSpec.components`/`ComponentOverride` (`engine-spec/src/
   theme.rs`) -- a separate namespace from `styles:`, keyed by
   `"<component>"`/`"<component>.<variant>"`. `ThemeState.components`/
   `shape()`/`elevation()` (`engine-py/src/window.rs`) -- a real
   2-tier, per-field lookup (caught and fixed a shadowing bug before
   any factory used it).
2. Wired the 8 Buttons & FAB factories -- caught and fixed a real bug
   where `add_split_button`/`add_button_group`'s own shape-morph code
   recomputed `height / 2.0` independently of whatever `add_button`
   itself had just resolved for `corner_radius`.
3. Wired the 10 Containers & Surfaces factories.
4. Wired the final 10 Navigation/data-entry/misc factories.
5. Populated the shipped `crates/engine-py/assets/default_theme.yaml`'s
   `components:` section with every *safe* (dimension-independent)
   default value -- deliberately omitting components whose real
   default is a formula over a caller-supplied dimension. Found and
   closed a real completeness gap: `Window.set_theme` had no way to
   auto-load any default theme at all, so gave it its own
   `default_theme` parameter mirroring `View.__new__`.

## Status
Complete. Full verification chain green at every phase boundary:
`cargo check`/`clippy -D warnings`/`fmt` clean, `cargo test --workspace
--release` (`engine-py` 20 +9, `engine-spec` 63 +3, every other suite
unchanged), `maturin develop --release`, `pytest tests/` (702 passed,
up from 663, +39, 1 skipped unchanged), all 84 examples (one extended),
showcase demo -- re-run in full after every phase to confirm zero
behavior change for any pre-existing example/test. Tracker generator
re-verified (50 milestones/149 phases/258 items/2 known gaps/19 fixed
gaps, up from 49/144/249/2/19). **M50 -- is now fully complete, all 5
phases.**
