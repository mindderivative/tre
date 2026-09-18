# Log: M28 — Code Review Follow-Through

Fixed all 3 review findings that needed a human design call, following
the trade-offs the review artifact itself proposed — plus one real
correction to those trade-offs, found only by investigating further
before implementing rather than executing the artifact's own framing
verbatim.

## Text-shaping cache (Phase 1)

The review found `TextRenderer::draw`/`draw_field` re-running the full
`parley` shaping pipeline — font matching, line-breaking, BiDi,
alignment — from scratch on every single paint, every frame,
regardless of whether the node's content or size had changed.

**Real finding that changed the plan, before writing any code:** the
review's own report recommended dirty-gating the redraw loop first
(framed as lower-risk) and a shaping cache second (framed as carrying
real invalidation risk). Re-investigating both options directly showed
the opposite. A shaping cache keyed on its own exact shaping inputs
(`content`, `font_family`, `font_weight`, `font_size`, `max_width`)
needs no separate invalidation logic at all — equality on the key *is*
the invalidation check, so there's nothing a future change could
forget to update. Gating the redraw loop correctly, by contrast, would
need a real dirty flag threaded through every mutation path that can
change what's on screen: `Tree::tick_all` already returns `any_active`
covering every *animated* property, but that misses one-shot
`Node.set()` writes, `TextField` typing, scroll, drag, focus changes,
and every one of `engine-py`'s ~30–40 imperative `add_*`/`set_*`
methods. That's a real architectural change to the core render loop
every example and the showcase demo runs through — correctly left open
rather than rushed (see "What was deliberately not done" in `PLAN.md`).

Implementation: `TextRenderer` gained a `HashMap<NodeId,
CachedLayout>`. `shaped_layout` looks up by the node's own identity,
rebuilding only when the key changed. `evict_stale_layouts(&tree)`
mirrors `ImageTextureCache::sync`'s already-existing eviction pattern
from the review's own autonomous fix — a `NodeId` no longer in the
tree is a safe staleness check (slotmap generational keys), called
once per frame from `App::run` right next to `sync_image_textures`.

`text.rs`'s own module is private (only `TextRenderer`/`TextPlacement`
are re-exported), so an external integration test can't reach the
cache's internals — a new `#[cfg(test)] mod tests` inside `text.rs`
itself proves: repainting an unchanged node doesn't grow the cache, a
genuinely different node gets its own entry, and removing a node from
the tree evicts its cached layout.

## Overlay-cleanup leak (Phase 1)

Small, contained: `Tree::remove` never cleared `self.overlays` for a
removed node that happened to be open overlay content — only
`close_overlay` did. A caller removing that same content through the
general-purpose `remove` (or removing one of its ancestors, reached
via `remove`'s own recursive descent) left a permanently orphaned
`OverlayMeta` entry. Fixed with one line (`self.overlays.remove(&id)`
right after `self.nodes.remove(id)`), a no-op for the common case of a
node that was never overlay content. New test covers both the
direct-removal and removed-ancestor cases.

## `window.rs` split (Phase 2)

The review flagged `engine-py/src/window.rs` (1712 lines) for mixing
four largely-independent responsibilities: node-factory methods,
synthetic input dispatch, docking delegation, and virtual-list/canvas
plumbing — sharing one file only because that's where each was added
at the time.

**Verified the mechanism before committing to the plan, not assumed:**
`PyWindow`'s `#[pymethods] impl` block can't itself span multiple
files without pyo3's `multiple-pymethods` Cargo feature — confirmed by
reading the vendored pyo3 source directly (its own doc comment: the
real cost is `inventory`-based registration not supporting Wasm, which
this desktop winit/wgpu project never targets). Enabled it in
`engine-py/Cargo.toml`.

Split into `window.rs` (construction, theming, GC lifecycle — kept),
`window_factory.rs` (`add_rect`/`add_text`/`add_checkbox`/
`add_slider`/`add_image`/`add_icon`/`add_text_field`/`build_shell`/
`add_splitter`), `window_input.rs` (`click`/`hover`/`scroll`/
`right_click`/`press_key`/`type_text`/`copy`/`cut`/`paste`/container-
transform), `window_docking.rs` (the 8 thin `dock.rs` wrappers), and
`window_virtual_canvas.rs` (`add_virtual_list`/
`set_virtual_list_window`/`add_canvas`/`redraw_canvas`). Every
method's real body moved verbatim — a Python script did the line-range
extraction against exactly grep-verified boundaries, not hand-retyped,
to rule out transcription drift. Each new file's `use` block was built
from a first pass, then corrected entirely by the compiler's own
missing/unused-import diagnostics.

`positioned_style` (needed by both `window_factory.rs` and
`window_virtual_canvas.rs`) stays `pub(crate)` in `window.rs` itself;
`parse_content_fit` and `ResolvedItemHeights` (each needed by exactly
one of the new files) moved there entirely instead. `wrap_node` and
the `materializers`/`canvas_draws` fields widened from private to
`pub(crate)` — the minimum visibility change the split needed,
extending `PyWindow`'s own already-established field-level
`pub(crate)` convention rather than inventing a new one.

Zero behavior change: every Python-visible method name, signature, and
body is byte-for-byte the same code, just relocated. The full pytest
suite, every example, and the showcase demo all pass unmodified — the
real proof that pyo3 genuinely merges all 5 `#[pymethods]` blocks into
one Python-visible `Window` class, not five separate ones.

## Verification

Full `cargo check --workspace --all-targets`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` clean across all
changed files. `cargo test --workspace --release` clean, including
both new regression tests (`engine-core` 145 tests, up from 144;
`engine-render` 3 unit tests, up from 2). `maturin develop --release`
+ `pytest tests/` (187 passed, unchanged, 1 pre-existing skip), all 29
examples, and `demo/showcase.py` all confirmed clean with the real
display.

M28 — Code Review Follow-Through is now complete. All 5 review
findings that needed either an autonomous fix or a human design call
are resolved; the redraw-loop half of the performance finding is
correctly re-scoped and left open, its real scope now sharper than the
review artifact's own original framing.
