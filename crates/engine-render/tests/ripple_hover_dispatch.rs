//! M4 Phase 5 (§7.3): proves ripple/hover are visible through the real
//! render pipeline (`build_tree_scene`/`paint_node`), not just correctly
//! *animated* inside `engine-core` (already true and unit-tested since
//! M3 Phase 5 step 9 / M4 Phase 1). Before this phase, `paint_node`
//! never read `Node::interaction` at all -- only the standalone
//! `build_ripple_scene` spike (no real `Tree` involved) ever drew one.
//!
//! Also proves a real, confirmed asymmetry in `Tree::dispatch` found
//! while investigating this phase: a real primary-button press spawns a
//! ripple on *any* hit node via `interaction_mut`'s own lazy-create
//! ("opts one node into interaction state... lazily creating it on
//! first use") -- no prior opt-in needed, unlike `update_hover`, which
//! `tree.rs`'s own doc comment states explicitly "never lazily creates
//! one just because a node happened to be hovered." Both facts are
//! exercised here exactly as they really behave, not as this phase
//! originally assumed ("ripple needs a Python-facing opt-in too") --
//! confirmed wrong by reading `dispatch`'s actual `PointerPressed` arm
//! before writing this test.
//!
//! Same headless render-to-texture-then-readback discipline as
//! `splitter_drag_dispatch.rs`.

use std::time::{Duration, Instant};

use engine_core::{InputEvent, InteractionConfig, NodeKind, PaintProperties, PointerButton, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

async fn render(tree: &Tree, root: engine_core::NodeId, width: u16, height: u16) -> (Vec<u8>, u32) {
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
            label: Some("engine-render ripple-hover-dispatch test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ripple-hover-dispatch test target"),
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
    (out, bytes_per_row)
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

fn white_rect_tree(width: u16, height: u16) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(f32::from(width)),
                height: length(f32::from(height)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xFF), 0.0, 0.0, 1.0),
    );
    let available = Size {
        width: AvailableSpace::Definite(f32::from(width)),
        height: AvailableSpace::Definite(f32::from(height)),
    };
    tree.compute_layout(root, available);
    (tree, root)
}

fn config() -> InteractionConfig {
    InteractionConfig {
        hover_opacity: 0.5,
        hover_duration: Duration::from_millis(100),
        focus_ring_opacity: 1.0,
        focus_ring_duration: Duration::from_millis(100),
        ripple_radius: 15.0,
        ripple_opacity: 1.0,
        ripple_duration: Duration::from_millis(200),
    }
}

#[test]
fn a_real_dispatched_press_paints_a_visible_ripple_with_no_prior_opt_in_needed() {
    pollster::block_on(async {
        let (width, height) = (120u16, 60u16);
        let (mut tree, root) = white_rect_tree(width, height);
        let cfg = config();

        let (before_data, before_bpr) = render(&tree, root, width, height).await;
        assert_eq!(
            pixel_at(&before_data, before_bpr, 30, 30),
            [0xFF, 0xFF, 0xFF, 0xFF],
            "before any press, the node must render as plain white -- interaction is None \
             and paint_node's new overlay code must be a true no-op"
        );

        // No `interaction_mut` call here on purpose -- `Tree::dispatch`'s
        // `PointerPressed` arm already lazily creates `InteractionState`
        // for whatever node it hits (confirmed by reading the real match
        // arm), so a ripple must appear with zero prior opt-in.
        let now = Instant::now();
        tree.dispatch(
            root,
            InputEvent::PointerPressed {
                position: Point::new(30.0, 30.0),
                button: PointerButton::Primary,
            },
            &cfg,
            now,
        );
        tree.tick_all(now + Duration::from_millis(100)); // halfway through the 200ms ripple

        let (after_data, after_bpr) = render(&tree, root, width, height).await;
        let at_origin = pixel_at(&after_data, after_bpr, 30, 30);
        assert!(
            at_origin[0] < 0xFF,
            "the ripple's own origin must be visibly darker than the plain white background, \
             got {at_origin:?}"
        );
        let far_corner = pixel_at(&after_data, after_bpr, 110, 50);
        assert_eq!(
            far_corner,
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a point well outside the ripple's own radius must stay untouched, got {far_corner:?}"
        );
    });
}

#[test]
fn hover_only_paints_after_a_real_opt_in_even_though_the_pointer_move_is_real() {
    pollster::block_on(async {
        let (width, height) = (120u16, 60u16);
        let (mut tree, root) = white_rect_tree(width, height);
        let cfg = config();
        let now = Instant::now();

        // A real `PointerMoved` dispatched directly over the node,
        // *without* opting it into interaction first -- `update_hover`'s
        // own doc comment states it "never lazily creates" `InteractionState`
        // the way `interaction_mut` does, so this must render as if
        // nothing happened at all.
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(60.0, 30.0),
            },
            &cfg,
            now,
        );
        tree.tick_all(now + Duration::from_millis(100));
        let (not_opted_in, bpr1) = render(&tree, root, width, height).await;
        assert_eq!(
            pixel_at(&not_opted_in, bpr1, 60, 30),
            [0xFF, 0xFF, 0xFF, 0xFF],
            "a node that never opted into interaction must never show a hover tint, even \
             under a real PointerMoved dispatch directly over it"
        );

        // `update_hover` is a no-op when the hit node is unchanged from
        // last call ("a repeated call with the same result is a no-op"),
        // so the pointer must genuinely leave and re-enter to produce a
        // real hover transition -- moving off-node first, matching how
        // a real cursor would actually have to move to hover again.
        let now2 = now + Duration::from_millis(200);
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(200.0, 30.0),
            },
            &cfg,
            now2,
        );

        // The real, minimal opt-in this phase adds a Python-facing
        // method for (`Node.enable_interaction`) -- at the engine-core
        // level, it's exactly this one call.
        tree.interaction_mut(root);
        let now3 = now2 + Duration::from_millis(50);
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(60.0, 30.0),
            },
            &cfg,
            now3,
        );
        tree.tick_all(now3 + Duration::from_millis(100)); // past hover_duration -- stable at its target

        let (opted_in, bpr2) = render(&tree, root, width, height).await;
        let center = pixel_at(&opted_in, bpr2, 60, 30);
        assert!(
            center[0] < 0xFF,
            "after opting in, the same real pointer-move dispatch must now show a hover tint, \
             got {center:?}"
        );
        let corner = pixel_at(&opted_in, bpr2, 10, 10);
        assert_eq!(
            corner, center,
            "hover covers the node's whole bounds uniformly, unlike a localized ripple -- \
             got corner {corner:?} vs center {center:?}"
        );
    });
}

/// M7 Phase 3 (§7.1): before this phase, the hover/ripple overlay was a
/// hardcoded `Color::from_rgba8(0, 0, 0, 255)` -- any tint blended over
/// white only ever produced *gray*, never a real hue. Sets a real,
/// non-black `InteractionState::tint` directly (the same field `engine-
/// py::Window.set_theme`/`Node.enable_interaction` populate) and proves
/// `paint_node` actually reads it: a red-channel-dominant overlay must
/// pull the green/blue channels down further than red, something a
/// hardcoded gray blend could never produce.
#[test]
fn hover_overlay_paints_the_real_interaction_tint_not_a_hardcoded_gray() {
    pollster::block_on(async {
        let (width, height) = (120u16, 60u16);
        let (mut tree, root) = white_rect_tree(width, height);
        let cfg = config();
        let now = Instant::now();

        tree.interaction_mut(root).unwrap().tint = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);
        tree.dispatch(
            root,
            InputEvent::PointerMoved {
                position: Point::new(60.0, 30.0),
            },
            &cfg,
            now,
        );
        tree.tick_all(now + Duration::from_millis(100));

        let (data, bpr) = render(&tree, root, width, height).await;
        let center = pixel_at(&data, bpr, 60, 30);
        assert!(
            center[0] > center[1] && center[0] > center[2],
            "a red interaction tint must leave the red channel visibly higher than green/blue \
             (a hardcoded black/gray tint would keep all three equal), got {center:?}"
        );
        assert!(
            center[1] < 0xFF,
            "the overlay must still actually paint something (green channel pulled down from \
             the plain white background), got {center:?}"
        );
    });
}
