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
//!
//! # Gradient fill (Phase 10 Step 10.2.1)
//! [`GpuRectStyle`]/[`GpuEllipseStyle`] both gained two trailing words --
//! `fill_kind` (`0` = solid, sourced from the vertex's own interpolated
//! `frag_color`; `1` = gradient, sourced from a [`GpuGradientStyle`]
//! record) and `gradient_word_index` (a SECOND style-buffer word index,
//! valid only when `fill_kind == 1`, pointing at that record) -- additive
//! to each struct's own existing fields, so every pre-existing solid-fill
//! caller is unaffected (`fill_kind` defaults to `0`, `bytemuck::
//! Zeroable`'s own zero value). A gradient is its own separate,
//! independently word-indexed record in the SAME style buffer, not
//! inlined into `GpuRectStyle`/`GpuEllipseStyle` themselves, because its
//! real size (a kind tag, two points, a stop count, and up to
//! [`GRADIENT_MAX_STOPS`] stops) is far larger than either of those
//! structs' own remaining budget, and because Polygon/Path's own
//! `PipelineKind::GradientFill` needs to reference the exact same record
//! type through a completely different path (a push-constant word index,
//! not a per-vertex style word) -- see that pipeline's own shader for the
//! account of why Polygon/Path can't use a per-vertex style record at
//! all today.

/// [`GpuRectStyle`]'s size in `u32` words -- the fixed stride
/// `sdf_rect_styled.frag` reads from a rectangle's own word index.
pub const RECT_STYLE_WORDS: u32 = 9;

/// Rectangle's non-uniform-corner/border/smoothing/fill style record
/// (Phase 10 Step 10.2, extended Step 10.2.1 for gradient fill). Read by
/// `sdf_rect_styled.frag` as 9 sequential `uint`s starting at a vertex's
/// own style word index:
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
/// | 7    | `fill_kind` (0 = solid, via `frag_color`; 1 = gradient) |
/// | 8    | `gradient_word_index` (a [`GpuGradientStyle`] word index, valid only when `fill_kind == 1`) |
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
    pub fill_kind: u32,
    pub gradient_word_index: u32,
}

const _: () = assert!(std::mem::size_of::<GpuRectStyle>() == (RECT_STYLE_WORDS as usize) * 4);

/// [`GpuEllipseStyle`]'s size in `u32` words -- the fixed stride
/// `sdf_ellipse.frag` reads from an ellipse's own word index.
pub const ELLIPSE_STYLE_WORDS: u32 = 6;

/// Circle/Ellipse's border/arc/fill style record (Phase 10 Step 10.2,
/// extended Step 10.2.1 for gradient fill). Read by `sdf_ellipse.frag` as
/// 6 sequential `uint`s starting at a vertex's own style word index:
///
/// | word | field                |
/// |------|----------------------|
/// | 0    | `border_color` (packed RGBA8) |
/// | 1    | `border_thickness`   |
/// | 2    | `arc_start_angle` (radians) |
/// | 3    | `arc_sweep_angle` (radians; `TAU` = a full, unswept ellipse) |
/// | 4    | `fill_kind` (0 = solid, via `frag_color`; 1 = gradient) |
/// | 5    | `gradient_word_index` (a [`GpuGradientStyle`] word index, valid only when `fill_kind == 1`) |
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEllipseStyle {
    pub border_color: u32,
    pub border_thickness: f32,
    pub arc_start_angle: f32,
    pub arc_sweep_angle: f32,
    pub fill_kind: u32,
    pub gradient_word_index: u32,
}

const _: () = assert!(std::mem::size_of::<GpuEllipseStyle>() == (ELLIPSE_STYLE_WORDS as usize) * 4);

/// The maximum number of color stops a single gradient can carry
/// (Phase 10 Step 10.2.1) -- a small, fixed, disclosed limit (matching
/// this workspace's own established "small, fixed, disclosed input cap"
/// precedent, e.g. `tre-svg`'s own parse limits), not an arbitrary GPU
/// constraint. `ShapeRegistry::create_gradient` rejects more than this
/// many stops with a real `Err`, not silent truncation.
pub const GRADIENT_MAX_STOPS: usize = 8;

/// [`GpuGradientStyle`]'s size in `u32` words -- the fixed stride both
/// `sdf_rect_styled.frag`/`sdf_ellipse.frag` (via `gradient_word_index`)
/// and `gradient_fill.frag` (via a push-constant word index, Polygon/
/// Path's own path since neither has a per-vertex style record) read a
/// gradient's own word index at.
#[allow(
    clippy::cast_possible_truncation,
    reason = "GRADIENT_MAX_STOPS is a small compile-time literal (8), nowhere near u32::MAX"
)]
pub const GRADIENT_STYLE_WORDS: u32 = 6 + (GRADIENT_MAX_STOPS as u32) * 2;

/// A real, evaluatable linear-or-radial gradient record (Phase 10 Step
/// 10.2.1), independently word-indexed in the same style buffer
/// [`GpuRectStyle`]/[`GpuEllipseStyle`] records live in. Read as
/// `GRADIENT_STYLE_WORDS` sequential `uint`s starting at the gradient's
/// own word index:
///
/// | word     | field |
/// |----------|-------|
/// | 0        | `kind` (0 = linear, 1 = radial) |
/// | 1        | `point0.x` (linear: start.x; radial: center.x) |
/// | 2        | `point0.y` (linear: start.y; radial: center.y) |
/// | 3        | `point1_or_radius.x` (linear: end.x; radial: radius) |
/// | 4        | `point1_or_radius.y` (linear: end.y; radial: unused, always `0`) |
/// | 5        | `stop_count` (`1..=GRADIENT_MAX_STOPS`) |
/// | 6..14    | `stop_positions[GRADIENT_MAX_STOPS]` (only the first `stop_count` are real) |
/// | 14..22   | `stop_colors[GRADIENT_MAX_STOPS]` (packed RGBA8, `rgba8`'s byte order; only the first `stop_count` are real) |
///
/// All positions/points are in the SAME local, untransformed space each
/// consuming shader already evaluates in (`sdf_rect_styled.frag`/
/// `sdf_ellipse.frag`'s own `frag_uv`; `gradient_fill.frag`'s own local
/// `frag_uv`, repurposed to carry pre-transform vertex position for
/// Polygon/Path, since neither shape has a natural untransformed offset
/// otherwise) -- never world/screen space, so a shape's own gradient
/// moves and rotates rigidly with it, matching every other per-shape
/// style field's own convention.
///
/// Stop colors are stored packed sRGB (the same `rgba8` convention as
/// every other color in this style buffer); interpolation between two
/// stops happens in LINEAR space in the shader (`srgb_to_linear` per
/// stop endpoint, then `mix`), matching this codebase's own established
/// blending discipline (`documentation/ARCHITECTURE.md` Section 6.1) --
/// interpolating in sRGB space directly would produce the well-known
/// "muddy midpoint" artifact.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuGradientStyle {
    pub kind: u32,
    pub point0: [f32; 2],
    pub point1_or_radius: [f32; 2],
    pub stop_count: u32,
    pub stop_positions: [f32; GRADIENT_MAX_STOPS],
    pub stop_colors: [u32; GRADIENT_MAX_STOPS],
}

const _: () =
    assert!(std::mem::size_of::<GpuGradientStyle>() == (GRADIENT_STYLE_WORDS as usize) * 4);

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
            fill_kind: 1,
            gradient_word_index: 128,
        };
        let bytes = bytemuck::bytes_of(&style);
        assert_eq!(bytes.len(), 36);
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
        assert_eq!(words[7], 1);
        assert_eq!(words[8], 128);
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
            fill_kind: 1,
            gradient_word_index: 64,
        };
        let bytes = bytemuck::bytes_of(&style);
        assert_eq!(bytes.len(), 24);
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes(c.try_into().expect("4-byte chunk")))
            .collect();
        assert_eq!(words[0], 0x1122_3344);
        assert_eq!(f32::from_bits(words[1]), 2.5);
        assert_eq!(f32::from_bits(words[2]), 0.25);
        assert_eq!(f32::from_bits(words[3]), std::f32::consts::TAU);
        assert_eq!(words[4], 1);
        assert_eq!(words[5], 64);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "exact arithmetic on literal f32s round-tripped through raw bits, same \
                   reasoning as this crate's other exact-arithmetic tests"
    )]
    fn gradient_style_round_trips_through_raw_words() {
        let mut stop_positions = [0.0f32; GRADIENT_MAX_STOPS];
        let mut stop_colors = [0u32; GRADIENT_MAX_STOPS];
        stop_positions[0] = 0.0;
        stop_positions[1] = 1.0;
        stop_colors[0] = 0xFF00_00FF;
        stop_colors[1] = 0x0000_FFFF;
        let style = GpuGradientStyle {
            kind: 0,
            point0: [10.0, 20.0],
            point1_or_radius: [30.0, 40.0],
            stop_count: 2,
            stop_positions,
            stop_colors,
        };
        let bytes = bytemuck::bytes_of(&style);
        assert_eq!(bytes.len(), (GRADIENT_STYLE_WORDS as usize) * 4);
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes(c.try_into().expect("4-byte chunk")))
            .collect();
        assert_eq!(words[0], 0);
        assert_eq!(f32::from_bits(words[1]), 10.0);
        assert_eq!(f32::from_bits(words[2]), 20.0);
        assert_eq!(f32::from_bits(words[3]), 30.0);
        assert_eq!(f32::from_bits(words[4]), 40.0);
        assert_eq!(words[5], 2);
        assert_eq!(f32::from_bits(words[6]), 0.0);
        assert_eq!(f32::from_bits(words[7]), 1.0);
        assert_eq!(words[6 + GRADIENT_MAX_STOPS], 0xFF00_00FF);
        assert_eq!(words[6 + GRADIENT_MAX_STOPS + 1], 0x0000_FFFF);
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
