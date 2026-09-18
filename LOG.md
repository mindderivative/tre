# Log: M30 Phase 2 Step 1 — Radio Button

## `CheckboxState`'s own real shape, mirrored

`RadioButtonState { selected, select_progress: Animated<f64>,
unselected_tint, selected_tint }` — `selected`/`select_progress` are
the direct analogues of `checked`/`check_progress`, the same Design
Principle 6 shape (the engine never toggles `selected` itself, only
reflects it once the app writes it).

## A real anatomy difference, not copied blindly

MD3's real radio button is a stroked *ring*, not a filled box — and
the ring's own color genuinely transitions between an unselected and
selected tint as it toggles, unlike `Checkbox`'s box (a static fill,
only the checkmark itself appears/disappears). `RadioButtonState`
carries two plain tints rather than one, and `engine-render`'s paint
arm interpolates between them using `select_progress` as the blend
factor — `Interpolate for peniko::Color` was already real (§5's own
animation core, `lerp_rect` under the hood), so this needed no new
color-blending machinery, just calling the existing trait method
directly outside the `Animated<T>` wrapper.

## A real, easy-to-miss ticking gap, caught by checking precedent directly

`Tree::tick_all` doesn't use an exhaustive `match` for per-`NodeKind`
ticking — it's a sequence of `if let NodeKind::X(state) = &mut
node.kind` arms, one per kind that needs central ticking. This means
the compiler's exhaustiveness check (which caught every other
`NodeKind::RadioButton` match site automatically) would **not** have
caught a missing `select_progress` tick arm — it would have compiled
clean and simply never animated. Found by deliberately re-reading
`Checkbox`'s/`Slider`'s own real precedent in this exact function
before considering the ticking wired up, not by trusting the compiler
to have already caught it. Added the mirror arm explicitly.

## Real, explicit scope limit on live re-theming

`Tree::set_all_component_tints` (the mechanism `Window.set_theme`
uses to retroactively re-tint already-created `Checkbox`/`Slider`/
`TextField` nodes) takes one shared `Color` and pushes it to every
matching component — its own already-documented real scope choice
("reuses this exact same already-resolved on-surface tint rather than
resolving a second, more specific MD3 role per component," `window.
rs`'s own `set_theme` doc comment). A radio button genuinely needs two
different real roles (`outline` for unselected, `primary` for
selected), which that single-color mechanism can't express without
contradicting its own stated simplification. Rather than force a bad
fit or build a second, wider re-tint mechanism, this is stated as a
real, honest limitation: a radio button created before `set_theme` is
not retroactively re-tinted by a later call. Every radio button still
starts correctly themed at construction time (the same real contract
`add_button`/`add_fab`/etc already have), which is the common case.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 41 binaries, all green (`radio_button_
paint.rs` included, proving the ring's real color interpolation and
the dot's real scale-in). `maturin develop --release` rebuilt. `pytest
tests/`: 243 passed, 1 skipped (9 new in `test_radio_button.py`, zero
regressions). All 35 examples and the showcase demo re-run clean.
`mypy --strict` clean against `examples/radio_button.py`.
