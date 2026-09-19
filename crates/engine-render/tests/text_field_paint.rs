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
use engine_render::{FrameRenderer, TextPlacement, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Point;
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

/// M18 Phase 1 (§8, §10, §11.9, §11.10): `hit_test_position`'s own real
/// claim -- it genuinely reaches `parley`'s own shaped `Layout`, not a
/// stub. No GPU/pixel readback needed at all (unlike every test above):
/// `TextRenderer::new()` is pure CPU-side font/layout setup, and `hit_
/// test_position` returns a plain `usize`, no `Scene`/`Resources`
/// involved -- the same reason `checkbox_paint.rs`-style pixel tests
/// exist for *painting* claims but a plain unit test suffices here.
#[test]
fn hit_test_position_at_the_very_start_of_the_field_returns_byte_offset_zero() {
    let mut renderer = TextRenderer::new();
    let state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
    let at = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };
    let offset = renderer.hit_test_position(&state, at, Point::new(0.0, 8.0));
    assert_eq!(
        offset, 0,
        "a click squarely on the field's own left edge must resolve to byte offset 0"
    );
}

#[test]
fn hit_test_position_far_past_the_end_returns_the_full_content_length() {
    let mut renderer = TextRenderer::new();
    let state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
    let at = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };
    // Well past where 5 real glyphs at 16px could possibly reach --
    // `parley::editing::Cursor::from_point`'s own real fallback (no
    // cluster hit) is the layout's own real text length, confirmed via
    // direct source read.
    let offset = renderer.hit_test_position(&state, at, Point::new(1000.0, 8.0));
    assert_eq!(
        offset,
        state.content.len(),
        "a click far past every real glyph must resolve to the end of the real content"
    );
}

#[test]
fn hit_test_position_accounts_for_the_placements_own_local_offset() {
    // The same field, painted at a real, non-zero (x, y) -- a click at
    // that same offset must still resolve to byte 0, proving `at.x`/
    // `at.y` are genuinely subtracted before reaching `Cursor::
    // from_point`, not silently ignored.
    let mut renderer = TextRenderer::new();
    let state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
    let at = TextPlacement {
        x: 40.0,
        y: 12.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };
    let offset = renderer.hit_test_position(&state, at, Point::new(40.0, 20.0));
    assert_eq!(offset, 0);
}

/// M30 Phase 9 Step 3 (§5, §8, §10): `Code Editor`'s own real,
/// load-bearing multiline claim -- a genuine `\n` in a multiline
/// field's own content produces real, vertically-stacked `parley`
/// layout lines, not one squashed/ignored line. Proven the identical
/// real, GPU-free way `hit_test_position`'s own claim above already
/// is: a click well below the first line's own height must resolve
/// inside the *second* line's own real content ("cd", bytes 3..5),
/// never back into the first ("ab", bytes 0..2) -- the real,
/// observable consequence of two real lines existing at all.
#[test]
fn a_multiline_fields_own_newline_produces_a_real_second_layout_line() {
    let mut renderer = TextRenderer::new();
    let mut state = TextFieldState::new("ab\ncd", "Roboto", 400.0, 16.0);
    state.multiline = true;
    let placement = || TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };

    let first_line_offset = renderer.hit_test_position(&state, placement(), Point::new(0.0, 4.0));
    assert!(
        first_line_offset <= 2,
        "a click near the field's own top-left must resolve inside \"ab\" (bytes 0..2), got \
         byte {first_line_offset}"
    );

    // Comfortably below any real 16px line's own height (parley's real
    // line metrics are never anywhere near this tall) -- unambiguously
    // the *second* real line if one genuinely exists, the last real
    // line otherwise (there are only two here either way).
    let second_line_offset = renderer.hit_test_position(&state, placement(), Point::new(0.0, 60.0));
    assert!(
        second_line_offset >= 3,
        "a click well below the first line must resolve inside \"cd\" (bytes 3..5), on a real \
         second layout line, not fall back into \"ab\", got byte {second_line_offset}"
    );
}

/// M31 Phase 3 (§5, §8): the real, definitive proof
/// `show_whitespace`'s own byte-offset remapping is correct, not just
/// plausible -- `·`/`→` are wider, multi-byte substitutes for a
/// single-byte space/tab, so a naive implementation could easily
/// return a *display*-space byte offset (out of bounds, or landing
/// mid-character) instead of a real, valid offset into `state.
/// content`. A click far past the end of a short, substituted field
/// must resolve to `state.content.len()` -- exactly 3 for `"a b"`, not
/// 4 (`"a\u{B7}b"`'s own real substituted length).
#[test]
fn hit_test_position_on_a_field_with_visible_whitespace_returns_real_content_offsets() {
    let mut renderer = TextRenderer::new();
    let state = {
        let mut s = TextFieldState::new("a b", "Roboto", 400.0, 16.0);
        s.show_whitespace = true;
        s
    };
    let placement = || TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };

    let start = renderer.hit_test_position(&state, placement(), Point::new(0.0, 4.0));
    assert_eq!(
        start, 0,
        "a click at the field's own start must resolve to real byte 0"
    );

    let far_right = renderer.hit_test_position(&state, placement(), Point::new(1000.0, 4.0));
    assert_eq!(
        far_right,
        state.content.len(),
        "a click far past the end must resolve to state.content's own real length (3), not the \
         longer substituted string's own length (4) -- a broken remap would return an invalid, \
         out-of-bounds byte offset here"
    );
}

/// M20 Phase 2 (§7.1, §7.3): the real, definitive proof `text_tint` is
/// genuinely read at paint time, not just stored -- the same real diff
/// -based proof `a_composing_preedit_paints_a_real_underline_distinct_
/// from_the_same_field_when_not_composing` (M17 Phase 2) already
/// established for a similarly hard-to-pin-down-exact-pixel claim: two
/// otherwise-identical fields, one with the real default `text_tint`,
/// one with a real, different one, must paint genuinely different
/// pixels.
#[test]
fn a_themed_text_field_paints_genuinely_different_pixels_than_the_default() {
    pollster::block_on(async {
        let default_state = TextFieldState::new("x", "Roboto", 400.0, 16.0);
        let (tree_a, root_a, _) = build_tree_with_state(default_state);
        let (data_a, _) = render(&tree_a, root_a, 100, 24).await;

        let mut themed_state = TextFieldState::new("x", "Roboto", 400.0, 16.0);
        themed_state.text_tint = Color::from_rgba8(0x00, 0xFF, 0x00, 0xFF);
        let (tree_b, root_b, _) = build_tree_with_state(themed_state);
        let (data_b, _) = render(&tree_b, root_b, 100, 24).await;

        assert!(
            data_a != data_b,
            "a real, non-default text_tint must paint genuinely different pixels than the \
             default -- the two renders were pixel-identical"
        );
    });
}

/// M31 Phase 4 (§5, §8): the real, definitive proof `syntax_spans`
/// genuinely colors *only* the real byte range it names, not the
/// whole field (which a bug in the real run-to-span matching could
/// easily produce) and not nothing at all (which a bug in the real
/// `Brush`-push-forces-a-run-split reasoning could equally produce).
/// Three real renders of the same two-character content, diffed
/// pairwise the identical way `a_themed_text_field_paints_genuinely_
/// different_pixels_than_the_default` already proves a hard-to-pin-
/// exact-pixel claim: no spans at all; one span covering the whole
/// content; one span covering only the first real character. All
/// three must be pixel-distinct from each other -- if the "half"
/// render matched the "whole" one, coloring leaked past its own real
/// span; if it matched "none", the span was silently ignored.
#[test]
fn syntax_spans_color_only_their_own_real_byte_range() {
    pollster::block_on(async {
        let none_state = TextFieldState::new("ab", "Roboto", 400.0, 16.0);
        let (tree_none, root_none, _) = build_tree_with_state(none_state);
        let (data_none, _) = render(&tree_none, root_none, 100, 24).await;

        let red = Color::from_rgba8(0xFF, 0x00, 0x00, 0xFF);

        let mut whole_state = TextFieldState::new("ab", "Roboto", 400.0, 16.0);
        whole_state.syntax_spans = vec![(0..2, red)];
        let (tree_whole, root_whole, _) = build_tree_with_state(whole_state);
        let (data_whole, _) = render(&tree_whole, root_whole, 100, 24).await;

        let mut half_state = TextFieldState::new("ab", "Roboto", 400.0, 16.0);
        half_state.syntax_spans = vec![(0..1, red)];
        let (tree_half, root_half, _) = build_tree_with_state(half_state);
        let (data_half, _) = render(&tree_half, root_half, 100, 24).await;

        assert!(
            data_whole != data_none,
            "a real syntax span covering the whole field must paint genuinely different \
             pixels than no spans at all"
        );
        assert!(
            data_half != data_whole,
            "a real syntax span covering only the first character must paint genuinely \
             different pixels than one covering the whole field -- coloring leaked past its \
             own real byte range"
        );
        assert!(
            data_half != data_none,
            "a real syntax span covering only the first character must still paint genuinely \
             different pixels than no spans at all -- the span was silently ignored"
        );
    });
}

/// M31 Phase 5 (§5, §8): the real, definitive proof a folded range
/// genuinely paints differently than its own real unfolded content --
/// a real render of `"hello world"` with `"llo wor"` (bytes 2..9)
/// folded must produce genuinely different pixels than the same real
/// content painted unfolded, the same diff-based proof this file's
/// own hard-to-pin-exact-pixel claims already use.
#[test]
#[allow(clippy::single_range_in_vec_init)]
fn a_folded_range_paints_genuinely_different_pixels_than_unfolded() {
    pollster::block_on(async {
        let unfolded_state = TextFieldState::new("hello world", "Roboto", 400.0, 16.0);
        let (tree_unfolded, root_unfolded, _) = build_tree_with_state(unfolded_state);
        let (data_unfolded, _) = render(&tree_unfolded, root_unfolded, 100, 24).await;

        let mut folded_state = TextFieldState::new("hello world", "Roboto", 400.0, 16.0);
        folded_state.folded_ranges = vec![2..9];
        let (tree_folded, root_folded, _) = build_tree_with_state(folded_state);
        let (data_folded, _) = render(&tree_folded, root_folded, 100, 24).await;

        assert!(
            data_folded != data_unfolded,
            "a real folded range must paint genuinely different pixels than the same content \
             unfolded -- the two renders were pixel-identical"
        );
    });
}

/// M31 Phase 5 (§5, §8): `hit_test_position`'s own real proof that a
/// click past a real folded range resolves to a real, valid offset
/// into `state.content` (not the longer, unfolded content's own
/// length, and not the shorter, marker-collapsed display length --
/// exactly the class of bug a broken fold-aware remap would produce,
/// the identical real shape `hit_test_position_on_a_field_with_
/// visible_whitespace_returns_real_content_offsets` already proves
/// for whitespace substitution).
#[test]
#[allow(clippy::single_range_in_vec_init)]
fn hit_test_position_on_a_folded_field_returns_real_content_offsets() {
    let mut renderer = TextRenderer::new();
    let state = {
        let mut s = TextFieldState::new("hello world", "Roboto", 400.0, 16.0);
        s.folded_ranges = vec![2..9]; // collapses "llo wor" to one marker
        s
    };
    let placement = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF),
    };

    let far_right = renderer.hit_test_position(&state, placement, Point::new(1000.0, 4.0));
    assert_eq!(
        far_right,
        state.content.len(),
        "a click far past a folded field's own end must resolve to state.content's own real \
         length (11), neither the shorter marker-collapsed display length nor an out-of-bounds \
         offset"
    );
}
