# Plan: M28 — Code Review Follow-Through

Corresponds to `BUILD_TRACKER.md` M28 (all 3 phases). Written
retroactively alongside implementation — see `LOG.md` and
`BUILD_TRACKER.md`'s own M28 entry for the complete real investigation,
findings, and verification record.

## What changed

- `crates/engine-render/src/text.rs`: `TextRenderer` gained a real
  per-`NodeId` shaped-`Layout` cache (`shaped_layout`), keyed on
  `content`/`font_family`/`font_weight`/`font_size`/`max_width` — no
  separate invalidation logic, since a changed key is the cache miss.
  New `evict_stale_layouts(&tree)`, called once per frame from
  `App::run` alongside `sync_image_textures`. New `#[cfg(test)] mod
  tests` proving caching and eviction both actually happen.
- `crates/engine-render/src/lib.rs`/`crates/engine-py/src/app.rs`:
  threaded `node_id`/the new eviction call through `paint_node` and the
  per-frame render path.
- `crates/engine-core/src/tree.rs`: `Tree::remove` now also clears
  `self.overlays` for the removed id (previously only `close_overlay`
  did). New test covering both direct removal and removed-ancestor
  cases.
- `crates/engine-py/src/window.rs` (1712 lines) split into `window.rs`
  (construction/theming/GC), `window_factory.rs` (9 `add_*`/
  `build_shell` methods), `window_input.rs` (11 synthetic-dispatch
  methods), `window_docking.rs` (8 `dock.rs` delegation wrappers), and
  `window_virtual_canvas.rs` (4 methods). Enabled by pyo3's
  `multiple-pymethods` feature (`crates/engine-py/Cargo.toml`) —
  verified this actually merges all 5 `#[pymethods]` blocks into one
  Python-visible `Window` class before relying on it. Zero behavior
  change: every method moved verbatim.

## What was deliberately not done

- The redraw-loop half of the text-shaping finding (the render loop
  requests a redraw unconditionally every frame, `engine-platform/src/
  lib.rs`) is still open. Investigated directly: `Tree::tick_all`'s
  `any_active` return value looked like a ready-made dirty signal, but
  it only covers animated properties — it misses one-shot property
  writes, `TextField` typing, scroll, drag, focus changes, and every
  imperative `add_*`/`set_*` call. Correctly gating redraws needs a
  real dirty-tracking mechanism threaded through ~30–40 mutating
  methods across `engine-core`/`engine-py` — a genuine architectural
  change to the core render loop, not a contained fix, so it's left
  for a deliberate future pass rather than rushed here.

See `LOG.md` for the full narrative and verification results.
