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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontError {
    InvalidFont,
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFont => {
                write!(
                    f,
                    "font bytes are not a valid font (rejected by skrifa or rustybuzz)"
                )
            }
        }
    }
}

impl std::error::Error for FontError {}

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

    let Ok(runs) = tre_text::shape_text(&face, &text.text) else {
        return;
    };

    let metrics = skrifa::MetadataProvider::metrics(
        &font,
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::default(),
    );
    let scale = text.px_size / f32::from(metrics.units_per_em);

    let mut pen = [0.0, metrics.ascent * scale];
    for run in &runs {
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
}
