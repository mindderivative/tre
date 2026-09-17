# Plan: M22 Phase 1 — Real Image Loading & Paint (§5)

Corresponds to `BUILD_TRACKER.md` M22 Phase 1: `NodeKind::Image(ImageState)`,
a real file-backed image loaded through the `image` crate and painted
through `vello_hybrid`'s own real `PaintType::Image` mechanism, plus
`Window.add_image` in `engine-py`.

## Investigation before writing code

- ARCHITECTURE.md's own §5 Core Data Model struct sketch names
  `NodeKind::Image(ImageState)` explicitly, at the same authoritative
  level `Slider(SliderState)`/`Checkbox(CheckboxState)` were named at
  before being built — confirmed via direct read. `engine_core::
  NodeKind` has no `Image` variant yet — confirmed via direct read of
  `crates/engine-core/src/node.rs`, whose own module doc comment
  already lists "`Image`/`Slider`/`Checkbox`/`Canvas` still land with
  their own later build-order steps" — this phase is exactly that.
- **Crate-boundary precedent, confirmed via direct read:**
  `NodeKind::Canvas`'s own module doc comment (`canvas.rs`) states its
  content is "resolved ahead of time by `Tree::set_canvas_content`
  rather than computed live" — the real Python draw callback runs in
  `engine-py::Window.redraw_canvas`, and only the *result*
  (`Vec<DrawCommand>`, plain inert data) ever reaches `engine-core`.
  This phase follows the identical split: file I/O and image *decoding*
  (the `image` crate, PNG/JPEG bytes → raw RGBA8 pixels) happens in
  `engine-py::Window.add_image`, not `engine-core` — `engine-core`
  only ever holds the already-decoded, renderer-agnostic result.
- **What "already-decoded, renderer-agnostic result" means concretely:**
  `peniko::ImageData` (`data: Blob<u8>`, `format: ImageFormat`,
  `alpha_type: ImageAlphaType`, `width: u32`, `height: u32`) —
  confirmed via direct read of the vendored `peniko-0.6.1/src/image.rs`.
  `peniko` is already a real, direct `engine-core` dependency (used
  today for `Color`), so `ImageState` can hold a `peniko::ImageData`
  field directly with zero new dependency-graph edge in `engine-core`.
- **The real paint-side conversion, confirmed via direct source read
  of the vendored crates (not assumed):** `vello_hybrid::Scene::
  set_paint` takes `impl Into<PaintType>`, where `vello_common::paint::
  PaintType = peniko::Brush<vello_common::paint::Image, Gradient>` and
  `vello_common::paint::Image = peniko::ImageBrush<ImageSource>` —
  *not* `peniko::ImageBrush<ImageData>` directly. `vello_common::paint::
  ImageSource::from_peniko_image_data(&peniko::ImageData) -> ImageSource`
  is the real, existing conversion function (confirmed via direct read
  of vendored `vello_common-0.2.0/src/paint.rs`) — the exact same
  function `vello_hybrid`'s own `text.rs` glyph-atlas path already
  relies on for its own real `Image` construction (`vello_hybrid::
  text::set_paint_image`), the closest real precedent already living
  in this dependency tree.
- **Real, necessary new direct dependency:** `vello_hybrid` never
  re-exports `vello_common::paint::{Image, ImageSource, PaintType}` —
  confirmed via grep across its own `lib.rs`'s `pub use` lines (it only
  re-exports `TextureId`/`SizeU16`/`multi_atlas`/`Pixmap`). So
  `engine-render` needs a real, direct `vello_common` dependency to
  name these types itself. Pinned to exactly what `vello_hybrid` 0.2.0
  itself requires (`vello_common = "0.2.0"`, its own `Cargo.toml`) —
  `grep -c 'name = "vello_common"' Cargo.lock` stays at 1 after adding
  it, the same "zero new resolution, only a direct edge" pattern
  `kurbo`/`parley` already established in this file. `default-features
  = false, features = ["std"]` matches `vello_hybrid`'s own exact
  feature set — its default `"png"` feature (an internal PNG decoder)
  is deliberately left off, since decoding goes through this
  workspace's own `image` crate instead.
- **The `image` crate itself, confirmed via `cargo add --dry-run`:**
  latest is `0.25.10`; its *default* feature set pulls in a broad
  format list (avif, bmp, dds, exr, ff, gif, hdr, ico, jpeg, png, pnm,
  qoi, rayon, tga, tiff, webp) — several with heavy/native codec
  dependencies (avif especially), a real risk for the M21-built
  manylinux wheel CI job. `default-features = false, features = ["png",
  "jpeg"]` — the two ubiquitous formats this milestone's own scoping
  paragraph named — mirrors this crate's own established minimal-
  dependency discipline (`arboard`'s own `default-features = false`,
  `tracing-subscriber`'s `env-filter`-only). `Cargo.lock` confirmed:
  `image v0.25.10` plus its real transitive decode deps (`png`,
  `zune-jpeg`, `flate2`, etc.), no `avif`/`rayon`/native-codec entries.
- **`Tree::tick_all` needs no new arm:** confirmed via direct read —
  its real per-kind dispatch only adds arms for `Checkbox`/`Slider`
  (`check_progress`/`thumb_position`, both real *animated* values). An
  `Image` has no animatable field of its own this phase; a real fade-
  in is already `PaintProperties.opacity`'s job, universal to every
  `NodeKind` already.
- **Hit-testing needs no new logic:** `Tree::hit_test_at`'s default
  rect test (§11.10) already applies to any `NodeKind` with no custom
  `CanvasState.hit_test` override — an `Image` node is hit-tested as a
  plain rect exactly like `Rect`/`Checkbox`/`Slider` already are.

## What will change

- `crates/engine-core/src/node.rs`: new `ImageState { pub image:
  peniko::ImageData }` (derives `Clone, Debug, PartialEq`, mirroring
  `peniko::ImageData`'s own derives exactly); new `NodeKind::
  Image(ImageState)` variant.
- `crates/engine-render/Cargo.toml`: new direct `vello_common`
  dependency (see investigation above).
- `crates/engine-render/src/lib.rs`: new `NodeKind::Image(state)` arm
  in `paint_node` — converts `state.image` via `ImageSource::
  from_peniko_image_data`, builds a `vello_common::paint::Image`
  (default `ImageSampler`), `scene.set_paint(...)`, then
  `scene.fill_path` over the node's full `(0, 0, w, h)` rect (Phase 1
  scope: stretched to fill, the same implicit behavior every other
  boxed `NodeKind`'s background fill already has; real content-fit
  modes are Phase 2's own explicit scope, §16.1).
- `crates/engine-py/Cargo.toml`: new direct `image` dependency (see
  investigation above).
- `crates/engine-py/src/error.rs`: new `EngineError::ImageLoadFailed
  { path: String, reason: String }` variant, mapped to `PyIOError` (a
  real file-load/decode failure, not a value/type mismatch the
  existing `PyValueError`/`PyTypeError` variants represent).
- `crates/engine-py/src/window.rs`: new `Window.add_image(path, width,
  height, x=None, y=None) -> PyResult<Node>` — mirrors `add_canvas`'s
  own real shape (hardcoded transparent `PaintProperties` background,
  §11.10/§11.11 precedent: a fully custom-drawn/replaced `NodeKind`
  doesn't expose a separate `background` param the way `Rect`/
  `Checkbox`/`Slider` do, since there's no meaningful "behind the
  content" fill this phase scopes). Reads the file, decodes via
  `image::open`, converts to `.to_rgba8()`, wraps its raw bytes in
  `peniko::Blob::from(Vec<u8>)` (`Blob<T>: From<Vec<T>>`, confirmed via
  direct read), builds `peniko::ImageData { format: Rgba8, alpha_type:
  Alpha (straight/unpremultiplied — the `image` crate's own real
  `to_rgba8()` output), width, height }`.
- New engine-core unit test(s): `NodeKind::Image` round-trips through
  `Tree::insert`/`Tree::get` like every other kind.
- New engine-render pixel test: a real small in-memory `peniko::
  ImageData` (synthesized directly, no file I/O in a Rust test) painted
  and read back via the existing `rect_window`-style pixel-probe
  harness, confirming real pixel colors land where expected.
- New `tests/test_image.py` (pytest, `engine_py` extension): `Window.
  add_image` with a real tiny PNG fixture checked into `tests/
  fixtures/` (or generated on the fly via the `image` crate... no,
  Python-side — via `PIL`? Check `tests/` for an existing image-fixture
  precedent or a lightweight in-test PNG-bytes literal before deciding
  final fixture strategy).
- New `examples/image.py` demonstrating a real loaded image on screen.

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v` (including the new `test_image.py`)
- Run every example script, including the new `examples/image.py`,
  confirmed rendering a real loaded image on screen (manual visual
  check, matching this project's own established example-running
  discipline).
