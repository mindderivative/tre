# PLAN — M48: General Live Property Exposure (Layout Mutation + Border)

## Goal
User's governing instruction: "I want all properties usable and exposed
for a users use. Customization is a key function of any GUI framework.
Even changing the MD3 theme should be customizable by a user as the MD3
specification is just the default setting." Scoped via a formal plan
(`EnterPlanMode`/`ExitPlanMode`, `/home/phil/.claude/plans/reflective-
sleeping-falcon.md`) into a sequenced roadmap; M48 is the first,
concrete, ready-to-implement piece. M49 (theme as a YAML cascade tier)
and M50 (live re-theme) are sketched in that plan but explicitly
deferred to their own future plan-mode passes.

## Design
1. `crates/engine-py/src/node.rs`: new `Node.set_layout(width, height,
   padding, gap)`, generalizing `resize_terminal`'s exact real pattern
   (clone `layout_style`, patch only the fields passed, push through
   `Tree::set_layout_style`). Immediate, not eased.
2. `crates/engine-py/src/node.rs`: `animate()`/`get()` gain
   `border_color`/`border_width` arms, mirroring `background`/
   `corner_radius` exactly.
3. `crates/engine-spec/src/spec.rs`: `StyleSpec` gains `border_width`/
   `border_color`. `crates/engine-spec/src/build.rs`: `node_kind_and_
   paint` split into itself (border resolution) + `node_kind_and_base_
   paint` (every pre-existing match arm, unchanged). `crates/engine-
   spec/src/cascade.rs`: `merge()` gains the matching two field-overlay
   arms.
4. `crates/engine-py/src/view.rs`: `apply_binding_value`'s `Str`-as-
   color guard widened to `border_color`; new pre-`animate()` branch
   routes `width`/`height`/`padding`/`gap` bindings to `Node.set_layout`.
5. `crates/engine-py/src/window_factory.rs`: `add_rect` gains optional
   `border_color`/`border_width` construction kwargs (the one genuinely
   generic imperative shape factory -- no `add_container` exists).
6. Docs/tests: `python/tre/_core.pyi` stubs, `tests/test_live_style.py`
   (19 new tests), `examples/live_style.py`/`.yaml`, `BUILD_TRACKER.md`.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt` clean, `cargo test --workspace --release` (all
pre-existing suites unchanged -- zero new Rust-level `#[test]`s needed,
no new `engine-core` logic), `maturin develop --release`, `pytest
tests/` (648 passed, up from 629, +19, 1 skipped unchanged), all 83
examples (+1), showcase demo. Tracker generator re-verified (48
milestones/140 phases/238 items/2 known gaps/19 fixed gaps, up from
47/139/233/2/19). **M48 -- is now fully complete.**
