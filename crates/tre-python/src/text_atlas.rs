//! A renderer-owned, real, live text atlas (Phase 12 Step 12.3): spawns
//! one `tre_atlas::AtlasOwner` background thread and one real GPU
//! texture at construction, then periodically refreshes that texture
//! from the atlas's own live pixel buffer as new glyphs resolve
//! (`tre_atlas::AtlasOwnerHandle::snapshot`/`generation`, Phase 12 Step
//! 12.2's own new capability) -- shared by `PyHeadlessRenderer` and
//! `PyWindowedRenderer`, since both need the identical real setup and
//! refresh logic.
//!
//! **A real, disclosed limitation**, not a silent gap: there is no
//! `RhiDevice` method to update an existing texture's pixels in place
//! (only `create_texture`, a fresh upload), so a refresh here creates a
//! brand-new texture -- and a brand-new bindless slot -- every time. The
//! bindless array is a fixed-size table (`tre-rhi-vulkan`'s own
//! `BINDLESS_TEXTURE_CAPACITY_TARGET`, 4096 slots) with no way to free
//! an old slot, so a real app whose text vocabulary keeps introducing
//! genuinely new glyphs forever, refreshed every
//! [`REFRESH_INTERVAL_FRAMES`] frames indefinitely, will eventually
//! exhaust it (`EngineError::BindlessArrayExhausted`).
//! `REFRESH_INTERVAL_FRAMES` bounds how *often* this can happen, not how
//! much it costs per refresh -- a real, disclosed mitigation, not a fix.
//! The real fix is a future `RhiDevice::update_texture` that writes into
//! an existing image, avoiding a new bindless slot per refresh entirely.
//! Most real UI text settles on a bounded glyph vocabulary quickly, so
//! this is a real, acceptable first-pass tradeoff, not a silent landmine
//! for the common case.

use pyo3::PyResult;
use tre_atlas::{AtlasOwner, AtlasOwnerHandle};
use tre_engine::{GlyphAtlasContext, RhiDevice, RhiTexture, TextureFormat};

use crate::error::engine_err;

/// Real default: comfortably holds a real UI's own working glyph set at
/// common sizes (scaled up from `atlas_concurrency_demo.rs`'s own
/// illustrative 256x256 toy size for a real, not just demonstrative,
/// renderer).
const ATLAS_SIZE: u32 = 1024;
const REQUEST_CAPACITY: usize = 512;
const SLOT_CAPACITY: usize = 4096;

/// See this module's own doc comment for why this isn't "every frame."
const REFRESH_INTERVAL_FRAMES: u64 = 15;

pub(crate) struct TextAtlas {
    // `Option` so `Drop` can `.take()` it to call `AtlasOwner::join`,
    // which needs ownership -- a `Drop` impl only ever gets `&mut self`.
    // Without this, dropping a renderer would leave its atlas's
    // background thread running detached until process exit instead of
    // being cleanly signaled to stop.
    owner: Option<AtlasOwner>,
    handle: AtlasOwnerHandle,
    texture: Box<dyn RhiTexture>,
    texture_index: u32,
    uploaded_generation: u64,
    frame_counter: u64,
}

impl TextAtlas {
    pub(crate) fn new(device: &dyn RhiDevice) -> PyResult<Self> {
        let owner = AtlasOwner::spawn(ATLAS_SIZE, ATLAS_SIZE, REQUEST_CAPACITY, SLOT_CAPACITY);
        let handle = owner.handle();
        #[allow(
            clippy::cast_possible_truncation,
            reason = "ATLAS_SIZE is a small fixed constant, far below usize::MAX on any real \
                       target"
        )]
        let blank = vec![0u8; (ATLAS_SIZE as usize) * (ATLAS_SIZE as usize) * 4];
        let texture = device
            .create_texture(ATLAS_SIZE, ATLAS_SIZE, TextureFormat::Rgba8Unorm, &blank)
            .map_err(engine_err)?;
        let texture_index = texture
            .bindless_index()
            .expect("a freshly created texture always has a real bindless index");
        Ok(Self {
            owner: Some(owner),
            handle,
            texture,
            texture_index,
            uploaded_generation: 0,
            frame_counter: 0,
        })
    }

    /// Refreshes the live GPU texture from the atlas's own current pixel
    /// buffer if due (see this module's own doc comment), then returns
    /// this frame's real `GlyphAtlasContext`.
    ///
    /// # Errors
    /// Raises `TreError` if a due refresh's real texture upload fails
    /// (e.g. `EngineError::BindlessArrayExhausted` -- see this module's
    /// own doc comment).
    pub(crate) fn context(&mut self, device: &dyn RhiDevice) -> PyResult<GlyphAtlasContext<'_>> {
        self.frame_counter += 1;
        if self.frame_counter % REFRESH_INTERVAL_FRAMES == 0 {
            let current_generation = self.handle.generation();
            if current_generation != self.uploaded_generation {
                let snapshot = self.handle.snapshot();
                let fresh = device
                    .create_texture(ATLAS_SIZE, ATLAS_SIZE, TextureFormat::Rgba8Unorm, &snapshot)
                    .map_err(engine_err)?;
                self.texture_index = fresh
                    .bindless_index()
                    .expect("a freshly created texture always has a real bindless index");
                self.texture = fresh;
                self.uploaded_generation = current_generation;
            }
        }
        Ok(GlyphAtlasContext {
            atlas: &self.handle,
            texture_handle: self.texture_index,
            dimensions: (ATLAS_SIZE, ATLAS_SIZE),
            current_frame: self.frame_counter,
        })
    }
}

impl Drop for TextAtlas {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            let _ = owner.join();
        }
    }
}
