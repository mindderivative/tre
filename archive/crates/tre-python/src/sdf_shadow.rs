//! Phase 16 Step 16.1: a real, continuously tunable SDF-based soft
//! shadow, built entirely on the existing custom shader API (Phase 13
//! Step 13.8) rather than by reworking `LayerDesc.blur`'s own fixed
//! 4-hop Dual-Kawase chain (`crates/tre-rhi-vulkan/src/lib.rs`'s
//! `BlurResources`) into something tunable -- a substantially bigger,
//! not-yet-needed RHI change. A caller compiles
//! [`sdf_shadow_shader_source`] once via `renderer.create_custom_shader`,
//! then renders a `tre.CustomShaded` quad through it for each shadow
//! instance, with [`shadow_sdf_params`] converting real pixel corner-
//! radius/blur values into the normalized params the shader expects.
//!
//! # Why normalized, not pixel, params
//! A general rounded-box shadow needs four independent numbers (half-
//! width, half-height, corner radius, blur softness/sigma) but
//! `UiVertex.params` only has three float slots (`crates/tre-engine/
//! src/gpu_style.rs`'s own doc comment: growing it "would bloat every
//! pipeline in the system"). The fix: the shader works in each axis'
//! own local space, where the box's half-extent is implicitly `1.0` on
//! both axes (`p = frag_uv * 2.0 - 1.0`, reusing `CustomShaded`'s
//! existing, unchanged `0..1` UV convention). Corner radius and blur
//! sigma are then fractions of the box's own half-extent *per axis*:
//! `params = [radius_frac, sigma_x_frac, sigma_y_frac]` -- exactly 3
//! floats.
//!
//! **Real, disclosed v1 scope limit**: for a non-square box, a
//! "radius_frac" corner is not a literal circular arc in true pixel
//! units (the two axes are independently normalized) -- visually
//! correct for modest aspect ratios (cards, buttons, tooltips), a real
//! approximation for extreme ones. The same category of disclosed
//! trade-off as `sdf_rect_styled.frag`'s own corner-smoothing/squircle
//! deviation, already measured and documented there.
//!
//! # A real bug found only once this actually rendered
//! A fragment shader is never invoked outside the triangles it's
//! rasterized on -- so a soft edge can never visually extend past a
//! `CustomShaded` quad drawn at exactly the shadow's own logical
//! `(x, y, width, height)`; the very first real render showed a hard
//! cutoff at the quad's own edge regardless of `blur_px`. The fix:
//! [`shadow_sdf_params`] returns the ENLARGED quad bounds a caller must
//! actually draw (the logical box expanded by `blur_px` on every side,
//! the same real margin concept `tre_engine::shadow_layer_bounds`
//! already uses for the v1 blur path), and the shader itself rescales
//! `frag_uv` by `(1 + sigma_frac)` per axis -- exactly the ratio between
//! the enlarged quad's own half-extent and the logical box's real
//! half-extent -- so the logical box's own edge (`d == 0`) still sits
//! at the right place inside the larger quad, with real room outside it
//! for the falloff to occupy.
//!
//! # The falloff: SDF soft edge, not a literal Gaussian blur
//! [`sdf_shadow_shader_source`] reuses the *exact* already-proven,
//! IQ-derived rounded-box signed-distance formula
//! `sdf_rect_styled.frag`'s own `sd_rounded_box` uses (adapted to a
//! single uniform radius -- per-corner radii would need a 4th float
//! this budget doesn't have), then applies a continuous `smoothstep`
//! falloff around the zero-distance edge. This is a real, honestly-
//! disclosed **SDF soft-edge** technique: continuously tunable (a
//! plain float, zero recompilation to retune) and rounds corners for
//! free via the reused SDF -- but its falloff curve is not a literal
//! Gaussian convolution of a box (that's the separate erf-based
//! technique real 2D box-shadow shaders use). What's real and
//! verifiable here is the *continuous* tunability itself, proven in
//! `demo/phase16_step16_1/demo.py` by rendering the same shadow at
//! several `blur_px` values and confirming the measured ink spread
//! grows monotonically -- unlike the old fixed-hop-count blur chain.

use pyo3::prelude::*;

/// See this module's own doc comment for the full design. Declares the
/// exact interface `VulkanDevice::create_custom_pipeline` requires
/// (`frag_color`/`frag_uv`/`frag_params` inputs, the 12-byte
/// `PushConstants` block, `out_color`) plus the real, disclosed
/// simplification (matching `bindless_textured.frag`'s own established
/// fallback branch) of linearizing `frag_color.rgb` without first
/// un-premultiplying it -- `draw_custom_shaded_quad` already
/// premultiplies vertex color by the shape's own opacity on the CPU
/// side before this shader ever sees it.
pub const SDF_SHADOW_SHADER_SOURCE: &str = r#"
#version 450
layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 2) in vec3 frag_params;
layout(location = 0) out vec4 out_color;
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

// IQ's exact rounded-box signed distance function, reused verbatim from
// this engine's own sdf_rect_styled.frag/sdf_rounded_rect.frag -- here
// evaluated in NORMALIZED per-axis local space (p in [-1,1], half_extent
// always (1,1)) rather than true pixel space, with a single uniform
// radius (see this module's own doc comment for why).
float sd_rounded_box(vec2 p, float radius) {
    vec2 q = abs(p) - vec2(1.0) + radius;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

void main() {
    float radius_frac = frag_params.x;
    float sigma_x_frac = frag_params.y;
    float sigma_y_frac = frag_params.z;

    // `frag_uv` spans the ENLARGED quad `shadow_sdf_params` returns
    // (the logical box expanded by blur_px on every side), not the
    // logical box itself -- rescale by (1 + sigma_frac) per axis
    // (exactly enlarged_half_extent / logical_half_extent) so the
    // logical box's own edge still lands at d == 0.
    vec2 p = (frag_uv * 2.0 - 1.0) * vec2(1.0 + sigma_x_frac, 1.0 + sigma_y_frac);
    float d = sd_rounded_box(p, radius_frac);
    float sigma = max(sigma_x_frac, sigma_y_frac);
    // A real SDF soft edge, not a literal Gaussian blur -- see this
    // module's own doc comment. `sigma <= 0` degrades to a hard edge
    // (a real, useful case: radius_frac alone still renders a plain
    // rounded rectangle with no softness at all).
    float alpha = sigma > 0.0 ? 1.0 - smoothstep(-sigma, sigma, d) : (d <= 0.0 ? 1.0 : 0.0);

    vec3 rgb = srgb_to_linear(frag_color.rgb);
    out_color = vec4(rgb * alpha, frag_color.a * alpha);
}
"#;

/// `tre.sdf_shadow_shader_source() -> str` -- the real GLSL source in
/// [`SDF_SHADOW_SHADER_SOURCE`], ready to pass straight to
/// `renderer.create_custom_shader(...)`.
#[pyfunction]
pub fn sdf_shadow_shader_source() -> &'static str {
    SDF_SHADOW_SHADER_SOURCE
}

/// Converts a shadow's own real, logical `(x, y, width, height)` plus
/// real pixel `radius_px`/`blur_px` into everything a caller needs to
/// actually draw it: the ENLARGED `CustomShaded` quad bounds
/// `(quad_x, quad_y, quad_width, quad_height)` -- the logical box
/// expanded by `blur_px` on every side, giving the shader's own soft
/// edge real rasterized room to fall off into (see this module's own
/// doc comment's "A real bug found only once this actually rendered")
/// -- and the normalized `(radius_frac, sigma_x_frac, sigma_y_frac)`
/// triple to set as that quad's own `param_x`/`param_y`/`param_z`.
///
/// `radius_px` is clamped so it never exceeds the box's own smaller
/// half-dimension (a radius larger than that has no further real
/// geometric meaning for a rounded box). `width_px`/`height_px` of
/// `0.0` degrade every fraction to `0.0` rather than dividing by zero
/// -- a real, disclosed no-op for a degenerate zero-size box, not a
/// panic; the returned quad bounds are still real (just the `blur_px`
/// margin around a zero-size point).
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "one field per real shadow-geometry input, mirroring shadow_layer_bounds' own \
               identical (x, y, width, height, ...) shape"
)]
pub fn shadow_sdf_params(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius_px: f32,
    blur_px: f32,
) -> (f32, f32, f32, f32, f32, f32, f32) {
    let half_w = width / 2.0;
    let half_h = height / 2.0;
    let min_half = half_w.min(half_h);
    let radius_frac = if min_half > 0.0 {
        (radius_px / min_half).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sigma_x_frac = if half_w > 0.0 { blur_px / half_w } else { 0.0 };
    let sigma_y_frac = if half_h > 0.0 { blur_px / half_h } else { 0.0 };
    let quad_x = x - blur_px;
    let quad_y = y - blur_px;
    let quad_width = width + 2.0 * blur_px;
    let quad_height = height + 2.0 * blur_px;
    (
        quad_x,
        quad_y,
        quad_width,
        quad_height,
        radius_frac,
        sigma_x_frac,
        sigma_y_frac,
    )
}

/// `tre.shadow_sdf_params(...)`, bound directly.
#[pyfunction]
#[pyo3(name = "shadow_sdf_params")]
#[allow(
    clippy::too_many_arguments,
    reason = "mirrors shadow_sdf_params' own identical shape, see that function's own reason"
)]
fn py_shadow_sdf_params(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius_px: f32,
    blur_px: f32,
) -> (f32, f32, f32, f32, f32, f32, f32) {
    shadow_sdf_params(x, y, width, height, radius_px, blur_px)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(sdf_shadow_shader_source, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(py_shadow_sdf_params, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_box_has_equal_sigma_fractions_on_both_axes() {
        let (qx, qy, qw, qh, radius_frac, sigma_x_frac, sigma_y_frac) =
            shadow_sdf_params(10.0, 20.0, 100.0, 100.0, 10.0, 5.0);
        assert_eq!(radius_frac, 0.2, "10px radius / 50px half-dimension");
        assert_eq!(sigma_x_frac, 0.1, "5px blur / 50px half-width");
        assert_eq!(sigma_y_frac, 0.1, "5px blur / 50px half-height");
        assert_eq!(
            (qx, qy, qw, qh),
            (5.0, 15.0, 110.0, 110.0),
            "quad expanded by blur_px on every side"
        );
    }

    #[test]
    fn a_wide_box_has_a_smaller_sigma_fraction_on_its_own_longer_axis() {
        // Same real 5px blur, but width (200) is double height (100):
        // the SAME pixel blur is a SMALLER fraction of the wider axis's
        // own half-extent.
        let (_qx, _qy, _qw, _qh, _radius_frac, sigma_x_frac, sigma_y_frac) =
            shadow_sdf_params(0.0, 0.0, 200.0, 100.0, 0.0, 5.0);
        assert_eq!(sigma_x_frac, 0.05, "5px blur / 100px half-width");
        assert_eq!(sigma_y_frac, 0.1, "5px blur / 50px half-height");
        assert!(
            sigma_x_frac < sigma_y_frac,
            "the wider axis's own real pixel blur must map to the smaller fraction"
        );
    }

    #[test]
    fn a_radius_larger_than_the_boxs_own_smaller_half_dimension_clamps_to_one() {
        // half_w = 20, half_h = 50 -- min_half = 20; a 50px radius request
        // would be 2.5x that, clamped to the real max of 1.0.
        let (.., radius_frac, _sx, _sy) = shadow_sdf_params(0.0, 0.0, 40.0, 100.0, 50.0, 0.0);
        assert_eq!(
            radius_frac, 1.0,
            "a radius past the box's own smaller half-dimension clamps to 1.0"
        );
    }

    #[test]
    fn a_zero_size_box_degrades_to_zero_fractions_not_a_panic() {
        let (_qx, _qy, qw, qh, radius_frac, sigma_x_frac, sigma_y_frac) =
            shadow_sdf_params(0.0, 0.0, 0.0, 0.0, 10.0, 5.0);
        assert_eq!((radius_frac, sigma_x_frac, sigma_y_frac), (0.0, 0.0, 0.0));
        assert_eq!(
            (qw, qh),
            (10.0, 10.0),
            "the quad is still real: a 5px margin on each side of a point"
        );
    }
}
