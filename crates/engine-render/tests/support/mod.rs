//! The headless render-to-texture-then-readback harness the building-block
//! pixel tests share (`m95_paint.rs`, `m96_paint.rs`): a 100x100 target, a
//! `Frame` to read pixels from, and small tree-building helpers. Each test
//! binary uses part of it.
#![allow(dead_code)]

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use engine_render::{FrameRenderer, GeometryCache, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

pub const SIZE: u16 = 100;
pub const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
pub const RED: Color = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);
pub const WHITE: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
pub const BLACK: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0xFF);
pub const GREEN: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
pub const CLEAR: Color = Color::from_rgba8(0, 0, 0, 0);

pub async fn render(tree: &Tree, root: NodeId) -> (Vec<u8>, u32) {
    let (width, height) = (SIZE, SIZE);
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("no wgpu adapter available in this environment");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("engine-render m95 test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("m95 test target"),
        size: wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut frame_renderer = FrameRenderer::new(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: u32::from(width),
            height: u32::from(height),
        },
    );
    let mut text_renderer = TextRenderer::new();
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
    );
    let render_size = RenderSize {
        width: u32::from(width),
        height: u32::from(height),
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &device, &queue, &mut encoder, &render_size, &view);
    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("failed to map readback buffer");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");
    let data = slice.get_mapped_range();
    let mut out = vec![0u8; data.len()];
    out.copy_from_slice(&data);
    (out, bytes_per_row)
}

pub struct Frame {
    pub data: Vec<u8>,
    pub bytes_per_row: u32,
}

impl Frame {
    pub async fn of(tree: &Tree, root: NodeId) -> Self {
        let (data, bytes_per_row) = render(tree, root).await;
        Self {
            data,
            bytes_per_row,
        }
    }

    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let start = (y * self.bytes_per_row + x * 4) as usize;
        [
            self.data[start],
            self.data[start + 1],
            self.data[start + 2],
            self.data[start + 3],
        ]
    }

    /// Whether any pixel in the rectangle satisfies `test`.
    pub fn any_in(
        &self,
        x0: u32,
        y0: u32,
        x1: u32,
        y1: u32,
        test: impl Fn([u8; 4]) -> bool,
    ) -> bool {
        (y0..y1).any(|y| (x0..x1).any(|x| test(self.at(x, y))))
    }
}

pub fn rgba(color: Color) -> [u8; 4] {
    color.to_rgba8().to_u8_array()
}

pub fn close(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| a.abs_diff(e) <= tolerance)
}

/// A node placed absolutely at `(x, y)` with a `w` x `h` box.
pub fn placed(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x),
            top: length(y),
            right: taffy::prelude::auto(),
            bottom: taffy::prelude::auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

/// A 100x100 background root.
pub fn scene() -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(SIZE)),
                height: length(f32::from(SIZE)),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );
    (tree, root)
}

pub fn layout(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(SIZE)),
            height: AvailableSpace::Definite(f32::from(SIZE)),
        },
    );
}
