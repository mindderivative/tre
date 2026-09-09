//! Phase 10 Step 10.2: GPU-side per-shape style records for data that
//! doesn't fit in `UiVertex`'s hard 32-byte layout (`params: [f32; 3]`,
//! see `documentation/ARCHITECTURE.md` Section 7's "Shape Style Buffer").
//! Growing `UiVertex` itself would bloat every pipeline in the system
//! (text, flat tessellated fills, blur) that doesn't need any of this
//! data -- instead, a style record is bump-allocated into
//! `RhiDevice::shape_style_buffer` (bound once, at device construction,
//! to the bindless descriptor set's binding 1) and referenced per-vertex
//! by a **word index** carried numerically in one `UiVertex.params`
//! float slot.
//!
//! # The word-index encoding
//! [`RhiDynamicRingBuffer::write`](crate::RhiDynamicRingBuffer::write)
//! returns a byte offset into the buffer. Every write this module ever
//! performs is 4-byte-aligned (every field here is `f32`/`u32`, and
//! `RhiDynamicRingBuffer::write` itself rounds every write up to a
//! 256-byte boundary -- see `RING_BUFFER_ALIGNMENT` in `tre-rhi-vulkan`),
//! so `byte_offset / 4` is always an exact integer: the record's **word
//! index**. [`style_index_param`] carries that `u32` through a
//! `UiVertex.params` float slot as a plain NUMERIC value (`as f32`), not
//! a bit-cast (`f32::from_bits`) -- see that function's own doc comment
//! for the real GPU bug (denormal flush-to-zero) a bit-cast encoding hit
//! in practice. The consuming fragment shader reads it back with
//! `uint(frag_params.x)` and reads fields at fixed relative word offsets
//! from a raw `readonly buffer { uint words[]; }` binding -- not GLSL
//! struct-array indexing, which would require the CPU and GPU to agree
//! on `std430`'s auto-computed per-field alignment/stride instead of the
//! plain, unambiguous byte-for-byte layout `#[repr(C)]` already gives
//! these structs.
//!
//! Each shape kind gets its own small record type sized to exactly what
//! that kind's shader needs -- not one large record shared (mostly
//! unused) by every kind. The style buffer's bump allocator doesn't care
//! that records have different sizes; nothing after construction ever
//! needs to iterate the buffer, only the exact word index a vertex was
//! given at write time.

/// [`GpuRectStyle`]'s size in `u32` words -- the fixed stride
/// `sdf_rect_styled.frag` reads from a rectangle's own word index.
pub const RECT_STYLE_WORDS: u32 = 7;

/// Rectangle's non-uniform-corner/border/smoothing style record
/// (Phase 10 Step 10.2). Read by `sdf_rect_styled.frag` as 7 sequential
/// `uint`s starting at a vertex's own style word index:
///
/// | word | field                        |
/// |------|------------------------------|
/// | 0    | `corner_radii[0]` (top-left) |
/// | 1    | `corner_radii[1]` (top-right)|
/// | 2    | `corner_radii[2]` (bottom-right) |
/// | 3    | `corner_radii[3]` (bottom-left) |
/// | 4    | `border_color` (packed RGBA8, `rgba8`'s byte order) |
/// | 5    | `border_thickness` |
/// | 6    | `corner_smoothing` (0 = circular arc, 1 = full superellipse blend) |
///
/// The GLSL side must stay in lockstep with this field order by hand --
/// nothing automatically checks it against the literal word offsets in
/// `sdf_rect_styled.frag`; [`gpu_style_tests::rect_style_round_trips_through_raw_words`]
/// guards the Rust-side encoding only.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuRectStyle {
    pub corner_radii: [f32; 4],
    pub border_color: u32,
    pub border_thickness: f32,
    pub corner_smoothing: f32,
}

const _: () = assert!(std::mem::size_of::<GpuRectStyle>() == (RECT_STYLE_WORDS as usize) * 4);

/// [`GpuEllipseStyle`]'s size in `u32` words -- the fixed stride
/// `sdf_ellipse.frag` reads from an ellipse's own word index.
pub const ELLIPSE_STYLE_WORDS: u32 = 4;

/// Circle/Ellipse's border/arc style record (Phase 10 Step 10.2). Read by
/// `sdf_ellipse.frag` as 4 sequential `uint`s starting at a vertex's own
/// style word index:
///
/// | word | field                |
/// |------|----------------------|
/// | 0    | `border_color` (packed RGBA8) |
/// | 1    | `border_thickness`   |
/// | 2    | `arc_start_angle` (radians) |
/// | 3    | `arc_sweep_angle` (radians; `TAU` = a full, unswept ellipse) |
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEllipseStyle {
    pub border_color: u32,
    pub border_thickness: f32,
    pub arc_start_angle: f32,
    pub arc_sweep_angle: f32,
}

const _: () = assert!(std::mem::size_of::<GpuEllipseStyle>() == (ELLIPSE_STYLE_WORDS as usize) * 4);

/// Bitcasts a style record's byte offset (as returned by
/// [`RhiDynamicRingBuffer::write`](crate::RhiDynamicRingBuffer::write))
/// into the `f32` that travels through a `UiVertex.params` slot -- see
/// this module's own doc comment for the full encoding.
///
/// A NUMERIC cast (`as f32`), not a bit-cast (`f32::from_bits`): a real
/// GPU bug, found and fixed while building `sdf_ellipse.frag`'s own
/// demo (`shape_full_rendering_demo.rs`) -- a small word index like `64`
/// bit-cast to `f32` is a subnormal number (its bit pattern's exponent
/// field is all zero), and this project's real hardware silently
/// flushed it to exactly `0.0` somewhere between the vertex and fragment
/// stage (interpolation and/or ALU flush-to-zero, both common, real GPU
/// behavior for subnormals), so `floatBitsToUint` in the shader read
/// back `style_index == 0` instead of the real value -- confirmed by
/// observing the fragment shader silently reading the WRONG style
/// record (REVIEW.md's own account of this finding has the full
/// diagnosis). A numeric cast has no such failure mode: every real word
/// index this codebase produces is a small integer, exactly
/// representable as a NORMAL `f32` (`f32` represents every integer up to
/// `2^24` exactly), so `value as f32` on the way in and `uint(value)` on
/// the way out of the shader round-trip exactly, with no denormal ever
/// in play.
///
/// # Panics
/// Panics (debug builds only) if `byte_offset` is not 4-byte-aligned --
/// every real write from this module is always a whole number of `u32`
/// words, so a misaligned offset here means a caller wrote something
/// other than a whole `GpuRectStyle`/`GpuEllipseStyle` (or the RHI's own
/// alignment guarantee regressed), not a normal runtime condition.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "word indices stay far below 2^24 for any real frame (this crate's shape-style \
               buffer is a few MiB at most), well within f32's exact-integer range"
)]
pub fn style_index_param(byte_offset: u32) -> f32 {
    debug_assert_eq!(
        byte_offset % 4,
        0,
        "style buffer byte offset {byte_offset} is not 4-byte aligned"
    );
    (byte_offset / 4) as f32
}

#[cfg(test)]
mod gpu_style_tests {
    use super::*;

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s round-tripped through raw bits, same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn rect_style_round_trips_through_raw_words() {
        let style = GpuRectStyle {
            corner_radii: [1.0, 2.0, 3.0, 4.0],
            border_color: 0xAABB_CCDD,
            border_thickness: 5.0,
            corner_smoothing: 0.5,
        };
        let bytes = bytemuck::bytes_of(&style);
        assert_eq!(bytes.len(), 28);
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes(c.try_into().expect("4-byte chunk")))
            .collect();
        assert_eq!(f32::from_bits(words[0]), 1.0);
        assert_eq!(f32::from_bits(words[1]), 2.0);
        assert_eq!(f32::from_bits(words[2]), 3.0);
        assert_eq!(f32::from_bits(words[3]), 4.0);
        assert_eq!(words[4], 0xAABB_CCDD);
        assert_eq!(f32::from_bits(words[5]), 5.0);
        assert_eq!(f32::from_bits(words[6]), 0.5);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s round-tripped through raw bits, same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn ellipse_style_round_trips_through_raw_words() {
        let style = GpuEllipseStyle {
            border_color: 0x1122_3344,
            border_thickness: 2.5,
            arc_start_angle: 0.25,
            arc_sweep_angle: std::f32::consts::TAU,
        };
        let bytes = bytemuck::bytes_of(&style);
        assert_eq!(bytes.len(), 16);
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes(c.try_into().expect("4-byte chunk")))
            .collect();
        assert_eq!(words[0], 0x1122_3344);
        assert_eq!(f32::from_bits(words[1]), 2.5);
        assert_eq!(f32::from_bits(words[2]), 0.25);
        assert_eq!(f32::from_bits(words[3]), std::f32::consts::TAU);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact integer-valued arithmetic on a small u32 cast to f32, not an \
                   epsilon-worthy computed value -- the whole point of the numeric (not \
                   bit-cast) encoding this function documents is that it round-trips exactly"
    )]
    fn style_index_param_numerically_encodes_the_word_index_not_the_byte_offset() {
        // byte offset 256 -> word index 64 -- a NUMERIC 64.0, not a
        // bit-cast (which would be a subnormal float that real GPU
        // hardware has been observed to flush to zero -- this function's
        // own doc comment has the full account).
        let param = style_index_param(256);
        assert_eq!(param, 64.0);
    }

    #[test]
    #[should_panic(expected = "not 4-byte aligned")]
    fn style_index_param_rejects_a_misaligned_offset_in_debug_builds() {
        let _ = style_index_param(3);
    }
}
