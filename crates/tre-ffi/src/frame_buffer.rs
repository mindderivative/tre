//! `TreFrameBuffer` -- the pointer+length shadow of the `Vec<u8>`
//! `tre_headless_renderer_render` hands back (TECHNICAL.md Section
//! 9.4.1: `Vec<T>` never crosses the C-ABI boundary directly). Raw
//! BGRA8, matching `HeadlessSwapchain::read_pixels_bgra8`'s own real
//! internal format exactly -- this crate does no color-order conversion
//! of its own, the same "expose the engine's real behavior transparently"
//! choice `docs/getting-started.md` already documents for `tre-python`'s
//! identical readback path.

use std::os::raw::c_uchar;

/// Freed by [`tre_frame_buffer_free`], never by the caller's own
/// allocator (TECHNICAL.md Section 9.4.1's "memory ownership" rule:
/// Rust's allocator and the host language's allocator are never assumed
/// compatible).
#[repr(C)]
pub struct TreFrameBuffer {
    pub data: *const c_uchar,
    pub len: usize,
    /// The real backing allocation's own capacity, distinct from `len`
    /// whenever the two differ -- `tre_frame_buffer_free` needs the
    /// *exact* original capacity to reconstruct the `Vec<u8>` this leaked
    /// (`Vec::from_raw_parts`'s own safety contract), and nothing about
    /// `Vec<u8>` guarantees `capacity() == len()` even after
    /// `shrink_to_fit` (the allocator is free to leave excess). A C
    /// caller has no use for this field beyond passing the whole struct
    /// back to `tre_frame_buffer_free` unchanged.
    pub capacity: usize,
}

impl TreFrameBuffer {
    /// Leaks `bytes`' own heap allocation into a raw pointer this struct
    /// carries across the boundary -- reclaimed by
    /// [`tre_frame_buffer_free`]'s matching `Vec::from_raw_parts`, which
    /// needs `len` AND `capacity` both preserved exactly as they were at
    /// the moment of leaking.
    pub(crate) fn from_vec(bytes: Vec<u8>) -> Self {
        let mut bytes = std::mem::ManuallyDrop::new(bytes);
        Self {
            data: bytes.as_mut_ptr(),
            len: bytes.len(),
            capacity: bytes.capacity(),
        }
    }

    /// The empty, "nothing to free" sentinel `tre_headless_renderer_render`
    /// leaves `*out` at when it returns a non-`Success` error code, so a
    /// caller that unconditionally calls `tre_frame_buffer_free` on every
    /// `TreFrameBuffer` it ever sees (rather than gating on the result
    /// code first) doesn't double-free or free a dangling pointer.
    pub(crate) const fn empty() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Frees a [`TreFrameBuffer`] returned by
/// [`crate::tre_headless_renderer_render`]. Safe to call on
/// [`TreFrameBuffer::empty`]'s own all-zero value (a no-op).
///
/// # Safety
/// `buffer` must be a value this crate itself produced and not yet freed
/// -- calling this on a `TreFrameBuffer` built any other way, or calling
/// it twice on the same value, is undefined behavior (the same contract
/// every `tre_*_free` function in this crate carries).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tre_frame_buffer_free(buffer: TreFrameBuffer) {
    if buffer.data.is_null() {
        return;
    }
    // SAFETY: `buffer.data`/`buffer.len`/`buffer.capacity` were produced
    // together by `TreFrameBuffer::from_vec`'s own
    // `ManuallyDrop<Vec<u8>>`, and the caller's contract (this function's
    // own `# Safety` section) guarantees this runs at most once per
    // value.
    drop(unsafe { Vec::from_raw_parts(buffer.data.cast_mut(), buffer.len, buffer.capacity) });
}
