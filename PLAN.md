# PLAN — M30 Phase 9 Step 1: Video

## Goal
Add `Window.add_video`/`Node.push_frame` — MD3 has no official page.
Reuse `NodeKind::Image` directly as a "frame sink," grounded in real,
directly-applicable precedent from the sibling `pyCopper` project's
own `Video` widget.

## Steps
1. Confirm no official MD3 Video page.
2. Read pyCopper's real `Video` widget (`src/pycopper/widgets/
   video.py`) directly for its own real design rationale, not assumed
   from memory.
3. Investigate whether TRE's own `engine-render` GPU texture cache
   (`image_cache.rs`) supports re-uploading an existing `Image` node's
   pixel content — read its own doc comment and `sync` logic directly.
4. **Found a real, confirmed gap**: `sync`'s own guard only checked
   whether a texture already existed for a node, never whether its
   content changed — a video's first frame would paint forever.
5. Design the fix: key re-upload on `peniko::Blob<u8>`'s own real
   content-identity `id()` (confirmed cheap/O(1) via direct source
   read of `linebender_resource_handle`, not a pixel memcmp) instead
   of node presence.
6. Implement the fix in `ImageTextureCache` (`uploaded: HashMap<NodeId,
   u64>`, `sync`'s guard, eviction cleanup).
7. Write a new, dedicated `engine-render` unit test proving the fix:
   replacing a node's `ImageState.image` with fresh content must
   trigger a real re-upload.
8. Implement `Window.add_video(width, height, fit, x, y)` in
   `window_factory.rs`, mirroring `add_image`'s exact contract minus
   the file-decode step (a single transparent placeholder pixel
   instead).
9. Implement `Node.push_frame(rgba, width, height)` in `node.rs`,
   mirroring `set_text`'s established chokepoint pattern (`get_mut` +
   `NodeKind` match, `EngineError::UnknownProperty` for any other
   kind); validate buffer length with a real `PyValueError`.
10. Add `.pyi` stubs for both.
11. Write `tests/test_video.py` and `examples/video.py` (a real
    synthetic decode loop, including a mid-stream resolution change).
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
13. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 9 heading icon,
    Step 1 line, "Just closed"/"Up next" trailer), regenerate +
    republish the Build Tracker artifact.
14. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-
render` 4 tests up from 3, 457 pytest passed/1 skipped up from 446,
all 63 examples, showcase demo, 44 Rust test binaries).
