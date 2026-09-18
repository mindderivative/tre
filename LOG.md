# LOG — M30 Phase 9 Step 1: Video

- Confirmed no official MD3 Video page via the same directory-listing
  technique used throughout this milestone.
- Read the sibling `pyCopper` project's own real `Video` widget
  directly (`/home/phil/pyDev/projects/pyCopper/src/pycopper/widgets/
  video.py`, an additional working directory this session already has
  access to) rather than assuming a design from memory. Real, directly
  applicable finding: `Video` there is a *frame sink*, not a decoder —
  nothing decodes `.mp4`/`.webm` in that project either (only Pillow
  for still images), and the application (not the framework) owns the
  decode loop, pushing frames via `push_frame(rgba)`. Reused this
  exact design and naming for TRE.
- Investigated whether TRE's own render pipeline supports an `Image`
  node's pixel content changing after creation, by reading `engine-
  render/src/image_cache.rs` directly rather than assuming. **Found a
  real, confirmed gap**: its own doc comment stated plainly "an Image
  node's pixel data never changes after Window.add_image... a real
  upload only ever happens once per node," and `sync`'s own guard
  (`if self.textures.contains_key(&id) { continue; }`) checked only
  node presence, never content. A live video stream would have
  silently kept painting its very first frame forever.
- Investigated whether `peniko::ImageData`/`Blob<u8>`'s own `PartialEq`
  would make a content-equality check affordable per frame at video
  resolutions — read the vendored `linebender_resource_handle` crate's
  own `Blob` source directly. Real, confirmed finding: `Blob<T>`
  carries a real, unique, monotonically-assigned `u64` `id()` set at
  construction (`Blob::new`/`Blob::from`), and its own `PartialEq`
  compares *only* that id — a cheap O(1) comparison, never a byte-
  level memcmp of the pixel buffer itself. This made the whole fix
  affordable without any new complexity.
- Fixed `ImageTextureCache`: added `uploaded: HashMap<NodeId, u64>`
  tracking each node's last-uploaded blob id; `sync`'s guard now
  compares the current blob id against the tracked one, re-uploading
  (recreating the whole GPU texture, a real, deliberate scope
  simplification over an incremental `write_texture`-only fast path)
  whenever they differ; the existing node-removal eviction loop also
  clears the tracked id.
- Wrote a new, dedicated `engine-render` unit test,
  `sync_reuploads_a_texture_when_its_own_node_content_genuinely_
  changes`: builds a real two-frame scene (a genuinely different
  `Blob` the second time), asserts the cache's own tracked content id
  actually changes on the second `sync`, and that exactly one texture
  exists throughout (no leak/duplicate). Passed on the first run.
- Implemented `Window.add_video(width, height, fit="fill", x=None,
  y=None)` in `crates/engine-py/src/window_factory.rs`, right after
  `add_image` — mirrors its exact contract (fixed box, `content_fit`
  param) minus the file-decode step, replaced with a single fully-
  transparent 1x1 placeholder pixel.
- Implemented `Node.push_frame(rgba, width, height)` in `crates/
  engine-py/src/node.rs`, right after `set_text` — reuses its exact
  established chokepoint pattern (`tree.get_mut` + `NodeKind` match,
  `EngineError::UnknownProperty` for any non-`Image` node). Validates
  `rgba.len() == width * height * 4` with a real, clear `PyValueError`
  (`add_icon`'s own "fail loudly on pure validation, no I/O involved"
  convention).
- Added `.pyi` stubs for both.
- Wrote `tests/test_video.py` (11 tests) — all passed on the first
  run: `add_video` returns a `Node`, positions like every other `add_*`
  method, accepts each real `fit` value and rejects an unknown one,
  `push_frame` accepts a correctly-sized buffer and rejects a wrong-
  sized one or a non-`Image` node, repeated pushes (a synthetic live
  stream) never raise, and a mid-stream resolution change is accepted.
- Wrote `examples/video.py` — a real synthetic decode loop pushing 5
  distinct frames (including a mid-stream resolution renegotiation)
  before `app.run`, the same "decode happens before the render loop,
  then app.run proves the full pipeline runs clean" structure
  `examples/image.py` already established (noted honestly in this
  script's own doc comment: `App.run` has no real per-frame Python
  hook today, confirmed via direct check of `_core.pyi`). Clean on the
  first run, `mypy --strict` clean too.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (44 binaries green,
  `engine-render` 4 up from 3), `maturin develop --release`, `pytest
  tests/` (457 passed, 1 skipped, up from 446), all 63 examples clean,
  showcase demo clean.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 92%, Phase 9 heading
  icon, Step 1 line, "Just closed"/"Up next" trailer), regenerated and
  republished the Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
