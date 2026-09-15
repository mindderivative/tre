//! Phase 12 Step 12.2: `Text` as a first-class, retained-mode
//! `ShapePrimitive` -- dirty-tracked and mutable in place, the same as
//! every other shape, instead of `RenderingCanvas::draw_text`'s own
//! immediate-mode-only path (which a caller must re-shape and re-look-up
//! every single frame regardless of whether the text actually changed).
//! `FontRegistry` owns real font bytes and hands back cheap, freshly
//! reconstructed `skrifa::FontRef`/`rustybuzz::Face` views on demand
//! (both are real, cheap table-directory parses, not a full font parse
//! -- see `FontRegistry::font_ref`/`face`'s own doc comments), so
//! storing a `FontId` on a `Text` primitive needs no self-referential
//! struct.

use crate::shapes::{Color, Primitive, PrimitiveCommon};
use crate::GlyphAtlasContext;

/// A stable handle into a [`FontRegistry`], returned by
/// [`FontRegistry::load_bytes`]. Scoped to the registry that created it,
/// the same convention [`crate::GradientId`] already establishes for its
/// own append-only table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);

/// [`FontRegistry::load_bytes`]'s own real validation failure -- the
/// font bytes could not be parsed by either `skrifa` (outline/metrics)
/// or `rustybuzz` (shaping), so storing them would only defer a real
/// failure to the first `Text` shape that references this `FontId`, at
/// flatten time, where recovering cleanly is much harder (deep inside a
/// `ShapeRegistry::flatten_into` call, not a caller-facing constructor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FontError {
    #[error("font bytes are not a valid font (rejected by skrifa or rustybuzz)")]
    InvalidFont,
}

/// Owns real, caller-supplied font bytes and assigns each one a stable
/// [`FontId`] -- append-only, no generational reuse, the same real,
/// disclosed scope decision [`crate::GradientId`]'s own table already
/// makes (no real UI use case this step targets discards/reloads fonts
/// at the same churn rate shapes themselves do).
#[derive(Default)]
pub struct FontRegistry {
    entries: Vec<Vec<u8>>,
}

impl FontRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates `bytes` against both real consumers a `Text` primitive
    /// will need -- `skrifa::FontRef` (outline/metrics) and
    /// `rustybuzz::Face` (shaping) -- before storing them, so a caller
    /// gets a real `Err` right here rather than a panic deep inside a
    /// future `flatten_into` call.
    ///
    /// # Errors
    /// See [`FontError`].
    ///
    /// # Panics
    /// Never in practice -- only if this registry has already loaded
    /// more than `u32::MAX` fonts in one session, far beyond any real
    /// use case.
    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> Result<FontId, FontError> {
        skrifa::FontRef::new(&bytes).map_err(|_| FontError::InvalidFont)?;
        if rustybuzz::Face::from_slice(&bytes, 0).is_none() {
            return Err(FontError::InvalidFont);
        }
        let index = u32::try_from(self.entries.len())
            .expect("far fewer fonts than u32::MAX are ever loaded in one real session");
        self.entries.push(bytes);
        Ok(FontId(index))
    }

    /// A fresh, cheap `skrifa::FontRef` view over `id`'s own bytes (a
    /// real table-directory parse, not a full font parse -- safe to call
    /// every frame), or `None` if `id` is out of range.
    #[must_use]
    pub fn font_ref(&self, id: FontId) -> Option<skrifa::FontRef<'_>> {
        self.entries
            .get(id.0 as usize)
            .and_then(|bytes| skrifa::FontRef::new(bytes).ok())
    }

    /// A fresh, cheap `rustybuzz::Face` view over `id`'s own bytes, or
    /// `None` if `id` is out of range. `load_bytes` already validated
    /// these same bytes construct one successfully, so `None` here means
    /// only "id is out of range," never "bytes are invalid."
    #[must_use]
    pub fn face(&self, id: FontId) -> Option<rustybuzz::Face<'_>> {
        self.entries
            .get(id.0 as usize)
            .and_then(|bytes| rustybuzz::Face::from_slice(bytes, 0))
    }
}

/// Everything [`crate::ShapeRegistry::flatten_into`] needs to flatten a
/// [`Text`] shape -- bundles the font table and the same
/// [`GlyphAtlasContext`] `RenderingCanvas::draw_text` already requires,
/// since a `Text`-bearing registry's `flatten_into` call needs both
/// together every time.
pub struct TextFlattenContext<'a> {
    pub fonts: &'a FontRegistry,
    pub atlas: &'a GlyphAtlasContext<'a>,
}

/// A real, retained-mode text primitive (Phase 12 Step 12.2): dirty-
/// tracked and mutable in place via [`crate::ShapeRegistry`], unlike
/// `RenderingCanvas::draw_text`'s own immediate-mode-only path.
/// Positioned the same way every other shape is -- `common.transform.
/// position` is this text's own top-left corner, not its baseline;
/// [`flatten_text`] offsets the real pen position down by the font's own
/// scaled ascent internally, so a caller never has to reason about
/// baseline metrics just to place a text block.
///
/// **Solid fill only, this pass** (a real, disclosed scope boundary,
/// matching this project's own established "solid fill first" sequencing
/// for `Rectangle`/`Circle`/`Polygon`/`Path` in Phase 10 Steps
/// 10.2/10.2.1/10.2.2): `RenderingCanvas::draw_text`'s own real glyph-quad
/// path takes a flat `rgba: u32`, not a [`crate::FillStyle`] -- real
/// gradient/texture-filled text would need new per-glyph UV-mapping work
/// in `draw_text`/`emit_glyph_quad` themselves, not just plumbing here.
#[derive(Debug, Clone)]
pub struct Text {
    pub common: PrimitiveCommon,
    pub text: String,
    pub font: FontId,
    pub px_size: f32,
    pub fill_color: Color,
    /// `None` (the default from [`Text::new`]) keeps this shape on the
    /// exact single-line rendering path every caller before Phase 15
    /// Step 15.1 already relies on -- byte-identical, zero regression.
    /// `Some(width)` enables real multi-line rendering (Phase 15 Step
    /// 15.1): `\n` always breaks a line, and words greedily wrap to fit
    /// `width` -- pass `f32::INFINITY` for hard-wrap-only (break only on
    /// `\n`, no width limit). See [`tre_text::wrap_lines`]'s own doc
    /// comment for the exact algorithm and its real, disclosed v1 scope
    /// (LTR-only, whitespace-boundary wrapping, no hyphenation).
    pub wrap_width: Option<f32>,
}

impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>, font: FontId, px_size: f32, color: Color) -> Self {
        Self {
            common: PrimitiveCommon::new(),
            text: text.into(),
            font,
            px_size,
            fill_color: color,
            wrap_width: None,
        }
    }
}

impl Primitive for Text {
    fn common(&self) -> &PrimitiveCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut PrimitiveCommon {
        &mut self.common
    }
}

/// [`crate::ShapeRegistry::flatten_into`]'s own `Text` dispatch --
/// shapes `text.text` against `text.font`'s real resolved font, then
/// draws each resulting run via `RenderingCanvas::draw_text`, chaining
/// the pen position across runs itself (multi-run text -- e.g. mixed-
/// script/bidi input -- is real but rare; `draw_text` itself only ever
/// advances a pen *within* one run, so this is the one real seam that
/// has to happen here rather than being delegated).
///
/// A real shaping failure (`tre_text::shape_text`'s own `Err`) renders
/// nothing this call, matching `draw_text`'s own established "report,
/// don't block" contract for an unresolved glyph -- not a panic, since a
/// malformed/unshapeable string is real caller input, not a programmer
/// error.
///
/// # Panics
/// Panics if `text.font` was never issued by `context.fonts` (a stale or
/// foreign `FontId`) -- the same real-programmer-error contract
/// `flatten_into`'s own `FillStyle::Gradient` handling already
/// establishes for a stale `GradientId`.
/// [`flatten_text`]'s own per-shape shaping cache (REVIEW.md finding
/// #225): without this, `ShapeRegistry::flatten_into`'s shape-kind-
/// agnostic dirty check ("any active animation re-flattens everything")
/// forced a full bidi + `rustybuzz` re-shape of `text.text` every single
/// frame a `Text` shape had *any* active animation, even one animating a
/// property (opacity, position, scale, ...) that never touches the
/// shaped output at all -- the exact defect this struct's own module doc
/// comment names retained-mode `Text` as existing to eliminate,
/// reappearing through the generic animation path. Keyed by exactly the
/// fields that actually affect shaping; every other field on [`Text`]
/// (`fill_color`, `common.transform`/`opacity`/...) can change freely
/// without invalidating this cache.
#[derive(Debug, Clone)]
pub(crate) struct TextShapeCache {
    text: String,
    font: FontId,
    px_size: f32,
    wrap_width: Option<f32>,
    runs: Vec<tre_text::ShapedRun>,
    /// The wrapped-lines pass's own output, cached alongside `runs` for
    /// the identical reason (`/review-project` Performance finding,
    /// dated 2026-09-13): before this field existed, `flatten_text`
    /// called `tre_text::wrap_lines` unconditionally on every call, even
    /// on a cache *hit* -- so an animated, wrapped `Text` shape (opacity/
    /// position changing every frame, forcing a re-flatten) still paid a
    /// full UAX #14 line-break pass plus two real per-call heap
    /// allocations (`flatten_glyphs` inside `wrap_lines`, and this
    /// module's own `flat_glyphs`) every single frame -- exactly the
    /// waste class this cache exists to eliminate, reappearing one step
    /// after the shaping it already covers. `None` whenever `wrap_width`
    /// is `None` (the single-line path never calls `wrap_lines` at all)
    /// or not yet computed for the current `runs`; always reset to
    /// `None` on a fresh `TextShapeCache` (`resolve_shaped_runs`'s own
    /// re-shape path constructs a brand-new value here, never mutates an
    /// existing one in place), so it can never outlive the `runs` it was
    /// computed from.
    wrapped: Option<WrappedLinesCache>,
}

/// `TextShapeCache::wrapped`'s own payload -- the wrapped lines
/// themselves plus the flat, concatenated glyph list `flatten_text`'s
/// per-line rendering loop slices into, so neither needs recomputing
/// on a cache hit.
#[derive(Debug, Clone)]
struct WrappedLinesCache {
    lines: Vec<tre_text::WrappedLine>,
    flat_glyphs: Vec<tre_text::ShapedGlyph>,
}

impl TextShapeCache {
    #[allow(
        clippy::float_cmp,
        reason = "exact equality is the correct semantics here, not a margin-of-error bug: \
                   px_size/wrap_width are caller-set values compared against their own \
                   previous value, and ANY change (even a tiny one) must invalidate the cache \
                   -- rounding two genuinely different sizes into 'close enough, reuse the old \
                   shaping' would silently render the wrong glyph metrics"
    )]
    fn is_valid_for(&self, text: &Text) -> bool {
        self.text == text.text
            && self.font == text.font
            && self.px_size == text.px_size
            && self.wrap_width == text.wrap_width
    }
}

/// Resolves `text`'s own shaped runs, reusing `cache` when nothing that
/// affects shaping has changed since the last call, and re-shaping (then
/// updating `cache` in place) otherwise. Returns `None` exactly when
/// `tre_text::shape_text` itself would (a real shaping failure),
/// matching [`flatten_text`]'s own established "report, don't block"
/// contract for unshapeable input.
fn resolve_shaped_runs<'a>(
    cache: &'a mut Option<TextShapeCache>,
    face: &rustybuzz::Face<'_>,
    text: &Text,
) -> Option<&'a [tre_text::ShapedRun]> {
    let is_valid = cache.as_ref().is_some_and(|c| c.is_valid_for(text));
    if !is_valid {
        let runs = tre_text::shape_text(face, &text.text).ok()?;
        *cache = Some(TextShapeCache {
            text: text.text.clone(),
            font: text.font,
            px_size: text.px_size,
            wrap_width: text.wrap_width,
            runs,
            wrapped: None,
        });
    }
    Some(
        &cache
            .as_ref()
            .expect("just set above if it wasn't already valid")
            .runs,
    )
}

/// Resolves `text`'s own wrapped lines (and their flat, concatenated
/// glyph list), reusing `cache.wrapped` when already populated and
/// computing (then storing) it otherwise. Mirrors [`resolve_shaped_runs`]'s
/// own caching discipline one layer up the pipeline -- see
/// `TextShapeCache::wrapped`'s own doc comment for why this exists
/// (`/review-project` Performance finding, 2026-09-13).
///
/// # Panics
/// Panics if `cache` is `None` -- callers must call
/// [`resolve_shaped_runs`] first to ensure it is populated; this
/// function only ever adds to an already-`Some` cache, never creates
/// one from scratch (it has no `rustybuzz::Face` to shape with).
fn resolve_wrapped_lines<'a>(
    cache: &'a mut Option<TextShapeCache>,
    text: &Text,
    wrap_width: f32,
    units_per_em: u16,
) -> &'a WrappedLinesCache {
    let needs_wrap = cache
        .as_ref()
        .expect("caller must ensure Some via resolve_shaped_runs first")
        .wrapped
        .is_none();
    if needs_wrap {
        let cache_ref = cache.as_ref().expect("just checked Some above");
        let lines = tre_text::wrap_lines(
            &text.text,
            &cache_ref.runs,
            text.px_size,
            units_per_em,
            Some(wrap_width),
        );
        let flat_glyphs: Vec<tre_text::ShapedGlyph> = cache_ref
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter().copied())
            .collect();
        // `cache_ref`'s last use is right above -- ends its borrow of
        // `*cache` here, freeing it for the mutable access below.
        cache.as_mut().expect("just checked Some above").wrapped =
            Some(WrappedLinesCache { lines, flat_glyphs });
    }
    cache
        .as_ref()
        .expect("checked Some above")
        .wrapped
        .as_ref()
        .expect("just populated above if it wasn't already")
}

#[allow(
    clippy::cast_precision_loss,
    reason = "unitsPerEm/ascent and every glyph's own advance stay far below f32's exact-integer \
               range for any real font/text -- the same precedent draw_text's own doc comment \
               already establishes"
)]
pub(crate) fn flatten_text(
    canvas: &mut crate::RenderingCanvas,
    text: &Text,
    context: &TextFlattenContext<'_>,
    shape_cache: &mut Option<TextShapeCache>,
) {
    let font = context.fonts.font_ref(text.font).unwrap_or_else(|| {
        panic!(
            "Text shape references FontId {:?} that this TextFlattenContext's FontRegistry never \
             issued",
            text.font
        )
    });
    let face = context.fonts.face(text.font).expect(
        "FontRegistry::load_bytes already validated these same bytes build a real rustybuzz::Face",
    );

    if resolve_shaped_runs(shape_cache, &face, text).is_none() {
        return;
    }

    let metrics = skrifa::MetadataProvider::metrics(
        &font,
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::default(),
    );
    let scale = text.px_size / f32::from(metrics.units_per_em);

    let Some(wrap_width) = text.wrap_width else {
        // The exact pre-Phase-15 single-line path -- untouched, so
        // every existing caller's rendering stays byte-identical.
        let cache = shape_cache
            .as_ref()
            .expect("resolve_shaped_runs above already ensured Some");
        let mut pen = [0.0, metrics.ascent * scale];
        for run in &cache.runs {
            canvas.draw_text(
                run,
                &font,
                text.font.0,
                pen,
                text.px_size,
                text.fill_color,
                context.atlas,
            );
            for glyph in &run.glyphs {
                pen[0] += glyph.x_advance as f32 * scale;
                pen[1] += glyph.y_advance as f32 * scale;
            }
        }
        return;
    };

    // Real multi-line rendering (Phase 15 Step 15.1): `\n` always
    // breaks a line, and (since `wrap_width` is finite here for any
    // real caller -- `f32::INFINITY` is the real "hard-wrap-only, no
    // width limit" escape hatch) words greedily wrap to fit. See
    // `tre_text::wrap_lines`'s own doc comment for the exact algorithm.
    // `resolve_wrapped_lines` caches this pass's own output the same
    // way `resolve_shaped_runs` already caches shaping itself, so an
    // animated-but-otherwise-unchanged wrapped `Text` shape pays this
    // cost once, not every frame (`/review-project` Performance
    // finding, 2026-09-13 -- previously ran unconditionally here, even
    // on a shaping-cache hit).
    let wrapped = resolve_wrapped_lines(shape_cache, text, wrap_width, metrics.units_per_em);

    // `metrics.descent` is a real, signed OpenType value -- negative,
    // extending below the baseline (confirmed empirically against a
    // real system font before trusting it here: this machine's default
    // cascade font reports `ascent=1069, descent=-293`, not a positive
    // magnitude) -- so the real total em-box height subtracts it
    // (equivalent to adding its real magnitude), not adds it.
    let line_height = (metrics.ascent - metrics.descent + metrics.leading) * scale;
    let mut pen_y = metrics.ascent * scale;
    // Each line's glyphs are a contiguous sub-range of the cached
    // `flat_glyphs`, so they go to the canvas as a borrowed slice --
    // not copied into a throwaway owned `ShapedRun` first, which was one
    // fresh heap `Vec` per visual line, every frame, even on a full
    // wrap-cache hit (`/review-project` Performance finding #247,
    // 2026-09-13: the residual allocation the wrap cache above left
    // behind, one step downstream of what it fixed).
    for line in &wrapped.lines {
        canvas.draw_glyphs(
            &wrapped.flat_glyphs[line.start_glyph..line.end_glyph],
            &font,
            text.font.0,
            [0.0, pen_y],
            text.px_size,
            text.fill_color,
            context.atlas,
        );
        pen_y += line_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same real system cascade font `crate::shapes`'s own
    /// `flatten_into_renders_a_text_shape_via_the_real_font_and_atlas_
    /// pipeline` test already discovers -- a real `rustybuzz::Face`, not
    /// a hand-built fake, since `resolve_shaped_runs`'s whole job is
    /// deciding whether to call the real `tre_text::shape_text` again.
    fn cascade_font_bytes() -> Vec<u8> {
        let cascade =
            tre_text::FontCascade::discover().expect("fontconfig cascade discovery failed");
        std::fs::read(&cascade.entries[0]).expect("failed to read the primary cascade font")
    }

    #[test]
    fn resolve_shaped_runs_reuses_the_cached_runs_when_nothing_shaping_relevant_changed() {
        let bytes = cascade_font_bytes();
        let face =
            rustybuzz::Face::from_slice(&bytes, 0).expect("real font bytes build a real Face");
        let text = Text::new("Hello", FontId(0), 24.0, 0xFFFF_FFFF);

        let mut cache = None;
        assert!(resolve_shaped_runs(&mut cache, &face, &text).is_some());
        let first_ptr = cache.as_ref().unwrap().runs.as_ptr();

        // A second call against the *identical* Text value -- exactly
        // what flatten_into's own generic "any active animation
        // re-flattens everything" dirty check produces every frame for
        // a Text shape animating a cosmetic property (opacity,
        // position, ...) that never touches shaping -- must reuse the
        // same cached Vec, not allocate a fresh one via a real re-shape.
        assert!(resolve_shaped_runs(&mut cache, &face, &text).is_some());
        let second_ptr = cache.as_ref().unwrap().runs.as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "an unchanged Text must reuse its cached shaped runs, not re-shape"
        );
    }

    #[test]
    fn resolve_shaped_runs_reshapes_when_the_text_content_changes() {
        let bytes = cascade_font_bytes();
        let face =
            rustybuzz::Face::from_slice(&bytes, 0).expect("real font bytes build a real Face");

        let mut cache = None;
        resolve_shaped_runs(
            &mut cache,
            &face,
            &Text::new("Hi", FontId(0), 24.0, 0xFFFF_FFFF),
        );
        let short_glyph_count: usize = cache
            .as_ref()
            .unwrap()
            .runs
            .iter()
            .map(|r| r.glyphs.len())
            .sum();

        resolve_shaped_runs(
            &mut cache,
            &face,
            &Text::new("Hello there, world", FontId(0), 24.0, 0xFFFF_FFFF),
        );
        let longer_glyph_count: usize = cache
            .as_ref()
            .unwrap()
            .runs
            .iter()
            .map(|r| r.glyphs.len())
            .sum();

        assert!(
            longer_glyph_count > short_glyph_count,
            "a genuinely different string must be re-shaped, not served from the old string's \
             stale cache (short: {short_glyph_count} glyphs, longer: {longer_glyph_count})"
        );
        assert_eq!(cache.as_ref().unwrap().text, "Hello there, world");
    }

    #[test]
    fn text_shape_cache_is_valid_for_checks_every_shaping_relevant_field_and_only_those() {
        let cache = TextShapeCache {
            text: "Hi".to_string(),
            font: FontId(0),
            px_size: 24.0,
            wrap_width: None,
            runs: Vec::new(),
            wrapped: None,
        };

        assert!(cache.is_valid_for(&Text::new("Hi", FontId(0), 24.0, 0xFFFF_FFFF)));
        assert!(
            !cache.is_valid_for(&Text::new("Bye", FontId(0), 24.0, 0xFFFF_FFFF)),
            "a different text string must invalidate the cache"
        );
        assert!(
            !cache.is_valid_for(&Text::new("Hi", FontId(1), 24.0, 0xFFFF_FFFF)),
            "a different font must invalidate the cache"
        );
        assert!(
            !cache.is_valid_for(&Text::new("Hi", FontId(0), 30.0, 0xFFFF_FFFF)),
            "a different px_size must invalidate the cache"
        );
        let mut wrapped = Text::new("Hi", FontId(0), 24.0, 0xFFFF_FFFF);
        wrapped.wrap_width = Some(100.0);
        assert!(
            !cache.is_valid_for(&wrapped),
            "a different wrap_width must invalidate the cache"
        );

        // The whole point of this fix: fill_color and every field under
        // `common` (opacity, position, scale, rotation, ...) must NOT
        // invalidate the cache -- these are exactly the cosmetic
        // properties a real animation would touch every frame.
        let mut cosmetic_change = Text::new("Hi", FontId(0), 24.0, 0x0000_00FF);
        cosmetic_change.common.opacity = 0.3;
        cosmetic_change.common.transform.position = [50.0, 80.0];
        cosmetic_change.common.transform.rotation = 1.2;
        assert!(
            cache.is_valid_for(&cosmetic_change),
            "fill_color/opacity/position/rotation must never invalidate the shaping cache"
        );
    }

    #[test]
    fn resolve_wrapped_lines_reuses_the_cached_lines_when_nothing_wrap_relevant_changed() {
        let bytes = cascade_font_bytes();
        let face =
            rustybuzz::Face::from_slice(&bytes, 0).expect("real font bytes build a real Face");
        let mut text = Text::new("Hello there, world", FontId(0), 24.0, 0xFFFF_FFFF);
        text.wrap_width = Some(40.0);

        let mut cache = None;
        resolve_shaped_runs(&mut cache, &face, &text);
        let metrics_units_per_em = 1000; // exact value is irrelevant to this test's own assertion
        let first = resolve_wrapped_lines(
            &mut cache,
            &text,
            text.wrap_width.unwrap(),
            metrics_units_per_em,
        );
        let first_ptr = first.lines.as_ptr();

        // A second call against the identical `Text` -- exactly what a
        // cosmetic-only animation on a wrapped `Text` shape produces
        // every frame -- must reuse the cached wrap output, not
        // recompute `wrap_lines` again.
        let second = resolve_wrapped_lines(
            &mut cache,
            &text,
            text.wrap_width.unwrap(),
            metrics_units_per_em,
        );
        let second_ptr = second.lines.as_ptr();
        assert_eq!(
            first_ptr, second_ptr,
            "an unchanged Text must reuse its cached wrapped lines, not re-wrap"
        );
    }

    #[test]
    fn resolve_wrapped_lines_rewraps_when_the_text_content_changes() {
        let bytes = cascade_font_bytes();
        let face =
            rustybuzz::Face::from_slice(&bytes, 0).expect("real font bytes build a real Face");
        let units_per_em = 1000;

        let mut cache = None;
        let mut short = Text::new("Hi", FontId(0), 24.0, 0xFFFF_FFFF);
        short.wrap_width = Some(40.0);
        resolve_shaped_runs(&mut cache, &face, &short);
        let short_line_count = resolve_wrapped_lines(&mut cache, &short, 40.0, units_per_em)
            .lines
            .len();

        let mut longer = Text::new(
            "Hello there, this is a much longer sentence that will wrap across several lines",
            FontId(0),
            24.0,
            0xFFFF_FFFF,
        );
        longer.wrap_width = Some(40.0);
        resolve_shaped_runs(&mut cache, &face, &longer);
        let longer_line_count = resolve_wrapped_lines(&mut cache, &longer, 40.0, units_per_em)
            .lines
            .len();

        assert!(
            longer_line_count > short_line_count,
            "a genuinely different, longer string must re-wrap into more lines, not reuse the \
             short string's stale wrap cache (short: {short_line_count} lines, longer: \
             {longer_line_count} lines)"
        );
    }
}
