//! §14 build-order step 13: the standalone "one dropdown menu" proof of
//! §11.3's overlay mechanism. Same headless render-to-texture-then-
//! readback discipline as every other pixel-level proof in this crate.
//!
//! Three claims, kept deliberately separate:
//!
//! 1. `Tree::open_overlay` genuinely positions its content relative to
//!    the anchor's *absolute* bounds -- not the origin, not the
//!    anchor's own parent-relative location.
//! 2. §11.3's actual headline claim: paint order is children-list
//!    order, so an *appended* overlay paints on top of whatever it
//!    overlaps, with zero special-casing anywhere in `build_tree_scene`.
//!    Proven by deliberately overlapping the menu with a full-canvas
//!    background panel inserted *before* the overlay ever opens: if
//!    append-order didn't control paint order, the panel's color would
//!    still win at the overlap.
//! 3. `Tree::close_overlay` actually removes the node from what gets
//!    painted, not just from bookkeeping.

use engine_core::{NodeKind, OverlayMeta, PaintProperties, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const GREEN: Color = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF); // the background panel
const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF); // the anchor ("trigger" button)
const ORANGE: Color = Color::from_rgba8(0xFF, 0xA5, 0x00, 0xFF); // the dropdown menu

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(left),
            top: length(top),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(width),
            height: length(height),
        },
        ..Default::default()
    }
}

async fn render(tree: &Tree, root: engine_core::NodeId, width: u16, height: u16) -> Vec<u8> {
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
            label: Some("engine-render overlay-menu test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("overlay-menu test target"),
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
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        frame_renderer.resources_mut(),
        &mut text_renderer,
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
    out
}

fn pixel_at(data: &[u8], bytes_per_row: u32, x: u32, y: u32) -> [u8; 4] {
    let row_start = (y * bytes_per_row) as usize;
    let start = row_start + (x * 4) as usize;
    [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ]
}

#[test]
fn dropdown_menu_positions_by_anchor_and_paints_above_the_background_via_append_order() {
    pollster::block_on(async {
        let width: u16 = 300;
        let height: u16 = 300;
        let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);

        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                size: Size {
                    width: length(f32::from(width)),
                    height: length(f32::from(height)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        // Inserted (and therefore painted) first -- covers the whole
        // canvas, so anything painted after it that overlaps must be
        // the reason a later pixel isn't green.
        let background = tree.insert(
            NodeKind::Rect,
            absolute(0.0, 0.0, f32::from(width), f32::from(height)),
            PaintProperties::new(GREEN, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, background);

        // The "trigger" button the menu will anchor to.
        let anchor = tree.insert(
            NodeKind::Rect,
            absolute(20.0, 20.0, 80.0, 30.0),
            PaintProperties::new(BLUE, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, anchor);

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::Definite(f32::from(height)),
            },
        );

        // The dropdown menu -- 120x60, opened against `anchor`. Its
        // own explicit size stays what it's given; open_overlay only
        // ever touches position/inset.
        let menu = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(120.0),
                    height: length(60.0),
                },
                ..Default::default()
            },
            PaintProperties::new(ORANGE, 0.0, 0.0, 1.0),
        );
        tree.open_overlay(
            root,
            anchor,
            menu,
            OverlayMeta {
                anchor,
                dismiss_on_outside_click: true,
                dismiss_on_escape: true,
            },
        );
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::Definite(f32::from(height)),
            },
        );

        // The menu is expected at x:[20,140], y:[50,110] -- directly
        // below the anchor's own bottom edge (20+30=50).
        let data = render(&tree, root, width, height).await;

        let anchor_pixel = pixel_at(&data, bytes_per_row, 30, 30);
        assert_eq!(
            anchor_pixel,
            [0x00, 0x00, 0xFF, 0xFF],
            "the anchor's own area must still show its own color, pixel {anchor_pixel:?}"
        );

        let far_pixel = pixel_at(&data, bytes_per_row, 250, 250);
        assert_eq!(
            far_pixel,
            [0x00, 0xFF, 0x00, 0xFF],
            "an area covered only by the background panel must show its color, pixel {far_pixel:?}"
        );

        // Real claim 1 + 2 together: this point is inside the menu's
        // real anchor-relative position *and* also covered by the
        // green background panel underneath -- only correct
        // positioning *and* append-order-is-paint-order together
        // explain seeing orange here.
        let overlap_pixel = pixel_at(&data, bytes_per_row, 30, 80);
        assert_eq!(
            overlap_pixel,
            [0xFF, 0xA5, 0x00, 0xFF],
            "the menu must be positioned below its anchor and paint above the background panel \
             it overlaps, pixel {overlap_pixel:?}"
        );

        // Real claim 3: closing the overlay must actually remove it
        // from what gets painted, not just from bookkeeping.
        assert!(tree.close_overlay(menu));
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::Definite(f32::from(height)),
            },
        );
        let data_after_close = render(&tree, root, width, height).await;
        let overlap_pixel_after_close = pixel_at(&data_after_close, bytes_per_row, 30, 80);
        assert_eq!(
            overlap_pixel_after_close,
            [0x00, 0xFF, 0x00, 0xFF],
            "after closing, that same point must revert to the background panel's own color, \
             pixel {overlap_pixel_after_close:?}"
        );
    });
}
