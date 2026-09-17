# Plan: M22 Phase 2 — Real Declarative Image Support & Content-Fit (§16.1), closing M22

Corresponds to `BUILD_TRACKER.md` M22 Phase 2: `NodeKindSpec::Image`
makes `kind: Image` declarable in `view.yaml`, plus a real content-fit
mode (cover/contain/fill) — closing the milestone.

## Investigation before writing code

- `engine-spec::spec.rs`'s own `NodeKindSpec` doc comment already
  named this exact gap: "`Image`/`Canvas` remain real, un-scoped future
  candidates" — confirmed via direct read. `Canvas` stays un-scoped
  (no `kind: Canvas` support exists or is asked for); `Image` is this
  phase's own real target.
- **Spec shape, following the established `text:`/`TextSpec` sibling-
  block precedent exactly** (confirmed via direct read of `WidgetSpec`/
  `TextSpec`, and the module's own doc comment explaining *why* a
  data-carrying `kind: {Image: {...}}` shape was rejected for `Text`):
  a new `WidgetSpec::image: Option<ImageSpec>` field, required (and
  validated as such at tree-build time, matching `text`'s own
  contract) when `kind: Image`. `ImageSpec { src: String, fit:
  ContentFitSpec }` — `src` a path relative to the owning `view.yaml`
  file, `fit` one of `Cover`/`Contain`/`Fill` (`#[serde(default)]` ->
  `Fill`, matching Phase 1's own real "stretched to fill" behavior
  byte-for-byte when a `view.yaml` author doesn't state one).
- **Real, necessary new capability: path resolution needs a
  `base_dir`, which `build_tree`/`load_view`/`load_styled_view` don't
  carry today** — confirmed via direct read, every existing kind's
  construction (`node_kind_and_paint`) is 100% synchronous/pure, no
  file I/O or path context anywhere. `include.rs`'s own `resolve_
  confined(base_dir, include_path)` already solves the identical real
  problem (a relative, potentially-malicious path from inside a
  `view.yaml`, confined to `base_dir`, symlink-escape-resistant, `../`
  and absolute paths rejected) for `include:` — reused here rather
  than inventing a second path-confinement scheme; needs widening from
  private (module-local to `include.rs`) to `pub(crate)` so `build.rs`
  can call it too. `engine-py::View` already tracks its own real
  `path: String` (M19 Phase 1) — the real, already-available source for
  the `base_dir` `View::_attach`/construction threads down into
  `parse_view_with_includes` today; the identical value threads into
  `build_tree` too, one extra parameter, not a new concept.
- **Real, necessary new dependency:** `engine-spec` gains a direct
  `image` dependency (pinned identically to `engine-py`'s own choice:
  `default-features = false, features = ["png", "jpeg"]`) — decoding
  happens here now too, since `kind: Image`'s own real file read/decode
  has to happen at tree-build time, the same moment every other kind's
  spec becomes real `engine-core` state. The ~10-line decode-into-
  `peniko::ImageData` routine `engine-py::Window.add_image` already has
  is small enough that duplicating it here (rather than inventing a
  shared crate neither `engine-spec` nor `engine-py` currently depends
  on, for one function) matches this codebase's own established
  tolerance for small, localized duplication over premature abstraction
  (§2's own "don't build ahead of need").
- **Content-fit is a real paint-time concern, not a build-time
  one:** `NodeKind::Image`/`ImageState` (`engine-core`) stay unchanged
  — the same `peniko::ImageData` Phase 1 already established, no new
  field there. `engine-render::paint_node`'s `NodeKind::Image` arm
  currently always stretches (`SampleRect.transform = scale_non_
  uniform(w/img_w, h/img_h)`, filling the node's box exactly,
  ignoring the image's own aspect ratio) — a real, necessary
  generalization: `ContentFit` (new, plain `Copy` enum:
  `Cover`/`Contain`/`Fill`) has to live somewhere `paint_node` can read
  it *per node*, which means it belongs on `ImageState` itself (a real,
  additive field, `engine-core`), not threaded as a separate parameter
  — matches `CheckboxState.mark_tint`/`SliderState.track_tint`'s own
  "paint-affecting state lives on the kind's own state struct" shape.
  `Fill` reproduces Phase 1's exact current math unchanged (`byte-for-
  byte no visual change` for any existing `Image` node/test); `Cover`/
  `Contain` each compute a real non-uniform-vs-uniform scale factor
  from the image's own real aspect ratio vs. the node's box aspect
  ratio, then center the result (`Cover` crops via a narrower
  `SampleRect.source_region` than the full image when the ratios
  don't match; `Contain` letterboxes by scaling *down* to fit,
  leaving the box's own `background`/transparent fill visible on the
  uncovered sides — real, standard CSS `object-fit` semantics, the
  same de facto standard `background-size: cover/contain` already
  established well before CSS `object-fit` existed).
- `Window.add_image` (`engine-py`, imperative API) is a **separate,
  already-complete real entry point** (Phase 1) — this phase does not
  change it. Should `Window.add_image` also gain a `fit` parameter for
  symmetry with the new declarative `fit:`? Real, deliberate scope
  decision: **yes** — `ImageState.content_fit` is the one real field
  both entry points construct, so leaving the imperative path stuck at
  `Fill` forever while only `view.yaml` can choose would be a real,
  arbitrary asymmetry between the two authoring paths this project has
  consistently avoided elsewhere (every other declarative field this
  project has added has a matching imperative one, and vice versa).
  Additive, default-`Fill`, so this stays fully backward-compatible
  with Phase 1's own `add_image` signature/tests.

## What will change

- `crates/engine-core/src/node.rs`: `ImageState` gains `pub content_fit:
  ContentFit` (new `#[derive(Clone, Copy, Debug, PartialEq)] pub enum
  ContentFit { Cover, Contain, Fill }`, `Fill` as the contract every
  existing Phase 1 test/example already assumes); re-exported from
  `lib.rs`.
- `crates/engine-render/src/lib.rs`: `paint_node`'s `NodeKind::Image`
  arm computes `SampleRect` differently per `state.content_fit` —
  `Fill` unchanged; `Cover`/`Contain` compute real aspect-ratio-aware
  scale/crop/letterbox geometry.
- `crates/engine-spec/Cargo.toml`: new direct `image` dependency
  (pinned identically to `engine-py`'s own choice).
- `crates/engine-spec/src/spec.rs`: new `WidgetSpec::image: Option<
  ImageSpec>`; new `ImageSpec { src: String, fit: ContentFitSpec }`;
  new `ContentFitSpec` enum (`Cover`/`Contain`/`Fill`, `#[serde(default)]`
  -> `Fill`); `NodeKindSpec` gains `Image`.
- `crates/engine-spec/src/include.rs`: `resolve_confined` widened from
  private to `pub(crate)`.
- `crates/engine-spec/src/build.rs`: `load_view`/`load_styled_view`/
  `build_tree`/`patch_node`/`node_kind_and_paint` all gain a `base_dir:
  Option<&Path>` parameter (mirroring `parse_view_with_includes`'s own
  shape exactly); `node_kind_and_paint` gains a real `NodeKindSpec::
  Image` arm — resolves `src` through `resolve_confined`, reads +
  decodes the file via the `image` crate, builds `peniko::ImageData`,
  maps `ContentFitSpec` -> `engine_core::ContentFit`.
- `crates/engine-py/src/view.rs`: threads `View`'s own real `path`
  (parent directory) through as `base_dir` to both `parse_view_with_
  includes` (already does, unchanged) and the new `build_tree`/`load_
  styled_view` parameter.
- `crates/engine-py/src/window.rs`: `Window.add_image` gains an
  optional `fit` parameter (default `Fill`), setting `ImageState.
  content_fit`.
- New `engine-spec` tests: `kind: Image` parses and builds a real
  `NodeKind::Image` node with a real decoded image and the right
  `ContentFit`; a `src:` path escaping `base_dir` is rejected the same
  way an `include:` escape already is; a missing `image:` block on
  `kind: Image` is a clear `SpecError`, not a panic.
- New `engine-render` pixel tests: `Cover`/`Contain`/`Fill` each paint
  the real, geometrically-distinct expected pixels for a non-square
  image in a differently-proportioned box.
- Updated `tests/test_image.py`: `add_image(..., fit=...)` accepted,
  defaults preserved.
- New declarative example (`examples/*.yaml` + a loader script, or an
  addition to an existing composition example) demonstrating `kind:
  Image` with a real `fit:` value.

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run every example script, including the new/updated one(s).
