//! [`RasterSource`] and [`AtlasInsertRequest`] -- ARCHITECTURE.md Section
//! 2.3's `raster_source: RasterSourceHandle` field, made concrete as a
//! trait object rather than a direct dependency on any specific content
//! crate. This is what keeps `tre-atlas` content-agnostic (Step 4.2.1's
//! own precedent: the atlas is shared by MSDF glyphs *and* plain-color
//! icons): the atlas owner can trigger rasterization without ever
//! knowing what's actually being rasterized. `tre-text` (or an icon
//! rasterizer, whenever one exists) supplies the real implementation.

use crate::AtlasKey;

/// A deferred rasterization operation -- `size()` tells the atlas owner
/// how large a rectangle to request from the packer *before* any pixel
/// work happens; `rasterize()` does the actual (potentially expensive)
/// work only once the owner has confirmed there's room, matching
/// ARCHITECTURE.md's "raster_source: RasterSourceHandle" naming (a
/// handle to an operation, not the pixels themselves, up front).
///
/// `rasterize()` must return RGBA8 pixels, `width * height * 4` bytes,
/// row-major -- the same layout `TextureFormat::Rgba8Unorm` (Step 4.2.3)
/// expects, so the atlas owner can copy them directly into the shared
/// atlas buffer with no further conversion.
pub trait RasterSource: Send {
    fn size(&self) -> (u32, u32);
    fn rasterize(&self) -> Vec<u8>;
}

/// One pending atlas insertion -- ARCHITECTURE.md Section 2.3's own
/// `AtlasInsertRequest` struct, carried through the real
/// [`tre_memory::MpscRingBuffer`] from any producer thread to the single
/// atlas owner.
pub struct AtlasInsertRequest {
    pub key: AtlasKey,
    pub raster_source: Box<dyn RasterSource>,
}
