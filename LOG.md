# LOG — M34 Phase 1: `Rect`/`Splitter` Tessellated-Path Cache

- Direct source read of the vendored `vello_hybrid = "0.2.0"` before
  designing anything: true GPU-level scissored/partial redraw is not
  achievable as shipped. `Renderer::render`'s own public signature has
  no scissor/dirty-rect parameter; `render()` hardcodes `clear = true`,
  running a full-target `LoadOp::Clear` pass with no scissor on every
  call. `Scene` itself has no public sub-fragment record/replay/merge
  API -- every field, including its own `CommandRecorder`, is
  `pub(crate)`. The one real region-limited path that exists
  (`clear_atlas_region`, genuinely uses `LoadOp::Load` + a scissor
  rect) is private and targets atlas layers, not the user-facing view.
- Checked the sibling `pyCopper` project's own real precedent (the
  established discipline for every prior real capability this session
  has built): it also redraws its whole GPU target every frame -- its
  real optimization is CPU-side, a `_needs_paint` dirty flag plus a
  numpy memcpy splice of cached per-subtree draw-list instances back
  into a fresh display list. Real, measured in their own numbers
  (0.002ms splice vs 3.27ms rebuild). Not portable to `vello_hybrid`'s
  `Scene` directly -- no equivalent splice API exists there.
- First `AskUserQuestion` (matching the Code Folding/Terminal Mouse
  Selection precedent for a genuinely novel capability with no real
  reference implementation): presented the real findings above. User
  chose "CPU-side subtree paint caching (Recommended)."
- Real benchmark before committing to a specific design: a scratch
  Rust test (1000 static Rect + 200 Text nodes, release build,
  `#[ignore]`d, removed after use) measured `build_tree_scene` at
  ~2.7-2.9ms/frame. Text nodes contributed negligible cost (already
  cached by `shaped_layout` since M28 Phase 1). A follow-up isolation
  benchmark -- filling the same 1000 rects from a fresh `BezPath` each
  time vs one tessellated once and reused -- found only ~20% of the
  cost (2.48ms -> 1.99ms) comes from `RoundedRect::to_path(0.1)`'s own
  tessellation; the remaining ~80% is `Scene::fill_path`'s own
  internal strip-generation cost, paid regardless of path freshness,
  unavoidable without forking vello_hybrid (confirmed no splice API).
- **Real, honest correction surfaced mid-implementation, not glossed
  over:** the first `AskUserQuestion`'s own framing ("CPU-side
  subtree caching... drops toward zero") turned out wrong once real
  numbers came in -- the achievable ceiling here is ~20%, not
  dramatic, since `Scene` can't skip re-emitting into itself the way
  pyCopper's own sliceable format can. Presented this reversal
  directly via a second `AskUserQuestion` rather than silently
  building against a disproven premise. User chose "Build the real
  ~20% win anyway (Recommended)."
- New `engine_render::GeometryCache` (`geometry_cache.rs`), mirroring
  `TextRenderer::shaped_layout`'s own equality-keyed cache pattern
  exactly: a `RectPathParams` enum (`Uniform`/`PerCorner`/`Border`) is
  the real invalidation check -- no separate "remember to invalidate"
  bookkeeping. Two separate `HashMap<NodeId, (RectPathParams,
  BezPath)>` maps (fill, border), not one shared map, since a single
  bordered `Rect` needs both cached independently.
- `paint_node`'s `Rect`/`Splitter` arm now calls `geometry.
  rounded_rect_fill`/`rounded_rect_fill_per_corner`/
  `rounded_rect_border` instead of building a fresh `RoundedRect::
  to_path(0.1)` inline every frame. `build_tree_scene`/`paint_node`
  both gained a new `geometry: &mut GeometryCache` parameter, the
  identical caller-owned cross-frame threading `resources`/`text`
  already establish.
- `GeometryCache::evict_stale(&tree)` mirrors `evict_stale_layouts`'s
  own established per-node-cache-leak fix. Wired into `engine-py::
  app.rs`'s real per-frame block alongside the existing calls;
  `GpuState` gained a `geometry_cache: GeometryCache` field.
- All 31 real `build_tree_scene` call sites in `engine-render`'s own
  integration tests, plus the standalone `rect_window.rs` example's
  own local `GpuState`, updated to thread the new parameter -- found
  and fixed exhaustively via `cargo check`'s own error list, the
  identical reliable-worklist technique M33 Phase 2 already
  established (script-patched 30 of 31 files mechanically; one file
  used `&mut self.text_renderer` through a struct field the script
  couldn't parse, fixed by hand).
- No Python-facing API change at all -- confirmed via `git diff
  --stat crates/engine-py/`: only `app.rs` touched, no `#[pymethods]`/
  `#[pyclass]` signature changed. No new `.pyi` stub, no new example.
- Four new real Rust unit tests (`geometry_cache.rs`): an unchanged
  node reuses the exact same cached `BezPath` (proven by pointer
  identity on its backing storage, since `BezPath` has no
  `PartialEq`); a changed `radius` invalidates the cache and produces
  a genuinely different path; fill and border caches for the same
  node don't collide; a removed node's cached paths are evicted.
- Real end-to-end verification (a second scratch benchmark, removed
  after use): the same 1000-Rect static tree through the real,
  now-cached `build_tree_scene` across 200 repeated frames -- 2.34ms/
  frame, down from the 2.69ms/frame elevation-0 baseline measured
  during scoping, a real ~13% end-to-end reduction (below the ~20%
  isolated figure, since the full walk also spends time on bounds/
  culling math the isolated test excluded).
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean (all 32 real
  `build_tree_scene` call sites compiling), `cargo test --workspace
  --release` clean (4 new `geometry_cache` tests, zero regressions,
  unchanged non-geometry_cache counts confirming additive-only),
  `maturin develop --release` rebuilt, `pytest tests/` 527 passed/1
  skipped (unchanged -- pure internal Rust optimization), all 71
  examples and the showcase demo re-run clean.
- Updated `BUILD_TRACKER.md` (new M34, 1 phase, Top Metrics row) --
  verified the parser's own reported item count before/after (33/109/
  199 -> 34/110/200, exactly +1/+1/+1 matching the single new phase),
  regenerated and republished the Build Tracker artifact. **This
  closes M34 Phase 1 and, with it, M34 itself, its 1 phase.**
