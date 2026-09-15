//! The three geometrically-scaled workloads the Time-Ramp Stress Test
//! (and the Interaction-Driven resize test's own fixed scene) draw from
//! -- each a real, already-proven pipeline elsewhere in the workspace,
//! not a new shader or rendering path invented for this tool.
//!
//! The spec named "shape rendering, textures, shaders" as example
//! stress categories. `shapes`/`textures` map directly onto
//! `crates/tre-rhi-vulkan/examples/shape_registry_zero_alloc_demo.rs`'s
//! and `texture_fill_demo.rs`'s own solid-fill/textured `Rectangle`
//! constructions. `shaders` was originally planned as MSDF `Text`
//! rendering (the engine's own most fragment-shader-costly pipeline),
//! but building it would need `TextFlattenContext`/`FontRegistry`/glyph-
//! atlas plumbing that no existing Rust example exercises at all (every
//! real Rust `Text`-via-`ShapeRegistry` precedent lives only on the
//! Python side) -- real, new scope disproportionate to this tool.
//! Substituted with `FillStyle::Gradient` instead: real, meaningfully
//! more per-pixel shader work than a solid fill (interpolated gradient-
//! stop evaluation, `PipelineKind::GradientFill`/the `SdfRectStyled`
//! gradient branch), and fully proven already in
//! `gradient_fill_demo.rs` with zero new plumbing needed.

use tre_engine::{
    rgba8, FillStyle, GradientDef, GradientKind, GradientStop, PrimitiveCommon, Rectangle,
    RhiDevice, ShapePrimitive, ShapeRegistry,
};
use tre_rhi_vulkan::VulkanDevice;

/// A shape's own grid cell size -- matches
/// `crates/tre-engine/benches/frame_processing.rs`'s own `(i % 100, i /
/// 100)` grid-layout convention, so a large `count` tiles the window
/// instead of stacking every shape at one point.
const CELL_SIZE: f32 = 8.0;
const RECT_SIZE: f32 = 6.0;
const GRID_COLUMNS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workload {
    Shapes,
    Textures,
    Shaders,
}

impl Workload {
    pub const ALL: [Workload; 3] = [Workload::Shapes, Workload::Textures, Workload::Shaders];

    pub fn name(self) -> &'static str {
        match self {
            Workload::Shapes => "shapes",
            Workload::Textures => "textures",
            Workload::Shaders => "shaders",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "shapes" => Some(Workload::Shapes),
            "textures" => Some(Workload::Textures),
            "shaders" => Some(Workload::Shaders),
            _ => None,
        }
    }
}

/// Real resources a workload needs once, up front, reused across every
/// ramp tier -- a bindless texture index for `textures` (the underlying
/// GPU texture is a device-level resource, unrelated to any one
/// `ShapeRegistry`), nothing for the other two.
pub struct WorkloadResources {
    texture_index: u32,
}

impl WorkloadResources {
    /// A real four-quadrant flag texture, matching `texture_fill_demo.rs`'s
    /// own established fixture exactly (pure 0/255 channel values so
    /// sRGB round-trips exactly -- irrelevant here since this tool
    /// never reads pixels back, kept for parity with the proven demo
    /// this borrows from).
    pub fn build(device: &VulkanDevice) -> Self {
        const SIZE: u32 = 8;
        let half = SIZE / 2;
        let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let (b, g, r) = match (x < half, y < half) {
                    (true, true) => (0u8, 0u8, 255u8),
                    (false, true) => (0, 255, 0),
                    (true, false) => (255, 0, 0),
                    (false, false) => (0, 255, 255),
                };
                pixels.extend_from_slice(&[b, g, r, 255]);
            }
        }
        let texture = device
            .create_texture(SIZE, SIZE, tre_engine::TextureFormat::Bgra8Srgb, &pixels)
            .expect("failed to create tre-perf-suite's workload texture");
        let texture_index = texture
            .bindless_index()
            .expect("create_texture always registers a real bindless index");
        // `texture` itself is intentionally leaked (`std::mem::forget`)
        // rather than stored: this tool's own process lifetime is the
        // texture's lifetime (it runs one ramp test end to end and
        // exits), and keeping the `Box<dyn RhiTexture>` alive would
        // require threading it through every workload/registry
        // rebuild for no real benefit -- the bindless *index* is all
        // any later `Rectangle::fill` needs.
        std::mem::forget(texture);
        Self { texture_index }
    }
}

/// Inserts `count` shapes of `workload`'s own kind into `registry`,
/// laid out on a simple grid.
pub fn populate(
    workload: Workload,
    registry: &mut ShapeRegistry,
    resources: &WorkloadResources,
    count: usize,
) {
    match workload {
        Workload::Shapes => {
            for i in 0..count {
                registry.insert(ShapePrimitive::Rectangle(rect_at(
                    i,
                    FillStyle::Solid(rgba8(0xE0, 0xA0, 0x40, 0xFF)),
                )));
            }
        }
        Workload::Textures => {
            for i in 0..count {
                registry.insert(ShapePrimitive::Rectangle(rect_at(
                    i,
                    FillStyle::Texture(resources.texture_index),
                )));
            }
        }
        Workload::Shaders => {
            let gradient_id = registry
                .create_gradient(GradientDef {
                    kind: GradientKind::Linear {
                        start: [0.0, 0.0],
                        end: [RECT_SIZE, RECT_SIZE],
                    },
                    stops: vec![
                        GradientStop {
                            position: 0.0,
                            color: rgba8(0xE0, 0x40, 0x40, 0xFF),
                        },
                        GradientStop {
                            position: 1.0,
                            color: rgba8(0x40, 0x40, 0xE0, 0xFF),
                        },
                    ],
                })
                .expect("tre-perf-suite's workload gradient definition is valid by construction");
            for i in 0..count {
                registry.insert(ShapePrimitive::Rectangle(rect_at(
                    i,
                    FillStyle::Gradient(gradient_id),
                )));
            }
        }
    }
}

fn rect_at(i: usize, fill: FillStyle) -> Rectangle {
    let col = i % GRID_COLUMNS;
    let row = i / GRID_COLUMNS;
    #[allow(
        clippy::cast_precision_loss,
        reason = "grid position stays well within f32's exact-integer range for any primitive \
                  count this tool's ramp ever reaches"
    )]
    let (x, y) = (col as f32 * CELL_SIZE, row as f32 * CELL_SIZE);
    let mut rect = Rectangle::new([RECT_SIZE, RECT_SIZE], 0);
    rect.common = PrimitiveCommon::new();
    rect.common.transform.position = [x, y];
    rect.fill = fill;
    rect
}
