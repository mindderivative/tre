# LOG — Milestone 86, Phase 2: Font Registration

- User-directed: "Start M86". Phase 1 (theme/stylesheet `*_spec=`
  kwargs) landed in `babef68`.

## What shipped

1. `engine-render/src/fonts.rs`: a process-global, append-only font
   registry. `register_font(data)` parses into a scratch
   `fontique::Collection` first (validation + family names), then
   appends the blob unless identical bytes are already registered, and
   bumps a generation counter.
2. `TextRenderer::new` registers the 4 vendored faces plus every
   registered blob. New `TextRenderer::sync_registered_fonts` registers
   blobs added since the renderer last synced and clears all three
   shaping caches; one atomic load when nothing changed.
3. `engine-py/src/app.rs`: the per-frame gate treats "fonts changed" as
   a third repaint reason beside `take_dirty` and `resized`. Limit: an
   idle window under `ControlFlow::Wait` repaints on its next wake.
4. Python `tre.register_font(data: bytes) -> list[str]`, exported from
   `tre`, stubbed in `_core.pyi`. Takes `&[u8]`, borrowing the `bytes`
   object directly; a `str` path is a `TypeError`.
- Tests: 4 Rust tests, including a render-level one that builds a
  Roboto-only renderer and proves the synced Hack face measures ~0.6em
  rather than a Roboto fallback; 6 pytest cases.
- Verification: `fmt --check`/`clippy -D warnings` clean; `maturin
  develop --release`; `pytest tests/` 914 passed, 2 skipped; `mypy
  --strict` on `_core.pyi` clean.

## Status

**Phase 2 complete.** Next: Phase 3, MkDocs and the full verification
chain.
