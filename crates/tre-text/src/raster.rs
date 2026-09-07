//! [`tre_atlas::RasterSource`] glue for glyph outlines -- promoted out of
//! two ad hoc, identical demo-local copies (`atlas_concurrency_demo.rs`,
//! `atlas_eviction_demo.rs`) into real, reusable code so `Canvas::
//! draw_text` (IMPLEMENTATION.md Step 5.1.2) has a non-demo type to hand
//! `AtlasOwnerHandle::request_insert`. Deliberately lives in `tre-text`,
//! not `tre-atlas`: `tre-atlas` stays content-agnostic (Step 4.2.1's own
//! precedent, `tre-atlas/src/raster.rs`'s doc comment), so the
//! content-aware glue -- turning glyph contours into RGBA8 pixels --
//! belongs on this side of the `RasterSource` trait object boundary.

use tre_atlas::RasterSource;

use crate::msdf::generate_msdf;
use crate::outline::Contour;

/// Rasterizes one glyph's MSDF on demand for [`tre_atlas::AtlasOwnerHandle::
/// request_insert`]. Callers must only construct this for a glyph already
/// confirmed to have real ink (a non-empty `contours`, e.g. via
/// [`crate::glyph_outline`]) -- `rasterize()` panics otherwise, since
/// [`generate_msdf`] legitimately returns `None` for glyphs with no ink
/// (whitespace) and this type has no non-panicking way to report that
/// back through `RasterSource::rasterize`'s non-optional `Vec<u8>`
/// signature.
pub struct GlyphRasterSource {
    pub contours: Vec<Contour>,
    /// The MSDF's own square canvas side length, in pixels -- forwarded
    /// to `generate_msdf` and reported back via `size()` unchanged.
    pub size: u32,
    /// Pixels of margin around the glyph on every side, `generate_msdf`'s
    /// own `range_px` parameter.
    pub range_px: f64,
}

impl RasterSource for GlyphRasterSource {
    fn size(&self) -> (u32, u32) {
        (self.size, self.size)
    }

    /// # Panics
    ///
    /// Panics if `contours` has no real ink -- see the struct-level doc
    /// comment.
    fn rasterize(&self) -> Vec<u8> {
        let bitmap = generate_msdf(&self.contours, self.size, self.range_px)
            .expect("GlyphRasterSource must only be constructed for a glyph outline with real ink");
        pad_rgb_to_rgba(&bitmap.pixels)
    }
}

/// [`generate_msdf`] returns raw RGB8 (Step 4.2.2); [`RasterSource::
/// rasterize`]'s contract (`tre-atlas/src/raster.rs`) requires RGBA8 to
/// match `TextureFormat::Rgba8Unorm`'s upload layout exactly -- padding
/// in a fully-opaque alpha byte per pixel is the only conversion needed.
fn pad_rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    rgb.chunks_exact(3)
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect()
}
