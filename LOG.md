# Log: M22 Phase 1 — Real Image Loading & Paint (§5)

Corresponds to `BUILD_TRACKER.md` M22 Phase 1: `NodeKind::Image(ImageState)`,
a real file-backed image loaded through the `image` crate and painted
through a real GPU texture, plus `Window.add_image` in `engine-py`.

## Investigation before writing code

ARCHITECTURE.md's own §5 struct sketch names `NodeKind::Image(ImageState)`
explicitly, at the same authoritative level `Slider`/`Checkbox` were named
at before being built — confirmed via direct read; `engine_core::NodeKind`
had no `Image` variant yet, the largest remaining gap between §5's sketch
and the live enum. `peniko::ImageData` (already a real `engine-core`
dependency via `Color`) is exactly the shape a decoded image needs, so
`ImageState` holds one directly, zero new dependency edge in `engine-core`
— decoding a real file (the `image` crate) happens in `engine-py::Window.
add_image`, the identical "engine-core holds inert data, resolved ahead of
time" split `NodeKind::Canvas`'s own module doc comment already
established for its Python draw callback.

**Real, decisive finding — not assumed, found by a failing test:**
`vello_hybrid` 0.2.0's ordinary `Scene::set_paint`+`fill_path` path
panics on any CPU-side pixel data (`ImageSource::Pixmap`) — "pixmap
image sources are not supported by Vello Hybrid." Its own wgpu renderer
only ever accepts `ImageSource::OpaqueId` (a pre-registered image), and
the `ImageCache` that would register one is `pub(crate)` inside
`vello_hybrid` itself, unreachable from an application. The one real,
currently-supported path for CPU-decoded image data is `Scene::
draw_texture_rects` + `TextureBindings`: an ordinary, externally-owned
`wgpu::Texture`, uploaded once and bound under a `TextureId` the caller
itself manages. This is a genuinely different mechanism from every other
`NodeKind`'s paint, discovered only by attempting the real render (the
same "empirical, not theoretical" discipline M21's manylinux
investigation already established this project's habit of).

## What happened

New `crates/engine-render/src/image_cache.rs`: `ImageTextureCache` —
per-`Image`-node GPU textures, keyed by a stable per-`NodeId` `u64`
(new `engine_core::node_id_as_u64`, the identical `KeyData::as_ffi()`
scheme `to_access_id` already established for a different foreign-handle
consumer). `sync(tree, device, queue)` walks a new `Tree::image_nodes()`
iterator (additive, engine-core), uploading a real premultiplied-RGBA8
texture for any `Image` node not already cached — reusing
`vello_common::paint::ImageSource::from_peniko_image_data`'s own real
RGBA8/BGRA8 + premultiply conversion (the same one `vello_hybrid`'s own
glyph-atlas path already relies on) rather than duplicating that logic.
An `Image` node's pixels never change after construction this phase (no
swap-the-image API is scoped), so a real upload only ever happens once
per node.

`FrameRenderer` now owns an `ImageTextureCache`, exposed via a new
`sync_image_textures(tree, device, queue)` method — fully additive to its
public API; every existing caller (`app.rs`'s render loop, every
`*_paint.rs` pixel test) needed zero signature changes, since `render`'s
own `TextureBindings` argument was already internal, and stays empty
(byte-for-byte this crate's pre-M22 behavior) unless a caller's tree
actually has `Image` nodes and calls the new method. `paint_node`'s new
`NodeKind::Image` arm computes the same deterministic `TextureId` and
calls `scene.draw_texture_rects(...)` with a `SampleRect` scaling the
image's own real pixel extent up to the node's `(w, h)` box — Phase 1's
own stated "stretched to fill" scope; real content-fit modes are Phase
2's, §16.1. `engine-py::App::run`'s real render loop now calls
`sync_image_textures` once per frame, right before scene construction.

`Window.add_image(path, width, height, x=None, y=None)`: reads and
decodes the file via the `image` crate (`default-features = false,
features = ["png", "jpeg"]` — the two ubiquitous formats this milestone
scoped, deliberately not `image`'s own much heavier default feature set,
which includes `avif`/`rayon`/several native codec deps a real risk for
the M21-built manylinux wheel CI job), converts to real straight-alpha
RGBA8 via `.to_rgba8()`, wraps the raw bytes in `peniko::Blob::from`.
Mirrors `add_canvas`'s own real precedent for `PaintProperties`
(hardcoded transparent fill, no separate `background` param) since a
fully custom/loaded-content `NodeKind` has no meaningful "behind the
content" color this phase scopes. New `EngineError::ImageLoadFailed`,
mapped to `PyIOError` (a real I/O/decode failure, not a value/type
mismatch the existing variants represent).

New `engine-core` unit test: `NodeKind::Image` round-trips through
`Tree::insert`/`Tree::get`. New `crates/engine-render/tests/image_paint.rs`
— the real, headless GPU pixel-readback proof (mirroring `checkbox_
paint.rs`/`slider_paint.rs`'s own established pattern): a synthesized
2x2 green `peniko::ImageData`, painted through the real new texture-cache
path, its real color confirmed at a pixel inside the node and plain
background confirmed outside it. New `tests/test_image.py` (pytest): a
real PNG generated on the fly (no fixtures directory exists in this
suite; a tiny in-test image costs nothing to keep in sync), `add_image`
returns a real `Node`, a missing/corrupt file raises a real `OSError`
instead of panicking. New `examples/image.py`: a real, tiny 4x4 PNG
(embedded as a base64 literal — no example script in this project
depends on anything beyond the standard library and `tre` itself,
confirmed via grep), loaded and rendered through a real, full `App.run`
loop, exiting cleanly after 60 frames.

Full `cargo test --workspace --release` (all crates, including the two
new tests), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` all clean. `maturin develop --release` +
`pytest tests/` (176 passed, 1 pre-existing skip) + every script in
`examples/` run headlessly.

**Real, pre-existing, unrelated finding (not a regression):**
`examples/animation_completion.py`/`container_transform.py` both fail —
a real animation's `on_complete` callback never fires in this dev
environment. Confirmed via a real git worktree at the M21-closing commit
(before any M22 work) that this fails identically on that baseline too —
not caused by this phase's changes. Flagged as a separate, out-of-scope
task rather than fixed here (this phase's own real scope is image
rendering, not this unrelated pre-existing gap).

M22 Phase 1 — Real Image Loading & Paint is now complete.
