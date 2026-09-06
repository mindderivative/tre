# Plan: Phase 4, Step 4.1 -- HarfBuzz & FreeType Integration

## Scope decisions (confirmed with the project owner, 2026-09-06)

**All-pure-Rust font stack: `rustybuzz` for shaping, `skrifa` for outline
extraction -- no HarfBuzz or FreeType C library anywhere in this
workspace**, a deliberate departure from IMPLEMENTATION.md's literal
wording ("HarfBuzz... via the `harfbuzz_rs` binding crate, or raw FFI" /
"FreeType (`FT_Outline_Decompose`)... via the `freetype` binding crate").
Both real C-linked options were confirmed available on this machine
(system HarfBuzz 14.3.1 and FreeType 26.6.20 via `pkg-config`, `harfbuzz_rs`
2.0.1 built on the actively Servo-maintained `harfbuzz-sys` 0.8.0), so this
isn't "the real thing wasn't viable" -- it's a project-owner call to keep
the workspace's existing all-Rust dependency story (`wide`, `usvg`, `ash`'s
own Vulkan loader are the only things that touch a C ABI, and Vulkan is
the one boundary this project can't avoid) rather than introduce a new
class of C build dependency and CI system-package step for text. `skrifa`
(part of Google Fonts' `fontations` project, `rust-version = 1.75` --
matching this workspace exactly) replaces FreeType for outline extraction;
`rustybuzz` (a complete, faithful port of HarfBuzz's own shaping algorithm,
already the crate `tre-svg`'s own `Cargo.toml` comment anticipated as
`usvg`'s disabled `text` dependency) replaces HarfBuzz for shaping. Neither
crate needs `unsafe` at this project's call sites, so the new crate keeps
`#![forbid(unsafe_code)]`.

Both `rustybuzz` and `skrifa` parse the same font file bytes independently
(via their own respective font-parsing sublibraries, `ttf-parser` and
`read-fonts`) -- an accepted, common redundancy in real text-rendering
stacks (e.g. Chrome itself runs HarfBuzz and FreeType as two independent
parsers over the same font file), not something this step tries to
unify.

**Font fallback cascade uses real `fontconfig`-driven system discovery,
not a hardcoded path list**, per the project owner's direction, following
Phase 1's Linux-first precedent (Windows/macOS system font APIs --
DirectWrite, Core Text -- explicitly deferred). This machine already has a
real, rich fontconfig install (5,252 fonts registered via `fc-list`,
including DejaVu Sans and a large Noto family covering non-Latin scripts)
to develop and test against. The `fontconfig` crate (YesLogic-maintained,
wraps the system `libfontconfig`) queries real installed families to build
an ordered cascade (primary UI sans -> a broad-coverage fallback -> an
emoji font), rather than asserting a specific font is present by
hardcoded path.

**New crate `tre-svg`-style: `tre-text`**, not code inside `tre-engine` or
`tre-svg`. Matches this project's own precedent (`tre-math`, `tre-svg`
each got a new crate when a new capability domain arrived) and DESIGN.md's
architecture diagram, which already treats text/typography as its own
concern distinct from vector path tessellation. Depends on `rustybuzz`,
`skrifa`, `unicode-bidi` (paragraph-level bidi run segmentation --
HarfBuzz/rustybuzz shape one direction- and script-uniform run at a time,
so segmenting a mixed-direction string into runs is the caller's job, not
rustybuzz's), `unicode-script` (real UAX #24 script-property lookups to
split runs by script, instead of a hand-rolled Unicode-block heuristic --
same "use a mature library for the solved sub-problem" reasoning `usvg`
got in Step 3.3.1), `fontconfig` (`#[cfg(target_os = "linux")]`-gated,
mirroring Phase 1's platform-gating precedent), and `tre-svg` (only for
this step's demo/verification, to reuse the existing, already-hand-rolled
Bezier-flattening + ear-clipping + Vulkan flat-color rendering path rather
than re-deriving a second curve-flattener -- see task 5). `tre-svg`'s
`flatten` module is currently private; this step makes its
cubic/quadratic-to-polyline functions `pub` and re-exports them from
`tre-svg`'s crate root, the same small, additive, non-breaking pattern
Step 3.3.3 used when it made `stencil` its own `pub` module.

**Not split into sub-steps.** IMPLEMENTATION.md's three tasks here (real
HarfBuzz-equivalent shaping with bidi, a font fallback cascade, real
FreeType-equivalent outline extraction) are comparably scoped to a single
normal step (Step 2.1's bindless textures, Step 2.3's background-thread
GC) -- not the "four largely independent chunks, comparable to all of
Phase 2" situation that justified splitting Step 3.3 into 3.3.1-3.3.3.
They're also naturally demonstrated together: a single demo shapes a real
mixed-script string (exercising bidi + fallback in one pass) and extracts
one resolved glyph's outline (exercising task 3) from whichever real font
the cascade or shaping actually resolved.

**No MSDF rasterization, no texture atlas, and no wiring into
`RenderingCanvas`'s public API this step.** IMPLEMENTATION.md explicitly
carves those out as Step 4.2 (MSDF Rasterizer & Atlas Packing) and later
Phase 5 work respectively. This step's job, per its own task list, stops
at: shaped glyph clusters with real advances/offsets, a real fallback
font selected when the primary lacks a glyph, and raw outline control-
point data for a resolved glyph -- the exact inputs Step 4.2's MSDF
generator and atlas packer will need, not a rasterizer of its own.

**Verifying outline extraction reuses the existing tessellation +
Vulkan pipeline, deliberately staying within what's already proven safe.**
A glyph with true holes (e.g. 'O', 'B', 'e') has multiple contours with
opposing winding -- correctly rendering that needs either true multi-
contour hole subtraction (not built) or extending Step 3.3.3's
stencil-and-cover pipelines to accumulate winding across more than one
`Polygon` (a real, separate piece of work, not this step's task 3 at all
-- MSDF rendering in Step 4.2 handles contours/winding natively via the
distance-field algorithm itself, without triangulation). This step's demo
therefore deliberately picks a **hole-free** glyph (e.g. an 'L', a 'V', or
a numeral) so its outer contour alone is a valid input to the existing,
unmodified ear-clipping tessellator and flat-color Vulkan pipeline --
proving the extracted control points are geometrically correct and
correctly wound, without taking on multi-contour rendering as scope creep.

## Goal

Given a real string containing both LTR and RTL text, produce correctly
shaped glyph runs (advances, offsets, cluster mapping, visual reordering)
using a real system font resolved through a real fallback cascade when the
primary font lacks a glyph -- and, for one resolved, hole-free glyph,
extract its true outline control points from the real font file and
render that exact shape through the existing GPU pipeline, proven by
reading back real pixels, matching this project's established
verify-before-trusting methodology.

## Tasks

1. **New `tre-text` crate** (`crates/tre-text`), added to the workspace
   `Cargo.toml`. `#![forbid(unsafe_code)]`. Dependencies: `rustybuzz`,
   `skrifa`, `unicode-bidi`, `unicode-script`, `fontconfig` (Linux-only via
   `[target.'cfg(target_os = "linux")'.dependencies]`), `tre-svg` (dev-only
   or a feature-gated demo dependency -- exact placement decided during
   implementation based on whether the reused `flatten` functions are
   needed outside the demo).

2. **Bidi + script run segmentation and shaping**
   (`shape_text(font: &rustybuzz::Face, text: &str) ->
   Vec<ShapedRun>`, exact name/shape TBD during implementation): resolves
   paragraph embedding levels via `unicode-bidi`, splits further by
   `unicode-script`'s per-character `Script` property, and shapes each
   resulting (level, script)-uniform run independently through
   `rustybuzz::shape` with the correct `Direction`/`Script` set on the
   buffer. Each `ShapedRun` carries the shaped glyph IDs, advances,
   offsets, and cluster (byte-index) mapping rustybuzz returns, plus the
   run's resolved direction -- visual (not logical) run order for a mixed-
   direction paragraph is derived from the bidi levels, not assumed to
   match logical string order.

3. **Font fallback cascade**
   (`FontCascade::discover() -> Result<FontCascade, TextError>` on Linux,
   backed by `fontconfig`): builds an ordered list of loaded font files by
   querying the system for a default UI sans family, then a broad-coverage
   fallback family, then a color emoji family. `resolve_run(cascade,
   run_text) -> (font_index, ShapedRun)` (exact shape TBD): shapes against
   the primary font first; if the shaped output contains the notdef glyph
   (glyph ID 0) for any cluster, re-shapes that run against the next
   cascade entry, continuing down the list -- real fallback behavior, not
   a cosmetic pass-through, verified by deliberately shaping a codepoint
   the primary/UI font doesn't cover (e.g. an emoji or a non-Latin script
   character) and asserting the resolved font actually changed and the
   resulting glyph ID is non-zero.

4. **Glyph outline extraction**
   (`glyph_outline(font: &skrifa::FontRef, glyph_id: skrifa::GlyphId) ->
   Result<Vec<Contour>, TextError>`, exact shape TBD): drives `skrifa`'s
   outline-drawing API with a custom `OutlinePen` implementation that
   records `MoveTo`/`LineTo`/`QuadTo`/`CurveTo`/`Close` commands into an
   owned `Contour` (a sequence of straight/quadratic/cubic segments,
   mirroring what `FT_Outline_Decompose`'s callbacks would have produced)
   -- the exact control-point data Step 4.2's MSDF generator will need as
   its input, this step only extracts and returns it.

5. **New example**
   (`crates/tre-rhi-vulkan/examples/text_shaping_demo.rs`,
   `demo/phase4_step4_1/`): discovers a real fallback cascade; shapes a
   short string mixing Latin text with a non-Latin/RTL script (e.g. Arabic
   or Hebrew) using the primary font, asserting the RTL portion's visual
   glyph order is actually reversed relative to logical order (independent
   ground truth worked out by hand -- same rigor as every prior step's
   pixel-probe methodology, just applied to shaping output instead of
   pixels this time); shapes a codepoint absent from the primary font
   (proving real fallback resolution, not a no-op cascade); extracts one
   resolved, hole-free glyph's outline via `tre-text`, flattens its
   curves via `tre-svg`'s now-`pub` flatten functions, triangulates and
   renders it through the existing (unmodified) ear-clipping + flat-color
   Vulkan pipeline, and reads back real pixels confirming the rendered
   shape matches the glyph (a handful of probe points worked out from the
   real font's own outline data before writing the pixel assertions, not
   guessed).

6. **Unit tests** in `tre-text`: bidi+script run segmentation on a known
   mixed-direction string (asserting the exact run boundaries and
   directions produced); fallback resolution choosing a different cascade
   entry when the primary lacks coverage; outline extraction against a
   glyph with a hand-verifiable point count/contour count (e.g. a simple
   glyph in a well-known open font already present on this system).

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace, including `tre-text`'s own `#![forbid(unsafe_code)]`.
- `text_shaping_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors -- it reuses the existing, unmodified flat-color pipeline, so
  this mostly confirms no regression rather than exercising new RHI
  surface area (this step touches no `tre-rhi-vulkan` code at all).
- All 10 pre-existing Vulkan examples re-run manually, unaffected.
- CI: add `libfontconfig1-dev` (build-time headers for the `fontconfig`
  crate) and concrete font packages covering the demo's Latin/RTL/emoji
  test text (exact package names confirmed during execution against the
  CI image, e.g. `fonts-dejavu-core` plus a small Arabic-capable and
  emoji-capable package) to the `apt-get install` steps that need them, so
  fontconfig's live discovery resolves deterministically in CI too, not
  just on this dev machine's already-rich font install. Add
  `text_shaping_demo` to the `vulkan-validation` job's example list; push,
  confirm green.

## Explicitly out of scope for this step

- MSDF rasterization and Guillotine atlas packing (IMPLEMENTATION.md Step
  4.2) -- this step produces the shaped-glyph and outline-control-point
  data Step 4.2 consumes, nothing further.
- True multi-contour hole rendering (a glyph like 'O' or 'B') -- deferred
  to however Step 4.2's MSDF approach (which handles winding/contours
  natively) or a future stencil-and-cover extension ends up handling it;
  this step's demo deliberately uses a hole-free glyph instead of taking
  that on as scope creep.
- Windows (DirectWrite) and macOS (Core Text) system font discovery --
  `FontCascade::discover()` is Linux/`fontconfig`-only this step, matching
  every other Phase 1 platform-gated feature's precedent.
- Wiring shaped text or extracted outlines into `RenderingCanvas`'s public
  `Canvas` API (a `draw_text`/`DrawText` IR command) -- proven directly via
  a dedicated demo first, matching this project's "prove the primitive
  before its real consumer exists" precedent.
- Vertical text, complex OpenType feature toggles beyond rustybuzz's
  defaults (ligatures/kerning are on by default and exercised incidentally
  by the demo text, but not explicitly tested), and font hinting/grid-
  fitting -- MSDF rendering (Step 4.2) is resolution-independent by
  design and doesn't need hinting the way traditional rasterization does.
