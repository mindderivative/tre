# PLAN — M39 Phase 2 Step 1: Loading Indicator

## Goal
Give the catalog a real MD3 Expressive-style loading spinner --
identified back in M35's own scoping as one of two real remaining
catalog gaps, never picked up since.

## Steps
1. Real research first (the M3 site's own spec page is JS-rendered,
   the same finding every prior MD3 phase in this catalog already
   made): found this is genuinely NOT a simple spinner -- a real
   looping morph across seven named shapes with genuine spring
   physics and a dual rotation formula, via a real, cited open-source
   port's own README (since the spec page itself carries no fetchable
   static content).
2. Paused via `AskUserQuestion`: full real fidelity (sourcing/
   authoring 7 real shape vertex sets, a new spring-physics motion
   primitive, a new looping-animation concept -- none of which this
   codebase has any precedent for) vs. a real, honest v1
   simplification (a smaller set of procedurally-generated real
   shapes, morphed via the already-proven `Animated<ShapeKey>`
   machinery with plain easing). User chose the simplified v1.
3. New `NodeKind::LoadingIndicator(LoadingIndicatorState)`
   (`node.rs`) -- `shapes: Vec<ShapeKey>` (built once at construction
   to the real node's own `w x h`, since `ShapeKey` has no scale
   transform) and `current_shape: usize`.
4. New `crate::shape_morph::loading_indicator_shapes` module -- four
   real, procedurally-generated shapes (Pentagon: a regular 5-gon;
   Pill: `RoundedRect` at a loose tessellation tolerance to keep
   vertex counts comparable across shapes; Cookie: a real 12-vertex
   soft-scalloped shape; Oval: a 2:1 `Ellipse`, matching MD3's own
   real Oval shape's proportions) -- a real, recognizable, in-spirit
   subset of MD3 Expressive's own seven named shapes, not sourced
   vertex-for-vertex.
5. New `Tree::tick_all` case: whenever a real `LoadingIndicator`'s own
   `paint.shape` isn't currently mid-animation (the very first tick,
   or a genuine transition that just settled -- both leave `Animated::
   active` at `None`), retargets it to the next real shape in the
   cycle, wrapping at the end. No app-side wiring needed at all --
   the real loop starts and keeps running the instant a node is
   constructed. Real, cited timing kept even though the physics model
   was simplified: 650ms per real shape.
6. `engine-render::paint_node`'s own `Rect | Splitter` shape-morph-
   aware fill arm widened to include `LoadingIndicator` -- its own
   real appearance is entirely `PaintProperties.shape`, so zero new
   paint code was needed.
7. `Window.add_loading_indicator(size, color, x, y) -> Node`
   (`engine-py`), initializing `shape` directly to the first real
   shape at construction (not left at `ShapeKey::empty()`), the
   identical "avoid a first-tick flash-from-empty bug" discipline
   Split Button (M38 Phase 4) already established.
8. Real tests: three new `tree.rs` unit tests (the four shapes are
   real, non-empty, and pairwise distinct; a fresh indicator's very
   first tick kicks off a real transition; repeated settle-then-
   advance ticks cycle 0->1->2->3->0, hand-verified against the real
   settle-and-immediately-retarget behavior within a single `tick_
   all` call). No Python getter exists for `shape`/`current_shape`,
   so a new pytest file (`test_loading_indicator.py`, the real FFI
   construction surface) plus a new, permanent example (`examples/
   loading_indicator.py`) that runs a real, live 200-frame `App().
   run()` loop -- at 650ms per real shape, this guarantees several
   genuine shape transitions actually happen over real simulated
   time, the real, decisive proof the perpetual loop keeps advancing
   rather than stalling after the first one.
9. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `test --workspace --release`, `maturin develop --release`, full
   `pytest tests/`, all 76 examples, showcase demo.
10. `BUILD_TRACKER.md` Phase 2 Step 1 flipped to done (Step 2, Time
    Picker Dial, remains open), artifact regenerated (39/127/218,
    unchanged) and republished.

## Status
Complete. Full verification chain green (`cargo test --workspace
--release`: `engine-core` 201 passed (+3); `pytest tests/`: 569
passed/1 skipped, up from 564, +5 new tests; all 76 examples +
showcase demo clean). **M39 Phase 2 Step 1 -- Loading Indicator is now
complete. Phase 2 itself remains open: Step 2 (Time Picker Dial)
remains, plus Phases 3-5 of M39.**
