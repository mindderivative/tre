# Log: M27 Phase 2 — MD3 Component & Theming Gallery Screen

`demo/showcase.py`'s "components" placeholder replaced with a real
gallery: `Checkbox`, `Slider`, `TextField`, `Image`, and `Icon` all
live, plus a real seed-color/dark-mode theme picker exercising
`Window.set_theme`'s own real live-re-theming path (confirmed:
`set_theme` unconditionally calls both `Tree::set_all_interaction_tints`
and `Tree::set_all_component_tints` on every call, retroactively
re-tinting every already-built themed component, not just future ones
— genuinely live re-theming, not just "new nodes pick up the new
theme").

**A real, genuine gap found while building this screen, not assumed:**
`Window` had no way to create a plain `NodeKind::Text` label at all —
`NodeKind::Text` has been fully real and renderable since §14 step 4,
and a declarative `kind: Text` widget has built one since step 5, but
no imperative `add_text` existed (confirmed via grep before writing
any code). New `Window.add_text(content, background, width, height,
font_family="Roboto", font_weight=400.0, font_size=16.0, x=None,
y=None)`, mirroring `add_rect`'s own exact shape — `background` is
repurposed as the glyph color, the identical real convention
`paint_node`'s own `NodeKind::Text` arm and the declarative
`required_background(..., "Text")` path already establish.

**Two more real, connected bugs found only by actually running the
gallery end to end:**

1. `Node.animate(property, value, duration_ms=0)` only *registers* the
   animation — it snaps to the target the next time something ticks
   the node, normally `App.run()`'s per-frame loop (already documented
   in `view.rs::apply_binding_value`'s own doc comment, but not
   something this demo's own first draft accounted for): a Slider
   nudge attempted via `.animate("thumb_position", 0.9, duration_ms=0)`
   before `app.run()` ever starts silently never landed. Fixed by
   using the same real, already-proven mechanism `examples/slider.py`
   established instead — a real Tab-focus + dispatched `ArrowRight`
   key, which internally ticks immediately (unlike `animate()`).
2. Tab order in the gallery screen starts *after* the two nav buttons
   (built first, in `build_showcase`, before any screen's own content
   exists) — a first draft's verification assumed the gallery's own
   `Checkbox` was the first Tab stop; it's actually the third (two nav
   buttons, then the checkbox). Fixed by accounting for the nav
   buttons explicitly, confirmed by a standalone debug script isolating
   the gallery screen alone (which needed only 2 Tabs, not 4) before
   fixing the real script.

Verification for the whole gallery avoids any new pixel-level readback
(the definitive color-correctness proof stays in `engine-render`'s own
tests, the same split every other example in this workspace already
uses): `Checkbox.get_checked()` before/after a real toggle,
`Slider.get("thumb_position")` before/after a real keyboard nudge,
`TextField.get_text()` round-tripping real content, and every one of
the 4 seed swatches plus the dark-mode toggle exercised through a real
`Window.set_theme` call with no error.

Full `cargo test --workspace --release` (all pre-existing suites
unmodified and passing)/clippy `-D warnings`/fmt clean. `maturin
develop --release` + `pytest tests/` (187 passed, unchanged, 1
pre-existing skip), all 33 pre-existing examples, and the updated demo
confirmed clean with the real display. Updated `docs/api/python/
window.md` with the new `add_text` method. `mkdocs build --strict`
clean.

M27 Phase 2 — MD3 Component & Theming Gallery Screen is now complete.
M27 continues with Phase 3 (motion & custom-drawing screen).
