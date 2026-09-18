# Log: M30 Phase 2 Step 2 — Switch

## `RadioButtonState`'s own real shape, mirrored a third time

`SwitchState { on, toggle_progress: Animated<f64>, track_off_tint,
track_on_tint, track_outline_tint, handle_off_tint, handle_on_tint }`
— `on`/`toggle_progress` are the direct analogues of `checked`/
`check_progress` and `selected`/`select_progress`.

## Real MD3 data, verified before writing any code

Checked Material Web's own real token source (`tokens/versions/
v0_192/_md-comp-switch.scss`) directly: track 52dp × 32dp, `corner-
full`. Unselected track has a real *separate* stroke role
(`outline`) distinct from its own fill role (`surface_container_
highest`) — two different colors, not the same role reused. Selected
track fills with `primary`, no stroke. The handle itself grows, not
just changes color: 16dp unselected (`outline` tint) to 24dp selected
(`on_primary` tint).

## The slide-and-grow formula, derived algebraically, not fitted

Real MD3 padding-from-edge differs by state (8px unselected, 4px
selected on a 32px track), and handle radius differs too (8px
unselected, 12px selected) — naively this looks like two different
formulas would be needed for the two ends of travel. Checked directly
before writing the paint code: `padding + handle_radius` is `8+8=16`
unselected and `4+12=16` selected — identical, `h*0.5` either way. That
identity is what makes one clean symmetric `cx = h*0.5 + t*(w-h)`
formula exact at both `t=0` and `t=1`, not an approximation that
happens to look close. `handle_radius = h*(0.25 + 0.125*t)` comes
directly from the same two real ratios (16/32=0.5 diameter unselected,
i.e. 0.25 radius; 24/32=0.75 diameter selected, i.e. 0.375 radius).

## The outline's real fade

The track's own separate outline stroke needed to visually disappear
as the switch turns on (real MD3 behavior: no visible outline once
selected, since the primary fill alone reads as "on"). Rather than
reuse the universal `PaintProperties.border_color`/`border_width`
fields (`Button`'s Outlined variant's own mechanism), this is drawn as
a plain conditional stroke inside `Switch`'s own paint arm, opacity
scaled by `1.0 - toggle_progress` — the outline is genuinely a
`Switch`-owned concept (a fifth real color no other component in this
catalog needed), not something the shared border fields were built
for.

## The same real, honest live-re-theming scope limit, restated

`Switch` needs five distinct real MD3 roles at once (`RadioButton`
needed two) — even further from `Tree::set_all_component_tints`'s
single-shared-tint design. The same real, stated limitation applies:
a switch created before `Window.set_theme` is not retroactively
re-tinted by a later call; every switch still starts correctly themed
at construction.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 42 binaries, all green (`switch_paint.rs`
included, proving the track's real color transition, the outline's
real fade, and the handle's real slide-and-grow). `maturin develop
--release` rebuilt. `pytest tests/`: 251 passed, 1 skipped (8 new in
`test_switch.py`, zero regressions). All 36 examples and the showcase
demo re-run clean. `mypy --strict` clean against `examples/switch.py`.
