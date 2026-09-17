# Log: M20 Phase 1 — Real Checkbox/Slider Component Theming (§7.1, §7.3)

Corresponds to `BUILD_TRACKER.md` M20 Phase 1: `CheckboxState` gains
`mark_tint: Color`, `SliderState` gains `track_tint: Color` — both
threaded through `paint_node`'s existing hardcoded-literal paint
sites, pushed a real resolved color by `Window.set_theme`.

## Investigation before writing code

`paint_node`'s `Checkbox`/`Slider` arms painted a hardcoded white
checkmark and a hardcoded gray track — confirmed via direct read.
`Tree::set_all_interaction_tints` (M7 Phase 3) is the real, established
precedent for this class of problem, but scoped specifically to the
optional `InteractionState::tint` — `mark_tint`/`track_tint` are
different in kind (plain fields every real `CheckboxState`/
`SliderState` always has), so a new, separate `Tree::
set_all_component_tints` was added rather than widening the existing,
already-tested method.

## What happened

`CheckboxState`/`SliderState` gain `mark_tint`/`track_tint`, each
defaulting to the exact historical hardcoded literal. New `Tree::
set_all_component_tints(tint)` walks every node, updating whichever
field matches — unconditional per matching node, no opt-in gate
(unlike `InteractionState`, these aren't an optional capability).
`paint_node` reads the new fields instead of the literals. `Window.
set_theme` and the real live OS `ThemeChanged` path both call the new
method alongside the existing one, using the identical already-
resolved "on-surface" tint.

**Real bug caught before it shipped, not by a test:** the first design
read `ThemeState::on_surface()` unconditionally at `add_checkbox`/
`add_slider` construction time, mirroring `Node.enable_interaction`'s
own real precedent — but `on_surface()`'s own no-theme-set default is
black, while `CheckboxState`/`SliderState`'s own real historical
defaults are white/gray. Applying it unconditionally would have
silently replaced every un-themed checkbox's white mark and slider's
gray track with black the moment this phase shipped — a real
regression `InteractionState.tint` never had, since its own hardcoded
default already happens to equal `on_surface()`'s no-theme value "by
coincidence." Fixed with a new `ThemeState::is_set()` accessor,
gating the construction-time read so the real historical default
survives untouched until an app genuinely calls `set_theme`.

New `engine-core` test (1, passed first run): `set_all_component_tints`
updates a `Checkbox`/`Slider` on their own distinct fields and leaves
an unrelated `NodeKind` untouched. New `engine-render` pixel tests
(3, all passed first run): an un-themed slider still paints the real,
byte-for-byte historical gray track; a themed checkbox paints the real
resolved tint on its checkmark; a themed slider paints the real
resolved tint on its track. Updated `examples/theme.py`: a checkbox
created *before* `set_theme` (re-tinted by the push) and a slider
created *after* (themed at construction) — both real, distinct
timing cases.

Full `cargo test --workspace --release` (`engine-core` 138, up from
137; `engine-render` 3 in `checkbox_paint.rs`, up from 2, 4 in
`slider_paint.rs`, up from 2)/`cargo clippy --workspace --all-targets
-- -D warnings`/`cargo fmt --check` all clean — every prior test
passed unmodified. `maturin develop --release` + full `pytest tests/`
(168 passed, unchanged, 1 skipped — confirming no new Python-facing
FFI surface was needed, colors aren't Python-observable via `Node.
get()`) and all twenty-seven examples confirmed clean.
