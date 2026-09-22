# PLAN — M52: Live Re-Theme for `Window`'s Imperative MD3 Catalog

## Goal
User: "Scope the live re-theme for Window's imperative MD3 catalog" --
the "live token-linkage" mechanism M51's own writeup named and
deliberately deferred since M49. Scoped via a formal plan
(`EnterPlanMode`/`ExitPlanMode`), grounded in a dedicated Explore
agent's exhaustive audit of all 58 `add_*` factories in
`window_factory.rs`/`window_virtual_canvas.rs`.

## Real investigation
46 of 58 factories resolve a theme-derived value and are in scope. The
file's 4 shared `resolve_*_colors` helpers are confirmed pure Rust (no
GIL type in any signature) -- every hook this milestone registers can
run with no Python interpreter involved. Node topology: 30 fixed, 8
conditional-but-bounded, 8 genuinely `Vec`-driven. `add_split_button`/
`add_button_group` are structurally unusual (call `add_button`
internally, then overwrite shape-morph fields via a second, independent
theme lookup). 9 factories set theme-derived colors on their own
`NodeKind` payload field, not `PaintProperties` -- 5 of those
(RadioButton/Switch/LinearProgress/CircularProgress/TimePickerDial) are
confirmed completely untouched by `Window.set_theme` today, a larger
gap than the milestone's own naming implied. Two pre-existing gaps
(`add_tooltip` color, `build_menu` panel shape/elevation) are named but
deliberately not fixed here.

## Design (6 phases, 46 factories)
1. Mechanism + `add_button` proof of concept.
2. Fixed/simple `PaintProperties`-only factories (19).
3. Buttons & FAB family (8), including the two structurally unusual
   nested-reuse cases last.
4. Conditional/variable multi-node Containers & Navigation (9).
5. Non-`PaintProperties` stateful components (9) -- closes 5 confirmed
   pre-existing gaps as a direct byproduct.
6. Docs/example/verification wrap-up.

## Status

**Complete, all 6 phases.**

Phase 1: `RetitheHook = Box<dyn Fn(&ThemeState, &mut Tree)>`, a new
`PyWindow.retheme_hooks` side table mirroring `materializers`/
`canvas_draws`, replayed at the end of `Window.set_theme` alongside the
existing (unchanged) tint pushes. `add_button` wired as proof of
concept. 5 new Rust unit tests -- a real test-authoring mistake caught
by running it (MD3's `on_primary` role can legitimately resolve to the
same white for two different dark seeds).

Phase 2: all 19 fixed/simple `PaintProperties`-only factories wired,
each hook grouped in one new dedicated preamble section rather than
scattered near each factory's own consts (deliberate, for this
milestone's own large batch). A real finding caught by running the
tests: the shipped default theme's `card.elevated` sub-key silently
shadows a bare-key override in the 2-tier lookup -- correct, existing
behavior, not a bug.

Phase 3: all 8 Buttons & FAB factories wired. Real emergent design
property found while wiring `add_split_button`/`add_button_group`:
since both call `self.add_button(...)` internally, that call already
registers its own plain button hook -- the new hooks only layer the
additional shape-morph geometry on top, not redundant color resolution.
A real bug caught by the compiler, not shipped: an initial draft
consumed a captured `Vec` via `.into_iter()`, which `Fn` (not
`FnOnce`) rejects outright since `RetitheHook` must be callable on
every `set_theme()` call.

Phase 4: all 9 conditional/variable Containers & Navigation factories
wired. Two real, pre-existing inconsistencies (a hardcoded icon-button
corner radius in `add_search_bar`, a third distinct key convention in
`add_pagination`) reproduced faithfully, not corrected -- out of this
milestone's scope. A real parser error (unclosed outer paren in a long
`BUILD_TRACKER.md` note) caught by the generator, not shipped.

Phase 5: real investigation before writing code confirmed `add_
checkbox`/`add_slider`/`add_text_field`/`add_code_editor` need zero new
Rust code -- their themed fields are exactly what the pre-existing
`Tree::set_all_component_tints` already re-tints on every `set_theme()`
call. Wired the 5 confirmed, previously-undocumented gaps instead:
`add_radio_button`, `add_switch`, `add_linear_progress`, `add_circular_
progress`, `add_time_picker_dial`. 2 new exact-value Rust unit tests.

Phase 6: `python/tre/_core.pyi`'s `Window.set_theme` docstring corrected
(it still claimed no live re-theming existed, now false) -- no
signature change. `examples/theme_customization.py` extended with a
`window.set_theme(...)` call proving the button's `corner_radius`/
`elevation` change live, mirroring M51's own `view.set_theme(...)`
proof.

Full final chain green: `cargo check --workspace --all-targets`/
`cargo clippy --workspace --all-targets -- -D warnings`/`cargo fmt
--check` clean; `cargo test --workspace --release` (`engine-py` 27, up
from 20 at M52's start, +7); `maturin develop --release`; `pytest
tests/` (747 passed, up from 709 at M52's start, +38, 1 skipped
unchanged); all 84 examples (one extended); `demo/showcase.py` (all 5
phases, exit 0). Tracker generator: 52 milestones/156 phases/272
items/2 known gaps/19 fixed gaps.

**M52 -- Live Re-Theme for `Window`'s Imperative MD3 Catalog -- is now
fully complete, all 6 phases.** Closes the "theme as a YAML file"
roadmap end to end, across both real node-creation surfaces this
engine has.
