# PLAN — M34 Phase 1: `Rect`/`Splitter` Tessellated-Path Cache

## Goal
Scope and implement the "partial/incremental repaint" gap M29's own
trailer named ("only redrawing the changed region of the screen") as
the next milestone, per the user's explicit "Scope the partial/
incremental repaint as the next milestone and start it."

## Investigation (before any design)
1. Direct source read of vendored `vello_hybrid = "0.2.0"`: true
   GPU-level scissored/partial redraw is not achievable as shipped --
   `Renderer::render` has no scissor/dirty-rect param, hardcodes a
   full-target clear every call, and `Scene` has no public sub-
   fragment splice/merge API (every field `pub(crate)`).
2. Checked sibling `pyCopper`'s own real precedent: also redraws its
   whole GPU target every frame; its real win is a CPU-side display-
   list splice (numpy memcpy of cached subtree instances) -- not
   portable to `vello_hybrid`'s `Scene`, which has no equivalent API.
3. First `AskUserQuestion`: presented the real findings above; user
   chose "CPU-side subtree paint caching."
4. Real scratch benchmark (1000-Rect tree, release build): build_tree_
   scene ~2.7-2.9ms/frame. Isolation benchmark: caching tessellated
   BezPath geometry only saves ~20% (2.48ms -> 1.99ms) -- the other
   ~80% is Scene::fill_path's own internal strip-generation cost,
   unavoidable without forking vello_hybrid.
5. Second `AskUserQuestion`, presenting this real ceiling (a
   correction to the premise the first answer was chosen under): user
   chose "Build the real ~20% win anyway."

## Steps
1. New `engine_render::GeometryCache` (`geometry_cache.rs`) -- mirrors
   `TextRenderer::shaped_layout`'s own equality-keyed cache pattern.
   `RectPathParams` enum (`Uniform`/`PerCorner`/`Border`) is the real
   invalidation check. Two separate `HashMap<NodeId, (params, BezPath)>`
   maps (fill, border) so a bordered Rect's two paths don't collide.
2. `paint_node`'s `Rect`/`Splitter` arm now calls `geometry.
   rounded_rect_fill`/`rounded_rect_fill_per_corner`/
   `rounded_rect_border` instead of building a fresh `to_path(0.1)`
   inline. `build_tree_scene`/`paint_node` gained a new `geometry:
   &mut GeometryCache` parameter.
3. `GeometryCache::evict_stale(&tree)` mirrors `evict_stale_layouts`;
   wired into `engine-py::app.rs`'s real per-frame block alongside the
   existing `text_renderer`/`sync_image_textures` calls. `GpuState`
   gained a `geometry_cache` field.
4. All 31 real `build_tree_scene` call sites in engine-render's own
   integration tests, plus `rect_window.rs`'s own local `GpuState`,
   updated to thread the new parameter -- found exhaustively via
   `cargo check`'s own error list (the M33 Phase 2 precedent).
5. Four new real Rust unit tests: cache-hit reuses the identical
   BezPath (pointer identity), cache-miss on a changed radius produces
   a genuinely different path, fill/border caches don't collide,
   eviction removes a removed node's cached paths.
6. Real end-to-end verification benchmark (removed after use): the
   same 1000-Rect tree through the real cached build_tree_scene,
   2.34ms/frame vs the 2.69ms/frame baseline -- a real ~13% end-to-end
   win.
7. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite, unchanged count -- pure internal Rust
   change, no Python-facing surface), all examples, showcase demo.
8. `BUILD_TRACKER.md` (new M34, 1 phase), artifact republish, memory
   update, commit (no push yet per "push after every milestone" --
   this closes the milestone, so push follows).

## Status
Complete. All steps done; full verification chain green (`pytest
tests/` 527 passed/1 skipped, unchanged, zero Python-facing change).
**M34 -- Per-Node Tessellated-Path Caching is now fully complete, its
1 phase.**
