# Plan: Phase 5, Step 5.1.2 -- Canvas Text Rendering (`draw_text`)

## Scope decisions

**A concrete, minimal signature instead of DESIGN.md's abstract sketch.**
DESIGN.md's own sketch (`Canvas::draw_text(&DynamicTextLayout, &Point,
&Paint)`) references `DynamicTextLayout` and `Paint` types that don't
exist anywhere in the codebase -- no rich text-layout engine or paint/
brush abstraction has been built, and building either would be a large,
separate undertaking unrelated to this step's actual goal (wiring
`tre-engine` into `tre-text`/`tre-atlas` for the first time). Matching
5.1.1's own precedent of adapting doc sketches to what's concretely
buildable, `draw_text` instead takes an already-shaped `tre_text::
ShapedRun` (Step 4.1's real output), a resolved `skrifa::FontRef`, and a
plain `u32` packed color -- no new abstraction layers.

**Single already-shaped run, single pre-resolved font -- no cascade, no
bidi, no multi-run paragraphs in this step.** `tre_text::FontCascade`/
`resolve_run` (font-fallback selection) and `unicode_bidi`-driven
multi-run reordering are both real, already-built machinery (Phase 4),
but wiring a whole paragraph of mixed-direction, mixed-font text through
`Canvas` is a distinct, separable concern from proving the core
atlas-backed glyph-quad mechanism works at all. This step's `draw_text`
renders exactly one `ShapedRun` against exactly one caller-supplied font;
a caller wanting fallback or multi-run text calls `resolve_run`/
`segment_runs` itself (already possible today) and invokes `draw_text`
once per resulting run. Multi-run/cascade-aware text is deferred to a
later step if a real caller ever needs it.

**Atlas and font resources are borrowed parameters, never owned by
`Canvas`.** ARCHITECTURE.md is explicit that the dynamic texture atlas
is a single global resource ("the global `RhiDevice` owns one dynamic
texture atlas shared by every window"), while `RenderingCanvas` is a
per-frame, transient recorder (`flatten(self)` consumes `self` every
frame). `draw_text` therefore takes `&tre_atlas::AtlasOwnerHandle` (already
`Clone`-able and safe to share across threads/frames) and the atlas's
current bindless GPU texture index as plain borrowed/copy arguments, the
same way `draw_rounded_rect` never owns a texture either.

**Whitespace/no-ink glyphs are filtered by outline emptiness, not by
inspecting the source text.** `ShapedGlyph` carries a `cluster: u32`
byte offset but not the character itself, and `tre_text::msdf`'s own doc
comment already establishes the relevant fact: a glyph with no visible
ink (space, zero-width joiner, etc.) decomposes to zero contours via
`glyph_outline` -- this is "the common case for e.g. U+0020 SPACE,"
not an error. So no new text-to-glyph char lookup is needed: a glyph
whose extracted outline has zero contours is skipped before it ever
touches the atlas (no `request_insert`, no rendered quad, pen still
advances) -- this is also what keeps `RasterSource::rasterize()`'s
non-optional `Vec<u8>` signature never needing to see an empty/`None`
case, since `generate_msdf`'s `Option<MsdfBitmap>` is only ever a real
error at that point (a genuinely non-empty outline that still fails to
rasterize), not the everyday whitespace case.

**Cache miss -> fire-and-forget request, render nothing this frame.**
Per `AtlasOwnerHandle::lookup`'s own documented contract, "not yet
requested," "requested but not yet processed," and "evicted" are all
indistinguishable from a reader's perspective, and DESIGN.md Section
2.6's placeholder-glyph fallback already anticipates this: "use a
placeholder this frame, re-check/re-request later." This step's
`draw_text` implements the honest, minimal half of that contract --
call `request_insert` once, draw nothing for that glyph this frame --
and treats an actual visible placeholder swatch (a gray box standing in
for a not-yet-resolved glyph) as a follow-on visual-polish concern, not
required to prove the wiring itself works.

**Promote the demo-only `GlyphRasterSource` glue into real, reusable
code in `tre-text`.** `atlas_concurrency_demo.rs` and
`atlas_eviction_demo.rs` each currently duplicate an ad hoc struct
wrapping `Vec<tre_text::Contour>` + `tre_text::generate_msdf` behind
`tre_atlas::RasterSource`. `draw_text` needs exactly this glue as real,
non-demo code, so it moves into `tre-text` as a new, small
`pub struct GlyphRasterSource` implementing `tre_atlas::RasterSource` --
requiring `tre-text` to gain a new dependency on `tre-atlas`. This does
not violate `tre-atlas`'s own "never depends on `tre-text`" precedent
(ARCHITECTURE.md/`tre-atlas/src/raster.rs`'s own doc comment: the atlas
stays content-agnostic via the `RasterSource` trait object): the new
edge points the opposite direction, `tre-text -> tre-atlas`, exactly
the shape `RasterSource` was designed to allow. Both existing demos are
updated to use the promoted type instead of their own local copies,
removing the duplication rather than leaving a third copy alongside it.

**Fixed-size square glyph quads, same simplification the existing,
already-verified `atlas_concurrency_demo` uses -- true per-glyph
metric-accurate quad sizing is out of scope.** `generate_msdf` fits each
glyph's own bounding box into a fixed `size x size` canvas with a
per-glyph scale factor (larger bbox dimension against the available
`size - 2*range_px` content area) that is *not* returned as metadata
today -- computing it would require exposing `msdf.rs`'s private
`bounding_box`/`apply_fit_transform` fit math as new public API, a
correctness-sensitive geometry change unrelated to this step's actual
goal. `draw_text` instead renders every resolved glyph as a fixed
`px_size x px_size` on-screen square (matching the MSDF canvas's own
fixed square aspect exactly), anchored with its bottom edge on the
baseline and horizontally centered on the shaped pen position plus the
glyph's `x_offset`/`y_offset` (scaled by `px_size / units_per_em`) --
consistent, simple, and sufficient to prove real shaped, atlas-backed
text renders correctly on screen, at the cost of not perfectly matching
each glyph's true design-space bounding box. Documented here as a known,
deliberate limitation, not silently accepted.

**Pen advance ignores run direction, matching `text_shaping_demo`'s own
already-established pattern.** `ShapedGlyph::x_advance`/`y_advance` are
summed directly (`pen_x += glyph.x_advance as f32 * scale`) regardless
of `ShapedRun::direction`; correct RTL pen-direction handling is
deferred, and this step's own tests/demo use LTR (Latin) text only.

## Goal

`Canvas::draw_text` renders one already-shaped `ShapedRun` as a sequence
of real, atlas-backed MSDF glyph quads: each glyph is looked up in the
shared atlas, a cache hit emits a real textured `DrawGeometry` command
(current transform/alpha/clip state applied exactly as
`draw_rounded_rect` already does), a cache miss fires a real
`request_insert` and renders nothing for that glyph this frame, and a
zero-ink glyph (whitespace) is skipped entirely -- proven by unit tests
against a fake in-process atlas/font setup and a new real GPU demo that
shapes and renders a real word end-to-end, reading back actual pixels
that confirm the word visibly rendered.

## Tasks

1. **Promote `GlyphRasterSource` into `tre-text`** (new file
   `crates/tre-text/src/raster.rs`, re-exported from `lib.rs`): a
   `pub struct GlyphRasterSource { pub contours: Vec<Contour> }`
   implementing `tre_atlas::RasterSource` (`size()` returns a caller-
   chosen fixed `(u32, u32)`; `rasterize()` calls `generate_msdf` and
   `.expect()`s a `Some` -- callers are responsible for only constructing
   this type for glyphs already confirmed to have real ink, exactly as
   `draw_text` itself will do). Add `tre-atlas = { path = "../tre-atlas"
   }` to `tre-text/Cargo.toml`. Update `atlas_concurrency_demo.rs` and
   `atlas_eviction_demo.rs` to import and use this instead of their own
   local struct.

2. **New `tre-engine` dependencies**: add `tre-text = { path =
   "../tre-text" }`, `tre-atlas = { path = "../tre-atlas" }`, and
   `skrifa` (pinned to the same `=0.33.2` `tre-text` uses) to
   `tre-engine/Cargo.toml`.

3. **`Canvas::draw_text` method**, roughly:
   ```rust
   pub fn draw_text(
       &mut self,
       shaped: &tre_text::ShapedRun,
       font: &skrifa::FontRef,
       font_id: u32,
       origin: [f32; 2],
       px_size: f32,
       rgba: u32,
       atlas: &tre_atlas::AtlasOwnerHandle,
       atlas_texture_handle: u32,
       current_frame: u64,
   )
   ```
   Per glyph: compute `units_per_em` once via `skrifa::MetadataProvider::
   metrics`; `scale = px_size / f32::from(units_per_em)`; advance a
   running `pen` by `x_advance`/`y_advance * scale`; build `AtlasKey::
   from_glyph(font_id, glyph.glyph_id)`; `atlas.lookup(key,
   current_frame)`:
   - **Hit**: emit a `DrawGeometry` command with a `px_size`-square quad
     (four `UiVertex`s, current `state.transform` applied to
     `position`, `uv` from the returned `PackedRect`/atlas dimensions
     exactly like the existing demo's own UV-normalization pattern,
     color via the existing `premultiply_alpha`, `texture_handle:
     atlas_texture_handle`, a new distinct `pipeline_state_id` constant
     marking the MSDF pipeline (`0` stays the SDF-rect pipeline's
     implicit id), `clip_bounds` from the clip stack exactly as
     `draw_rounded_rect` already does).
   - **Miss**: `tre_text::glyph_outline(font, skrifa::GlyphId::from(
     glyph.glyph_id))`; empty/absent outline -> skip (no atlas touch,
     no quad); non-empty -> `atlas.request_insert(key, Box::new(
     tre_text::GlyphRasterSource { contours }), current_frame)`
     (ignore the `bool` return -- "report, don't block," no retry this
     frame), still render nothing.

4. **A new `pipeline_state_id` constant** (e.g. `pub const
   PIPELINE_MSDF_TEXT: u16 = 1;` alongside a documented `0` for the
   existing SDF-rect pipeline) -- the first real distinction between
   pipeline ids in the IR, though nothing downstream consumes it yet
   (same "nothing consumes this yet" honesty already established for
   `sort_key: 0`).

5. **Unit tests**, this crate's own hand-computed-expected-value style:
   - A `ShapedRun` with two real, distinct glyph ids against a fake
     `AtlasOwnerHandle` pre-seeded (via `request_insert` + polling
     `lookup`, same pattern `atlas_concurrency_demo` already
     establishes) so both glyphs are cache hits -- asserts two
     `DrawGeometry` commands with the expected transformed positions
     and pen-advance spacing.
   - A glyph never requested (`lookup` returns `None`) -- asserts
     `request_insert` was called (observable via a real, unseeded
     handle: a subsequent `lookup` after polling resolves) and that
     zero `DrawGeometry` commands were emitted for it this call.
   - A whitespace glyph (real space character, real font) -- asserts no
     atlas interaction at all and correct pen advance past it.
   - `save`/`transform`/`set_alpha`/`push_clip` around `draw_text`
     compose exactly as they already do for `draw_rounded_rect` (reusing
     a cache-hit glyph so the emitted quad's transformed/alpha/clip
     values are directly checkable).

6. **New demo** (`crates/tre-rhi-vulkan/examples/canvas_draw_text_demo.rs`,
   `demo/phase5_step5_1_2/`): a real cascade font (`FontCascade::
   discover`), a real shaped word (`shape_text`/`segment_runs` ->
   `resolve_run`), a real `AtlasOwner` background thread, `Canvas::
   draw_text` called once for the whole run, `flatten()`, the resulting
   commands translated into real vertex/index buffers and drawn via the
   existing, unmodified `bindless_textured.vert`/`msdf.frag` pipeline
   (already proven by `atlas_concurrency_demo`, no shader/RHI changes
   needed) -- reading back real pixels to confirm the word actually
   rendered (non-background fill within each glyph's own on-screen
   quad, same scanning-a-whole-quad-not-just-its-center precedent
   `atlas_concurrency_demo` already established for glyphs with open
   counters).

7. **Docs**: IMPLEMENTATION.md Step 5.1.2 subsection; REVIEW.md entry
   for anything found during implementation; ARCHITECTURE.md only if a
   new convention surfaces beyond what's already planned here.

8. **CI**: add `canvas_draw_text_demo` to the `vulkan-validation` job's
   example list.

## Verification plan

- `cargo fmt` / `clippy -D warnings` / `build` / `test` clean across the
  workspace.
- `canvas_draw_text_demo` run under `VK_LAYER_KHRONOS_validation`, zero
  errors.
- All pre-existing examples re-run manually against real Vulkan
  hardware (the promoted `GlyphRasterSource` refactor touches
  `atlas_concurrency_demo`/`atlas_eviction_demo` -- both must still pass
  their own existing assertions unchanged).
- CI: push, confirm green via `gh run watch`.

## Explicitly out of scope for this sub-step

- `DynamicTextLayout`/`Paint` or any richer text-layout/brush
  abstraction -- `draw_text` takes a single already-shaped `ShapedRun`
  and a plain packed color.
- Font-fallback cascade resolution and multi-run/bidi paragraph
  handling inside `draw_text` itself -- callers already can (and must)
  call `resolve_run`/`segment_runs` themselves per run.
- A real visible placeholder swatch for a cache-miss glyph -- renders
  nothing this frame, per DESIGN.md Section 2.6's fallback contract's
  minimal honest reading.
- True per-glyph metric-accurate quad sizing/anchoring (exposing
  `msdf.rs`'s internal fit-transform math) -- fixed `px_size`-square
  quads only, same simplification `atlas_concurrency_demo` already uses.
- RTL/vertical pen-advance direction handling.
- The real 64-bit sort key, `begin_overlay`, and real batch flattening
  (`sort_key: 0` stays a placeholder) -- Step 5.1.3.
- Any change to `tre-rhi-vulkan`'s pipeline/shader code -- reuses the
  existing `bindless_textured.vert`/`msdf.frag` pipeline exactly as
  `atlas_concurrency_demo` already does.
