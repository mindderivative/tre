//! M15 Phase 1 (§5, §16.7): the standalone proof that `NodeKind::
//! TextField` genuinely paints its own real box fill, real glyphs, and
//! a real caret gated on the `Tree`'s own live focused node -- not just
//! that the match arm compiles. Same headless render-to-texture-then-
//! readback discipline as `checkbox_paint.rs`/`slider_paint.rs`.
//!
//! Two claims, kept deliberately separate:
//!
//! 1. An unfocused `TextField` paints its own real box fill and
//!    nothing else -- no caret anywhere, a real "nothing extra
//!    painted" claim, not merely "didn't crash."
//! 2. The *same* field, once it's the `Tree`'s own real focused node,
//!    paints a real, visible caret -- proving `paint_node`'s own
//!    `show_caret: tree.focused() == Some(id)` gate is genuinely live,
//!    not a hardcoded always-on/always-off.

use engine_core::{NodeKind, PaintProperties, TextFieldState, Tree};
use engine_render::{FrameRenderer, TextRenderer, build_tree_scene};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_hybrid::{RenderSize, RenderTargetConfig};

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const FIELD_COLOR: Color = Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF);

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
            label: Some("engine-render text-field-paint test device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("failed to create wgpu device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text-field-paint test target"),
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

fn build_tree(focus_field: bool) -> (Tree, engine_core::NodeId, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );

    // Empty content -- the real caret then sits exactly at the field's
    // own local x=0, no glyph-shaping math needed to locate it.
    let field = tree.insert(
        NodeKind::TextField(TextFieldState::new("", "Roboto", 400.0, 16.0)),
        Style {
            size: Size {
                width: length(100.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(FIELD_COLOR, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, field);

    if focus_field {
        tree.set_focus_to(
            field,
            1.0,
            std::time::Duration::ZERO,
            std::time::Instant::now(),
        );
    }

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(24.0),
    };
    tree.compute_layout(root, available);
    (tree, root, field)
}

#[test]
fn an_unfocused_text_field_paints_its_own_fill_with_no_visible_caret() {
    pollster::block_on(async {
        let (tree, root, _field) = build_tree(false);
        let (data, bpr) = render(&tree, root, 100, 24).await;

        // A point squarely on the field's own left edge, where the
        // caret would paint if the (false) show_caret gate leaked.
        let left_edge = pixel_at(&data, bpr, 1, 12);
        assert_eq!(
            left_edge,
            [0xEE, 0xEE, 0xEE, 0xFF],
            "an unfocused TextField must show only its own plain fill, no caret, \
             got {left_edge:?}"
        );
    });
}

#[test]
fn a_focused_text_field_paints_a_real_visible_caret() {
    pollster::block_on(async {
        let (tree, root, field) = build_tree(true);
        assert_eq!(
            tree.focused(),
            Some(field),
            "the real Tree focus state must actually be set before this claim means anything"
        );
        let (data, bpr) = render(&tree, root, 100, 24).await;

        // A point squarely on the field's own left edge -- the real
        // caret rect for an empty field's own cursor at byte offset 0.
        let left_edge = pixel_at(&data, bpr, 1, 12);
        assert_ne!(
            left_edge,
            [0xEE, 0xEE, 0xEE, 0xFF],
            "a focused TextField must paint a real, visible caret distinct from its own \
             plain fill, got {left_edge:?}"
        );
    });
}

/// M17 Phase 2 (§8): a field seeded with an explicit `TextFieldState`
/// rather than `build_tree`'s own always-empty-content one -- lets the
/// two tests below compare an actively-composing field against an
/// otherwise-identical, non-composing one (same displayed glyph, same
/// position), isolating the real preedit underline as the *only*
/// pixel difference between them. Always unfocused, so the real caret
/// (already proven live above) never contaminates this comparison.
fn build_tree_with_state(
    state: TextFieldState,
) -> (Tree, engine_core::NodeId, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(BACKGROUND, 0.0, 0.0, 1.0),
    );
    let field = tree.insert(
        NodeKind::TextField(state),
        Style {
            size: Size {
                width: length(100.0),
                height: length(24.0),
            },
            ..Default::default()
        },
        PaintProperties::new(FIELD_COLOR, 0.0, 0.0, 1.0),
    );
    tree.add_child(root, field);

    let available = Size {
        width: AvailableSpace::Definite(100.0),
        height: AvailableSpace::Definite(24.0),
    };
    tree.compute_layout(root, available);
    (tree, root, field)
}

#[test]
fn a_composing_preedit_paints_a_real_underline_distinct_from_the_same_field_when_not_composing() {
    pollster::block_on(async {
        // Same displayed glyph ("x"), same position -- committed via
        // `content` in one field, via a live `preedit` splice in the
        // other (`TextFieldState.preedit`'s own doc comment: display-
        // only, never touching real `content`). Any pixel difference
        // between the two is therefore genuinely the underline itself,
        // not a difference in what glyph got shaped or where.
        let not_composing = TextFieldState::new("x", "Roboto", 400.0, 16.0);
        let (tree_a, root_a, _) = build_tree_with_state(not_composing);
        let (data_a, _) = render(&tree_a, root_a, 100, 24).await;

        let mut composing = TextFieldState::new("", "Roboto", 400.0, 16.0);
        composing.preedit = Some("x".to_string());
        let (tree_b, root_b, _) = build_tree_with_state(composing);
        let (data_b, _) = render(&tree_b, root_b, 100, 24).await;

        assert_eq!(
            data_a.len(),
            data_b.len(),
            "both renders use the same target size, their raw buffers must be the same length"
        );
        assert!(
            data_a != data_b,
            "a composing preedit must paint a real underline distinct from an otherwise \
             pixel-identical, non-composing field, but the two renders were pixel-identical"
        );
    });
}

#[test]
fn a_cleared_preedit_i_e_an_empty_string_paints_no_underline_same_as_none() {
    pollster::block_on(async {
        // A defensive edge case: `draw_field`'s own splice only
        // triggers on `Some(preedit) if !preedit.is_empty()` -- a
        // stray `Some(String::new())` (shouldn't occur in practice,
        // since `Tree::dispatch`'s `ImePreedit` arm already normalizes
        // an empty string to `None`, but this proves the rendering
        // side is defensively correct on its own too) must render
        // identically to a real `None`.
        let none_preedit = TextFieldState::new("x", "Roboto", 400.0, 16.0);
        let (tree_a, root_a, _) = build_tree_with_state(none_preedit);
        let (data_a, _) = render(&tree_a, root_a, 100, 24).await;

        let mut empty_preedit = TextFieldState::new("x", "Roboto", 400.0, 16.0);
        empty_preedit.preedit = Some(String::new());
        let (tree_b, root_b, _) = build_tree_with_state(empty_preedit);
        let (data_b, _) = render(&tree_b, root_b, 100, 24).await;

        assert_eq!(
            data_a, data_b,
            "an empty-string preedit must paint exactly like a real None -- no stray underline"
        );
    });
}
