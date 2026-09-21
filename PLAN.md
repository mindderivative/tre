# PLAN — M49: Theme as a YAML File (Color Overrides + Per-Kind Default Styles)

## Goal
Continuation of the roadmap M48's own writeup named. User: "Yes scope
M49." Scoped via a formal plan (`EnterPlanMode`/`ExitPlanMode`,
`/home/phil/.claude/plans/reflective-sleeping-falcon.md`) after a
dedicated Explore investigation found color overrides could reach the
*entire* real MD3 catalog (not just the declarative 7-kind surface),
while corner-radius/elevation stay a real dead end for this milestone
(every one of `window_factory.rs`'s 56 factories hardcodes them or
takes them as a required argument, never theme-driven).

## Design (4 phases)
1. `crates/engine-md3/src/color.rs`: `ColorScheme::set_role`/
   `apply_overrides` -- the shared choke point. Wired into
   `Window.set_theme(custom_theme=)`/`View::new(default_theme=,
   custom_theme=)`.
2. `crates/engine-spec/src/spec.rs`: `StyleSpec.elevation` (parity gap).
   New `crates/engine-md3/src/shape.rs`: real MD3 shape/elevation scale
   constants (data only, not yet consumed by `window_factory.rs`).
3. `crates/engine-spec/src/cascade.rs`: `resolve_style` split into
   `resolve_style_within_sheet` + `resolve_style_layered` (default <
   custom < stylesheet < inline, each layer's own internal cascade
   resolved independently before merging). Threaded through
   `build_tree`/`patch_node`/`load_styled_view`/`Reconciler`.
4. `crates/engine-py/src/view.rs`: `View.__new__(default_theme=,
   custom_theme=)`; omitting `default_theme` loads the engine's own
   shipped `crates/engine-py/assets/default_theme.yaml` (embedded via
   `include_str!`). New example `examples/theme_customization.py`.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt` clean, `cargo test --workspace --release` (`engine-md3`
19 +5, `engine-spec` 60 +8, every other suite unchanged), `maturin
develop --release`, `pytest tests/` (663 passed, up from 648, +15, 1
skipped unchanged), all 84 examples (+1), showcase demo. Tracker
generator re-verified (49 milestones/144 phases/249 items/2 known
gaps/19 fixed gaps, up from 48/140/238/2/19). **M49 -- is now fully
complete, all 4 phases.**
