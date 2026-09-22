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
1. **Mechanism + `add_button` proof of concept.** `RetitheHook = Box<dyn
   Fn(&ThemeState, &mut Tree)>`, a new `PyWindow.retheme_hooks: RefCell
   <Vec<RetitheHook>>` side table (mirrors `materializers`/`canvas_
   draws`), replayed at the end of `Window.set_theme` alongside the
   existing (unchanged) tint pushes. Each themed factory builds its hook
   via a small, named, independently-testable builder function, not an
   inline closure.
2. Fixed/simple `PaintProperties`-only factories (~19).
3. Buttons & FAB family (~8), including the two structurally unusual
   nested-reuse cases last.
4. Conditional/variable multi-node Containers & Navigation (~9).
5. Non-`PaintProperties` stateful components (~9) -- closes the 5
   confirmed pre-existing gaps as a direct byproduct.
6. Docs/example/verification wrap-up. No `_core.pyi` change needed --
   `Window.set_theme`'s signature is unchanged throughout.

## Explicitly out of scope
Making `add_tooltip`'s color or `build_menu`'s panel shape/elevation
theme-resolved for the first time (M49/M50's job, not this milestone's).
Any new Python-facing API surface. Pruning `retheme_hooks` on node
removal (matches the existing, accepted `handlers`/`materializers`/
`context_menus` precedent).

## Status
**Phases 1-2 of 6 complete.** Phase 1: `RetitheHook` mechanism +
`Window.set_theme` wiring + `add_button` fully wired. 5 new Rust unit
tests (GIL-free: 2 generic-mechanism, 3 `button_retheme_hook`
exact-value -- a real test-authoring mistake caught by running it:
MD3's `on_primary` role can legitimately resolve to the identical white
for two different dark seeds, so an initial "must differ" assertion was
wrong, fixed to assert exact resolved values instead). 4 new pytest
tests.

Phase 2: all 19 remaining fixed/simple `PaintProperties`-only factories
wired -- each a new, named `*_retheme_hook` builder function, grouped
together in one dedicated preamble section. Handled several real
non-trivial shapes: `add_toolbar`'s conditional default fallbacks
(branch on `is_floating`/`vertical`/original `width`/`height`, not
fixed constants); `add_date_picker_day`'s 4-outcome branch; the
milestone's first `IconState.tint`-touching hooks (`add_list_item`/
`add_spin_box`, a plain non-`Animated` field); `add_accordion_header`/
`add_tree_node` carefully never touch their chevron's `paint.
transform`; `add_tooltip`'s hook correctly does less than its siblings
(color is a named, deliberately unfixed pre-existing gap). 11 new
pytest tests extending each factory's own M50-era fixture with a
second `set_theme` call, plus one combined "does not raise" test for
color-only factories. **A real, honest finding caught by running the
tests:** an `add_card` test's bare `"card"` elevation override never
took effect -- the shipped default theme already sets the more
specific `card.elevated` key, which the 2-tier lookup always checks
first; fixed to override that key explicitly.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
`cargo test --workspace --release` (`engine-py` 25, unchanged from
Phase 1 -- Phase 2 added no new Rust tests), `maturin develop
--release`, `pytest tests/` (725 passed, up from 709 at M52's start,
+16, 1 skipped unchanged), all 84 examples, showcase demo. Tracker
generator: 52 milestones/152 phases/266 items/2 known gaps/19 fixed
gaps.

Phase 3: all 8 Buttons & FAB family factories wired -- `add_icon_
button`, `add_fab`, `add_extended_fab`, `add_chip`, `add_badge`
(dot/labeled pair), `add_segmented_button` (per-segment state captured
at construction time, `is_selected` never re-derived per Design
Principle 6), then `add_split_button`/`add_button_group` last. **Real
emergent design property discovered while wiring the two nested-reuse
cases:** since both call `self.add_button(...)` internally, that call
already registers its own plain button hook -- the new hooks only
layer the additional shape-morph geometry on top (disjoint fields),
not redundantly re-resolve colors. **A real compiler-caught bug, not
shipped:** an initial draft of the segmented-button/button-group hooks
consumed their captured `Vec`s via `.into_iter()` -- `RetitheHook`
(`Fn`, not `FnOnce`) must be callable multiple times, so this would
have panicked on a second `set_theme()` call; caught by `cargo check`
before any test ran, fixed to iterate by reference.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
`cargo test --workspace --release` (unchanged -- no new Rust tests this
phase either), `maturin develop --release`, `pytest tests/` (734
passed, up from 725, +9, 1 skipped unchanged), all 84 examples,
showcase demo. Tracker generator: 52 milestones/153 phases/267
items/2 known gaps/19 fixed gaps. **Up next: Phase 4, the ~9
conditional/variable multi-node Containers & Navigation factories.**
