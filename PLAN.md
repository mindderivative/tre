# Plan: M23 Phase 1 — Real Icon Rendering Primitive & Starter Icon Set (§1, §3)

Corresponds to `BUILD_TRACKER.md` M23 Phase 1: `NodeKind::Icon(IconState)`,
a real MD3 vector icon painted as a solid-tint fill, plus a small,
real curated set of genuine Material Symbols icons and
`Window.add_icon`.

## Investigation before writing code

- ARCHITECTURE.md §1 Locked Decisions: "MD3's own icon set embedded as
  `kurbo::BezPath` data at build time; `usvg`+`vello_svg` reserved for
  user-supplied custom SVG only" — confirmed via direct read, this is
  a real, always-intended design, not invented. `engine_core::NodeKind`
  has no `Icon` variant, confirmed via direct read; no crate in this
  workspace depends on `usvg`/`vello_svg`, confirmed via grep.
- **Real, scope-simplifying finding, verified empirically (a real
  network fetch, not assumed):** a real Material Symbols icon,
  fetched directly from `fonts.gstatic.com` (Google's own CDN; eight
  distinct icons checked — `home`/`search`/`menu`/`close`/`check`/
  `arrow_back`/`add`/`settings`), is a plain, single `<path
  d="...">` SVG with a fixed `viewBox="0 -960 960 960"` — no text, no
  clipping, no masking, no filters, no patterns, no group opacity, no
  gradients, not even a `<g transform>`. Every one of `vello_svg`'s
  own documented gaps (§15 Risk Register) is therefore structurally
  irrelevant to this specific, simple icon format.
- **Real consequence: zero new dependencies needed, not `usvg`.**
  `peniko::kurbo::BezPath::from_svg(&str) -> Result<BezPath,
  SvgParseError>` (`kurbo = "0.13.1"`, already pinned transitively via
  `peniko`, confirmed via direct source read of the vendored crate)
  parses an SVG path's own `d=` attribute directly into a real,
  paintable `BezPath`. Painted through the ordinary `scene.
  set_paint`/`fill_path` mechanism every solid-color fill already
  uses — a vector path fill, not raster pixel data, so none of
  `Image`'s own real GPU-texture-cache complication (M22 Phase 1)
  applies here.
- **Crate placement, following established precedent:** `NodeKind::
  Icon` lives in `engine-core`, mirroring `Checkbox`/`Slider`/
  `TextField` (all real MD3 components whose `NodeKind` variant still
  lives in `engine-core`, not `engine-md3` — "MD3-agnostic" means
  `engine-core` never hardcodes MD3 *theme* resolution, not that MD3-
  shaped components can't have their `NodeKind` there). The curated
  icon *data* (real MD3-specific content) belongs in `engine-md3`
  instead, a new `icons` module — the identical "engine-core holds
  the generic mechanism, engine-md3 holds the MD3-specific data/
  resolution" split `DynamicTheme`/`ColorScheme` already established
  for color.
- **Where parsing happens:** `Window.add_icon` (`engine-py`) looks up
  the curated `d=` string via `engine_md3::icons::path_for(name)`,
  parses it via `BezPath::from_svg` once, at construction — the same
  "resolved ahead of time, not computed live" split `ImageState`/
  `CanvasState` already established. No lazy-static/cache needed:
  parsing a ~200-byte SVG path string once, at `add_icon` call time,
  costs nothing worth optimizing yet (`Window.add_image`'s own file-
  decode step is a strictly heavier real operation done the same
  simple way).
- **The real 960-unit viewBox → node-box transform:** every curated
  icon shares the identical `viewBox="0 -960 960 960"` (x: 0..960, y:
  -960..0) — a real, fixed MD3 convention, not a per-icon variable, so
  a single constant (`engine_core::ICON_VIEWBOX_SIZE: f64 = 960.0`)
  suffices. `paint_node`'s new `Icon` arm temporarily changes the
  active scene transform to `composed * scale(w/960, h/960) *
  translate(0, 960)` (translate first, to bring the real y range into
  `0..960`, then scale into the node's own local `(0,0)-(w,h)` box),
  fills the path, then restores `composed` — required because
  `Scene::fill_path` always draws in whatever transform is currently
  active (unlike `Image`'s own `draw_texture_rects`, whose `SampleRect.
  transform` is a real, separate per-call parameter needing no such
  restore) and the post-match ripple/hover overlay code relies on
  `composed` still being active afterward.
- **Real, deliberate scope boundary:** a small, curated starter set
  (the eight icons above — common, broadly useful, and enough to prove
  the real mechanism end-to-end) rather than the full multi-thousand-
  icon Material Symbols library, matching this codebase's own "don't
  build ahead of need" discipline (`DrawCommand`'s own real variant
  set is the direct precedent: "additive whenever a real future need
  asks for more").
- **Real API shape, deliberately narrower than every other `add_*`
  method:** `add_icon(name, color, size, x=None, y=None)` takes a
  single `size` (not `width`+`height`) — Material Symbols icons are a
  real, uniformly square icon system by design (every fetched icon's
  own `height`/`width` attributes are identical), so a single size
  parameter is a genuine ergonomic fit, not an invented shortcut;
  internally still builds an ordinary square `Size` via the same
  `positioned_style` helper every other `add_*` method uses. An
  unknown `name` is a real, clear `PyValueError`, the same "fail
  loudly at the boundary" pattern `parse_dock_side`/`parse_content_
  fit` already established (not routed through `EngineError`, since
  this is a pure name-lookup failure with no I/O involved, matching
  where those two live too).

## What will change

- `crates/engine-core/src/node.rs`: new `pub const ICON_VIEWBOX_SIZE:
  f64`; new `IconState { path: peniko::kurbo::BezPath, tint:
  peniko::Color }`; new `NodeKind::Icon(IconState)` variant.
- `crates/engine-core/src/lib.rs`: re-export `IconState`,
  `ICON_VIEWBOX_SIZE`.
- `crates/engine-py/src/node.rs`: `kind_name`'s exhaustive match gains
  an `Icon` arm (compiler-required, mirrors every prior `NodeKind`
  addition).
- New `crates/engine-md3/src/icons.rs`: real `d=` path-data constants
  for `home`/`search`/`menu`/`close`/`check`/`arrow_back`/`add`/
  `settings` (fetched directly from `fonts.gstatic.com`, recorded
  verbatim); `pub fn path_for(name: &str) -> Option<&'static str>`.
  Exported from `engine-md3`'s own `lib.rs`.
- `crates/engine-render/src/lib.rs`: new `NodeKind::Icon` paint arm
  (see the transform reasoning above).
- `crates/engine-py/src/window.rs`: new `Window.add_icon(name, color,
  size, x=None, y=None) -> PyResult<Node>`.
- New `engine-core` unit test: `NodeKind::Icon` round-trips through
  `Tree::insert`/`Tree::get`.
- New `crates/engine-render/tests/icon_paint.rs`: a real, synthesized
  simple square `BezPath` (not a fetched icon — a hand-built shape
  with a known, real filled/empty region) painted through the real
  transform, confirming a point inside the shape shows the real tint
  and a point outside shows plain background — the same headless
  pixel-readback discipline `checkbox_paint.rs`/`image_paint.rs`
  already established.
- New `tests/test_icon.py`: `add_icon` returns a real `Node` for each
  of the eight curated names; an unknown name raises a real
  `ValueError`.
- New `examples/icon.py`: a few real curated icons rendered through a
  full `App.run` loop.

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run every example script, including the new `examples/icon.py`.
