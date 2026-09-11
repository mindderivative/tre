//! Registers every real shape-rendering pipeline (Phase 10 Steps 10.1-
//! 10.2.6) into a [`PipelineRegistry`] in one call, for any consumer that
//! wants to render an arbitrary [`tre_engine::ShapeRegistry`] scene
//! without hand-wiring each of the eight `PipelineKind`s itself the way
//! every demo up to now has (each one only ever wired the two or three
//! kinds its own scene happened to use).
//!
//! Shaders are embedded into this crate's own compiled binary at build
//! time (`include_bytes!` against `build.rs`'s `OUT_DIR`), not read from
//! disk at runtime the way every example does via `env!("OUT_DIR")` --
//! that macro only resolves correctly for source files compiled as part
//! of *this* crate's own build (which examples are); a downstream
//! consumer in a different crate (`tre-python`, and eventually `tre-ffi`)
//! has no such access, and a shipped Python wheel or C dynamic library
//! has no guarantee the build-time `OUT_DIR` path even exists on the
//! machine it's installed on. Embedding bakes the compiled SPIR-V
//! directly into this crate's own `.rlib`/`.so`, matching TECHNICAL.md
//! Section 9.3's "shader compilation happens in a Cargo build script,
//! never at runtime in a shipping build" -- extended here to mean the
//! shaders themselves ship inside the binary too, not as loose files
//! next to it.

use ash::vk;
use tre_engine::{EngineError, PipelineKind, PipelineRegistry, RhiDevice};

use crate::VulkanDevice;

macro_rules! spv {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name))
    };
}

const SDF_ROUNDED_RECT_VERT: &[u8] = spv!("sdf_rounded_rect.vert.spv");
const SDF_ROUNDED_RECT_FRAG: &[u8] = spv!("sdf_rounded_rect.frag.spv");
const SDF_RECT_STYLED_FRAG: &[u8] = spv!("sdf_rect_styled.frag.spv");
const SDF_ELLIPSE_FRAG: &[u8] = spv!("sdf_ellipse.frag.spv");
/// Public within this crate (Phase 13 Step 13.8: custom shader API) -- a
/// custom fragment shader is paired with this SAME real vertex shader
/// (via `VulkanDevice::create_custom_pipeline`), the identical one
/// `TexturedQuad`/`GradientFill`/`MsdfText` already use, so a custom
/// fragment shader can rely on the exact `frag_color`/`frag_uv`
/// varyings `bindless_textured.frag`'s own source declares.
pub(crate) const BINDLESS_TEXTURED_VERT: &[u8] = spv!("bindless_textured.vert.spv");
const BINDLESS_TEXTURED_FRAG: &[u8] = spv!("bindless_textured.frag.spv");
const GRADIENT_FILL_FRAG: &[u8] = spv!("gradient_fill.frag.spv");
const MSDF_FRAG: &[u8] = spv!("msdf.frag.spv");
const WALKING_SKELETON_VERT: &[u8] = spv!("walking_skeleton.vert.spv");
const WALKING_SKELETON_FRAG: &[u8] = spv!("walking_skeleton.frag.spv");
const FLAT_COLOR_BLEND_FRAG: &[u8] = spv!("flat_color_blend.frag.spv");

/// Registers every real shape-rendering pipeline into `registry`, against
/// `color_format` (the real swapchain/headless-target format a caller is
/// about to render into).
///
/// `PipelineKind::FlatColorBlend` is only registered when
/// `device.local_read_blend_supported()` is true -- matches this
/// project's own established fail-closed degradation elsewhere
/// (`ShapeRegistry`'s own `draw_polygon_fill` dispatch): a caller only
/// ever selects a non-`Normal` `BlendMode` when it means to, so skipping
/// registration on unsupported hardware makes that pipeline lookup fail
/// loudly at `execute_frame` time, not render silently wrong.
///
/// # Errors
/// Returns [`EngineError::PipelineCreationFailed`] if any pipeline fails
/// to compile or link.
pub fn register_shape_pipelines(
    device: &VulkanDevice,
    registry: &mut PipelineRegistry,
    color_format: vk::Format,
) -> Result<(), EngineError> {
    registry.register(
        PipelineKind::SdfRoundedRect as u16,
        Box::new(device.create_pipeline(
            SDF_ROUNDED_RECT_VERT,
            SDF_ROUNDED_RECT_FRAG,
            color_format,
        )?),
    );
    registry.register(
        PipelineKind::SdfRectStyled as u16,
        Box::new(device.create_pipeline(
            SDF_ROUNDED_RECT_VERT,
            SDF_RECT_STYLED_FRAG,
            color_format,
        )?),
    );
    registry.register(
        PipelineKind::SdfEllipse as u16,
        Box::new(device.create_pipeline(SDF_ROUNDED_RECT_VERT, SDF_ELLIPSE_FRAG, color_format)?),
    );
    registry.register(
        PipelineKind::TexturedQuad as u16,
        Box::new(device.create_pipeline(
            BINDLESS_TEXTURED_VERT,
            BINDLESS_TEXTURED_FRAG,
            color_format,
        )?),
    );
    registry.register(
        PipelineKind::GradientFill as u16,
        Box::new(device.create_pipeline(
            BINDLESS_TEXTURED_VERT,
            GRADIENT_FILL_FRAG,
            color_format,
        )?),
    );
    registry.register(
        PipelineKind::MsdfText as u16,
        Box::new(device.create_pipeline(BINDLESS_TEXTURED_VERT, MSDF_FRAG, color_format)?),
    );
    registry.register(
        PipelineKind::FlatColor as u16,
        Box::new(device.create_pipeline(
            WALKING_SKELETON_VERT,
            WALKING_SKELETON_FRAG,
            color_format,
        )?),
    );
    if device.local_read_blend_supported() {
        registry.register(
            PipelineKind::FlatColorBlend as u16,
            Box::new(device.create_blend_mode_pipeline(
                WALKING_SKELETON_VERT,
                FLAT_COLOR_BLEND_FRAG,
                color_format,
            )?),
        );
    }
    Ok(())
}
