# Log: M22 Phase 2 — Real Declarative Image Support & Content-Fit (§16.1), closing M22

Corresponds to `BUILD_TRACKER.md` M22 Phase 2, closing M22 entirely:
`NodeKindSpec::Image` makes `kind: Image` declarable in `view.yaml`,
plus a real content-fit mode (`Cover`/`Contain`/`Fill`).

## Investigation before writing code

`engine-spec::spec.rs`'s own `NodeKindSpec` doc comment already named
this exact gap ("`Image`/`Canvas` remain real, un-scoped future
candidates"). `Canvas` stays un-scoped (its content is a Python draw
callback, no obvious static YAML shape); `Image` is this phase's real
target, following `text:`'s own established sibling-block precedent
(`WidgetSpec::image: Option<ImageSpec>`, required when `kind: Image`).

**Real, necessary new capability, confirmed via direct read:**
`build_tree`/`load_view`/`load_styled_view`/`node_kind_and_paint` were
100% synchronous/pure before this phase — no path context anywhere —
but resolving a relative `image.src:` needs a `base_dir`, the identical
real problem `include:` already solved (`include.rs`'s own private
`resolve_confined`: canonicalization-based, symlink-escape-resistant
path confinement). Reusing that function directly turned out wrong:
its own `SpecError` variants are named for `include:` specifically
(`"include: ... resolves outside the view directory"`), and reusing
them verbatim for a `src:` escape would misname the real YAML key an
author wrote. Duplicated the ~15-line confinement logic instead
(`resolve_image_src`), with its own `ImageSrc*`/`ImageReadFailed`/
`ImageDecodeFailed` `SpecError` variants — small, deliberate
duplication over a premature shared abstraction for two call sites,
this codebase's own established tolerance.

`engine-py::View` already tracked its own real `path` (M19 Phase 1) and
already threaded a `base_dir` into `parse_view_with_includes` — the
identical value now also reaches `build_tree`/`patch_node` through
`Reconciler::load`/`reconcile` (whose own public signatures already
carried `base_dir`, just previously dropped it before this phase), so
`View`'s own code needed zero changes.

`load_view`'s own public signature (its doc comment: "kept exactly
as-is for its one existing caller") stays completely unchanged —
`None` is real and valid, the identical `include:`-established
contract, so a `kind: Image` loaded through it gets a real, clear
`SpecError::ImageSrcNoBaseDir` rather than a widened signature nobody
asked for.

## What happened

New `engine-core::ContentFit` (`Cover`/`Contain`/`Fill`, `#[default]
Fill`) on a new `ImageState.content_fit` field (`ImageState::new`
seeds it to `Fill` — byte-for-byte Phase 1's own only behavior, zero
visual change for any existing `Image` node). New `engine-render`
`image_sample_rect` helper computes the real `(source_region,
transform)` pair `Scene::draw_texture_rects` needs per mode: `Fill`
unchanged; `Contain` scales uniformly by the smaller axis ratio and
centers via a real composed translate (letterboxing the box's own
`background` on the longer axis); `Cover` crops `source_region` to the
box's own aspect ratio (centered within the full image) so a uniform
scale of the crop lands exactly on the box with zero overflow —
deliberately not "scale the full image up and rely on an implicit
clip," since `NodeKind::Image` has none and none was needed.
`Window.add_image` gained a matching `fit` parameter (`"cover"`/
`"contain"`/`"fill"`, default `"fill"`), parsed via a new
`parse_content_fit`, the identical string-vocabulary pattern
`parse_dock_side` already established — kept symmetric with the new
declarative `image.fit:` rather than leaving the imperative path stuck
at `Fill` forever.

`engine-spec`: new `WidgetSpec::image: Option<ImageSpec>`
(`{src, fit}`), new `ContentFitSpec` (mirrors `ContentFit` exactly),
new `NodeKindSpec::Image`. `node_kind_and_paint` gained a real `Image`
arm — resolves `src` via `resolve_image_src`, decodes through the
`image` crate (pinned identically to `engine-py`'s own choice),
builds `peniko::ImageData`, maps `ContentFitSpec` → `ContentFit`.
`reconcile.rs`'s `node_props_equal` now also compares `image` — a real
correctness fix caught by design review before it shipped: without it,
a real `image.src:`/`fit:` change across a hot-reload, with everything
else unchanged, would have been silently skipped by `patch_node`,
leaving the old image on screen.

New `engine-spec` tests (6): a real `kind: Image` builds a real node
with the real decoded image and `ContentFit`; a missing `fit:` defaults
to `Fill`; a missing `image:` block, a missing `base_dir`, a `src:`
escaping `base_dir`, and an undecodable file are each a real, distinct,
clear `SpecError`, not a panic. New `engine-render` pixel tests (2):
`Contain` genuinely letterboxes (background visible in the bars);
`Cover` genuinely fills with zero background showing through — the
same real image, same non-square box, opposite real geometry. New
`tests/test_image.py` tests for `fit` (each real value accepted, an
unknown one rejected). New `examples/declarative_image.py` — a real
`kind: Image` widget, loaded through `View`, its node genuinely
reachable.

**Real, corrective finding, caught by a real CI failure, not locally:**
an earlier version of `tests/test_image.py` generated its test PNG via
Pillow — installed in this session's own dev `.venv` for unrelated
reasons, but absent from both `pyproject.toml` and `ci.yml`'s own `pip
install` step, so CI's Linux job failed with `ModuleNotFoundError: No
module named 'PIL'`. Fixed by embedding a real, tiny base64-encoded PNG
instead — the same dependency-free approach `examples/image.py`
already used, extended to `examples/declarative_image.py` too.

Full `cargo test --workspace --release` (`engine-core` 139, `engine-
spec` 52 up from 46, new `engine-render` content-fit tests), `cargo
clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
all clean. `maturin develop --release` + `pytest tests/` (177 passed,
up from 172, 1 pre-existing skip) and all thirty examples run
headlessly — the same two pre-existing, unrelated `on_complete`
failures already flagged as a separate task, no new regressions.

M22 — Real Image Rendering is now fully complete.
