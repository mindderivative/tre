# Log: M23 Phase 1 — Real Icon Rendering Primitive & Starter Icon Set, closing M23 (§1, §3)

Corresponds to `BUILD_TRACKER.md` M23 Phase 1, closing M23 entirely:
`NodeKind::Icon(IconState)`, a real vector icon painted as a solid-
tint fill, plus a small, real curated set of genuine Material Symbols
icons and `Window.add_icon`.

## Investigation before writing code

ARCHITECTURE.md §1 Locked Decisions: "MD3's own icon set embedded as
`kurbo::BezPath` data at build time; `usvg`+`vello_svg` reserved for
user-supplied custom SVG only." `engine_core::NodeKind` had no `Icon`
variant, confirmed via direct read; no crate in this workspace
depended on `usvg`/`vello_svg`, confirmed via grep.

**Real, scope-simplifying finding, verified empirically, not
assumed:** a real network fetch of eight distinct Material Symbols
icons directly from Google's own CDN (`fonts.gstatic.com`) confirmed
every one is a plain, single `<path d="...">` SVG with a fixed
`viewBox="0 -960 960 960"` — no text, clipping, masking, filters,
patterns, group opacity, or even a `<g transform>`. Every one of
`vello_svg`'s own documented gaps (§15 Risk Register) is therefore
structurally irrelevant to this specific, simple icon format. Real
consequence: **zero new dependencies**, not `usvg` — `peniko::kurbo::
BezPath::from_svg(&str)` (already pinned via `peniko`, confirmed via
direct source read of the vendored `kurbo 0.13.1`) parses an SVG
path's own `d=` attribute directly into a real, paintable `BezPath`.

Crate placement follows the established `Checkbox`/`Slider`/
`TextField` precedent exactly: `NodeKind::Icon` lives in `engine-core`
("MD3-agnostic" means never hardcoding MD3 *theme* resolution there,
not excluding MD3-shaped components); the curated icon *data* lives
in `engine-md3` instead, the identical "generic mechanism in
engine-core, MD3-specific data/resolution in engine-md3" split
`DynamicTheme`/`ColorScheme` already established for color.

## What happened

New `engine_core::ICON_VIEWBOX_SIZE: f64 = 960.0` (every curated icon
shares this identical real viewBox dimension) and `IconState { path:
peniko::kurbo::BezPath, tint: Color }`, mirroring `ImageState`'s own
"already-resolved, inert paint data" shape — parsing happens once, at
`Window.add_icon` call time (`engine-py`), not in `engine-core`.

New `engine-render` `NodeKind::Icon` paint arm: temporarily changes
the active scene transform to `composed * scale(w/960, h/960) *
translate(0, 960)` (the translate brings the real negative-`y` SVG
range into `0..960` first, then the scale maps it into the node's own
local box), fills the path with `state.tint` (multiplied by the
node's own real `PaintProperties.opacity`, the same universal handling
every other fill already gets), then restores `composed` — required
because `Scene::fill_path` always draws in whatever transform is
currently active (unlike `Image`'s own `draw_texture_rects`, whose
`SampleRect.transform` is a real, separate per-call argument) and the
post-match ripple/hover overlay code relies on `composed` still being
active afterward. Verified correct on the first real test run via a
deliberately asymmetric hand-built "left half of the viewBox" shape —
a wrong transform would have painted neither half, not just the wrong
one, so this is a genuine geometry proof.

New `crates/engine-md3/src/icons.rs`: eight real, verbatim `d=` path
strings fetched directly from `fonts.gstatic.com` (`home`/`search`/
`menu`/`close`/`check`/`arrow_back`/`add`/`settings`), `path_for(name)
-> Option<&'static str>`, `names()` iterator. New `Window.add_icon
(name, color, size, x=None, y=None)` — deliberately takes one square
`size`, not `width`+`height` (Material Symbols icons are a real,
uniformly square system by design, confirmed across all eight fetched
icons' own identical `width`/`height` SVG attributes), and no
`background` param at all (mirroring `add_canvas`/`add_image`'s own
real precedent for a kind with no meaningful separate background). An
unknown `name` is a real, clear `PyValueError` listing the real known
set, the same `parse_dock_side`/`parse_content_fit` "fail loudly at
the boundary" pattern, not routed through `EngineError` (a pure
name-lookup failure, no I/O involved).

New `engine-md3` tests (2): every curated icon's own real path data
parses as a real `BezPath`; an unknown name returns `None`. New
`engine-core` test: `NodeKind::Icon` round-trips through `insert`/
`get`. New `crates/engine-render/tests/icon_paint.rs`: the real,
decisive left-half-only geometry proof described above. New
`tests/test_icon.py` (10 tests): every one of the eight real curated
icons builds a real `Node`; an unknown name raises a real `ValueError`.
New `examples/icon.py`: four real curated icons, different real
sizes/tints, rendered through a full real `App.run` loop.

**Real, deliberate scope boundary:** a small, curated eight-icon
starter set, not the full multi-thousand-icon Material Symbols
library — additive to grow later exactly the way `DrawCommand`'s own
real variants have only ever grown when a real need asked for more.

Full `cargo test --workspace --release` (`engine-core` 140, up from
139; `engine-md3` 11, up from 9; new `engine-render` `icon_paint.rs`),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check` all clean. `maturin develop --release` + `pytest tests/` (187
passed, up from 177, 1 pre-existing skip) and all thirty-two examples
run headlessly — the same two pre-existing, unrelated `on_complete`
failures already flagged separately (`task_a5249ecc`), no new
regressions.

M23 — MD3 Icon Pipeline is now fully complete.
