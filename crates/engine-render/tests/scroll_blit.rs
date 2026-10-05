//! 0.5.4 (#126): a scrolled view is moved in the kept frame instead of redrawn.
//!
//! The only test of a block copy that matters is the pixels: every frame here
//! is drawn the way the app draws it (a `WindowRenderer`: persistent target, the
//! scroll shift, only the damage redrawn) and compared byte for byte with the
//! same tree drawn from nothing in full. Tests also check the shift was really
//! used where it should be (the damage is the uncovered strip, not the view)
//! and not where it must not be (the pixels still match, the damage is wide).

use engine_core::{
    ItemExtent, NodeId, NodeKind, PaintProperties, ScrollViewState, TextState, Tree,
    VirtualListState,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

mod support;

const W: f32 = 260.0;
const H: f32 = 180.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The scrolled view: position and size, logical.
const VIEW: (f32, f32, f32, f32) = (20.0, 20.0, 200.0, 120.0);
const ROW: f32 = 30.0;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        let (device, queue) = pollster::block_on(support::device("scroll blit test device"));
        Self { device, queue }
    }
}

struct Window {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    surface_view: wgpu::TextureView,
    width: u16,
    height: u16,
    prepare_time: std::time::Duration,
    draw_time: std::time::Duration,
    gpu_time: std::time::Duration,
}

impl Window {
    fn new(gpu: &Gpu, scale: f64) -> Self {
        let (width, height) = ((f64::from(W) * scale) as u16, (f64::from(H) * scale) as u16);
        let surface = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: u32::from(width),
                height: u32::from(height),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let surface_view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let mut renderer = WindowRenderer::new(
            &gpu.device,
            FORMAT,
            u32::from(width),
            u32::from(height),
            true,
        );
        renderer.set_scale(scale);
        // The scenes here are small: always use the shift, to test it.
        renderer.set_scroll_blit_min_nodes(0);
        Self {
            renderer,
            surface,
            surface_view,
            width,
            height,
            prepare_time: Default::default(),
            draw_time: Default::default(),
            gpu_time: Default::default(),
        }
    }

    fn frame(&mut self, gpu: &Gpu, tree: &Tree, root: NodeId, partial: bool) -> Damage {
        let began = std::time::Instant::now();
        let damage = self.renderer.prepare(
            tree,
            root,
            self.width,
            self.height,
            partial,
            &gpu.device,
            &gpu.queue,
        );
        self.prepare_time += began.elapsed();
        let began = std::time::Instant::now();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            tree,
            root,
            self.width,
            self.height,
            &damage,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &self.surface,
            &self.surface_view,
        );
        self.draw_time += began.elapsed();
        let began = std::time::Instant::now();
        gpu.queue.submit([encoder.finish()]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        self.gpu_time += began.elapsed();
        damage
    }

    fn pixels(&self, gpu: &Gpu) -> Vec<u8> {
        support::read_texture(&gpu.device, &gpu.queue, &self.surface)
    }
}

fn layout(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(W),
            height: AvailableSpace::Definite(H),
        },
    );
}

fn placed(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x),
            top: length(y),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

fn shade(i: usize) -> Color {
    Color::from_rgba8(
        (40 + (i * 37) % 200) as u8,
        (60 + (i * 91) % 180) as u8,
        (90 + (i * 53) % 150) as u8,
        255,
    )
}

fn label(text: &str) -> NodeKind {
    NodeKind::Text(TextState {
        content: text.into(),
        font_family: "Roboto".into(),
        font_weight: 400.0,
        font_size: 14.0,
        align: Default::default(),
        line_height: None,
        options: Default::default(),
    })
}

struct World {
    tree: Tree,
    root: NodeId,
    view: NodeId,
    rows: Vec<NodeId>,
}

/// A window with an opaque scroll view of `count` coloured rows, each with a
/// text label when `labels`.
fn scroll_world(count: usize, view_fill: Color, labels: bool) -> World {
    scroll_world_with(count, view_fill, labels, 0)
}

/// `scroll_world`, with `extra` more text nodes in each row.
fn scroll_world_with(count: usize, view_fill: Color, labels: bool, extra: usize) -> World {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(Color::from_rgba8(25, 25, 30, 255), 0.0, 1.0),
    );
    let (x, y, w, h) = VIEW;
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        placed(x, y, w, h),
        PaintProperties::new(view_fill, 0.0, 1.0),
    );
    tree.add_child(root, view);
    let content = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(w),
                height: length(ROW * count as f32),
            },
            flex_direction: taffy::FlexDirection::Column,
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(view, content);
    let mut rows = Vec::new();
    for i in 0..count {
        let row = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(w),
                    height: length(ROW),
                },
                flex_shrink: 0.0,
                ..Default::default()
            },
            PaintProperties::new(shade(i), 0.0, 1.0),
        );
        tree.add_child(content, row);
        rows.push(row);
        if !labels {
            continue;
        }
        for k in 0..extra {
            let t = tree.insert(
                label(&format!(
                    "extra label {k} of row number {i}, with some more words"
                )),
                placed(8.0, 2.0 + (k as f32 % 3.0) * 8.0, 190.0, 12.0),
                PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
            );
            tree.add_child(row, t);
        }
        let text = tree.insert(
            label(&format!("row number {i}")),
            placed(8.0, 6.0, 150.0, 18.0),
            PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
        );
        tree.add_child(row, text);
    }
    layout(&mut tree, root);
    World {
        tree,
        root,
        view,
        rows,
    }
}

fn scroll_to(world: &mut World, offset: f64) {
    if let NodeKind::ScrollView(state) = &mut world.tree.get_mut(world.view).unwrap().kind {
        state.scroll.current = offset;
    }
    layout(&mut world.tree, world.root);
}

fn area(damage: &Damage) -> f64 {
    match damage {
        Damage::None => 0.0,
        Damage::Full => f64::INFINITY,
        Damage::Rects(rects) => rects.iter().map(peniko::kurbo::Rect::area).sum(),
    }
}

/// Fails with where the first differing pixel is, not a dump of every byte.
fn assert_same(got: &[u8], want: &[u8], width: u32, context: &str) {
    assert_within(got, want, width, 0, context);
}

/// `assert_same`, letting each channel differ by up to `tolerance`. Text is
/// the one thing a copy cannot reproduce to the last bit: the rasteriser
/// computes the edge pixels of a glyph from where it is in the window, so the
/// same glyph drawn a few pixels along differs by one level in a pixel or two.
/// Everything else is exact.
fn assert_within(got: &[u8], want: &[u8], width: u32, tolerance: u8, context: &str) {
    if got == want {
        return;
    }
    let stride = (width * 4).next_multiple_of(256) as usize;
    let mut count = 0;
    let mut first = None;
    for (i, (a, b)) in got.chunks(4).zip(want.chunks(4)).enumerate() {
        if a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > tolerance) {
            count += 1;
            first.get_or_insert((i, a.to_vec(), b.to_vec()));
        }
    }
    let (i, a, b) = first.unwrap_or((0, vec![], vec![]));
    let row_pixels = (stride / 4) as u32;
    let (x, y) = (i as u32 % row_pixels, i as u32 / row_pixels);
    if count == 0 {
        return;
    }
    panic!("{context}: {count} pixels differ; first at ({x}, {y}): drawn {a:?}, full draw {b:?}");
}

/// What a fresh renderer draws for `tree` in full.
fn fresh(gpu: &Gpu, scale: f64, tree: &Tree, root: NodeId) -> Vec<u8> {
    let mut window = Window::new(gpu, scale);
    window.frame(gpu, tree, root, false);
    window.pixels(gpu)
}

/// Draws each offset in turn the way the app does and checks every frame
/// against a full draw. Returns the damage of each frame after the first.
fn run_offsets(scale: f64, offsets: &[f64], labels: bool) -> Vec<Damage> {
    let gpu = Gpu::new();
    let tolerance = u8::from(labels);
    let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), labels);
    let mut window = Window::new(&gpu, scale);
    window.frame(&gpu, &world.tree, world.root, true);
    let mut damages = Vec::new();
    for (step, &offset) in offsets.iter().enumerate() {
        scroll_to(&mut world, offset);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert_within(
            &window.pixels(&gpu),
            &fresh(&gpu, scale, &world.tree, world.root),
            window.width.into(),
            tolerance,
            &format!(
                "frame {step} (offset {offset}, scale {scale}, labels {labels}, damage {damage:?})"
            ),
        );
        damages.push(damage);
    }
    damages
}

fn view_area(scale: f64) -> f64 {
    f64::from(VIEW.2) * f64::from(VIEW.3) * scale * scale
}

#[test]
fn whole_pixel_scrolls_move_the_kept_pixels_and_match_a_full_draw() {
    let steps = [5.0, 18.0, 31.0, 36.0, 60.0, 59.0, 120.0, 118.0, 0.0];
    run_offsets(1.0, &steps, true);
    let damages = run_offsets(1.0, &steps, false);
    // The large jumps redraw more, but a small scroll is a strip and a
    // scrollbar, far less than the view.
    for damage in &damages[..4] {
        assert!(
            area(damage) < view_area(1.0) * 0.5,
            "a small scroll should redraw a strip, got {damage:?}"
        );
    }
}

#[test]
fn a_whole_scale_scroll_matches_a_full_draw() {
    let steps = [3.0, 11.0, 20.0, 35.0, 34.0];
    run_offsets(2.0, &steps, true);
    let damages = run_offsets(2.0, &steps, false);
    for damage in &damages[..3] {
        assert!(area(damage) < view_area(2.0) * 0.5, "{damage:?}");
    }
}

#[test]
fn a_fractional_scale_scroll_matches_a_full_draw_however_it_is_drawn() {
    // 1.5x: rows land on rounded device pixels, and not every step moves
    // them all alike. Whatever the tracker decides, the pixels must match.
    let steps = [1.0, 2.0, 2.4, 7.0, 9.3, 20.0, 21.0, 33.0];
    run_offsets(1.5, &steps, true);
    run_offsets(1.5, &steps, false);
}

#[test]
fn a_fractional_offset_matches_a_full_draw() {
    let steps = [0.4, 1.7, 3.2, 10.5, 10.6, 25.1];
    run_offsets(1.0, &steps, true);
    run_offsets(1.0, &steps, false);
}

#[test]
fn many_random_scroll_steps_match_a_full_draw() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    for scale in [1.0, 2.0] {
        let mut offsets = Vec::new();
        let mut at = 0.0f64;
        for _ in 0..40 {
            at = (at + (next() % 90) as f64 - 30.0).clamp(0.0, 1000.0);
            offsets.push(at);
        }
        run_offsets(scale, &offsets, true);
        run_offsets(scale, &offsets, false);
    }
}

/// A scroller whose background shows what is behind it, or whose shape is not
/// a plain box, must not be copied: the pixels still match, with wide damage.
fn expects_wide_damage(make: impl Fn(&mut World)) {
    let gpu = Gpu::new();
    let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), true);
    make(&mut world);
    layout(&mut world.tree, world.root);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &world.tree, world.root, true);
    for offset in [4.0, 9.0, 21.0] {
        scroll_to(&mut world, offset);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert!(
            area(&damage) >= view_area(1.0) * 0.9,
            "a scroller that cannot be copied redraws its view, got {damage:?}"
        );
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, 1.0, &world.tree, world.root),
            window.width.into(),
            &format!("offset {offset}"),
        );
    }
}

#[test]
fn a_see_through_background_is_redrawn() {
    expects_wide_damage(|w| {
        w.tree.get_mut(w.view).unwrap().paint.background.current =
            Color::from_rgba8(20, 20, 24, 200);
    });
}

#[test]
fn rounded_corners_are_redrawn() {
    expects_wide_damage(|w| {
        w.tree.get_mut(w.view).unwrap().paint.corner_radius.current = 12.0;
    });
}

#[test]
fn a_border_is_redrawn() {
    expects_wide_damage(|w| {
        let paint = &mut w.tree.get_mut(w.view).unwrap().paint;
        paint.border_width.current = 3.0;
        paint.border_color.current = Color::from_rgba8(255, 255, 255, 255);
    });
}

#[test]
fn a_node_painted_over_the_view_is_redrawn() {
    expects_wide_damage(|w| {
        let badge = w.tree.insert(
            NodeKind::Rect,
            placed(60.0, 50.0, 80.0, 40.0),
            PaintProperties::new(Color::from_rgba8(255, 255, 0, 255), 0.0, 1.0),
        );
        w.tree.add_child(w.root, badge);
    });
}

#[test]
fn a_faded_ancestor_is_redrawn() {
    expects_wide_damage(|w| {
        w.tree.get_mut(w.root).unwrap().paint.opacity.current = 0.8;
    });
}

#[test]
fn a_sticky_row_stays_put_while_the_rest_moves() {
    let gpu = Gpu::new();
    let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), true);
    world.tree.set_sticky(world.rows[0], Some(0.0));
    layout(&mut world.tree, world.root);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &world.tree, world.root, true);
    for offset in [10.0, 25.0, 31.0, 80.0] {
        scroll_to(&mut world, offset);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert_within(
            &window.pixels(&gpu),
            &fresh(&gpu, 1.0, &world.tree, world.root),
            window.width.into(),
            1,
            &format!("offset {offset}, damage {damage:?}"),
        );
    }
}

#[test]
fn a_row_changing_while_scrolling_is_redrawn_where_it_is() {
    let gpu = Gpu::new();
    let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), true);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &world.tree, world.root, true);
    for (step, offset) in [12.0, 24.0, 41.0, 70.0].into_iter().enumerate() {
        // Some row changes colour, and one hides, in the frame that scrolls.
        let row = world.rows[1 + step];
        world.tree.get_mut(row).unwrap().paint.background.current =
            Color::from_rgba8(255, (step * 60) as u8, 0, 255);
        let hide = world.rows[7 + step];
        let node = world.tree.get_mut(hide).unwrap();
        node.visible = !node.visible;
        scroll_to(&mut world, offset);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert_within(
            &window.pixels(&gpu),
            &fresh(&gpu, 1.0, &world.tree, world.root),
            window.width.into(),
            1,
            &format!("step {step}, damage {damage:?}"),
        );
    }
}

#[test]
fn a_virtual_list_scrolled_matches_a_full_draw() {
    let gpu = Gpu::new();
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(Color::from_rgba8(25, 25, 30, 255), 0.0, 1.0),
    );
    let (x, y, w, h) = VIEW;
    let list = tree.insert(
        NodeKind::VirtualList(VirtualListState::new(
            200,
            ItemExtent::Fixed(f64::from(ROW)),
        )),
        placed(x, y, w, h),
        PaintProperties::new(Color::from_rgba8(20, 20, 24, 255), 0.0, 1.0),
    );
    tree.add_child(root, list);
    let make = |i: usize| {
        (
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(w),
                    height: length(ROW),
                },
                ..Default::default()
            },
            PaintProperties::new(shade(i), 0.0, 1.0),
        )
    };
    tree.set_virtual_list_window(list, 0..6, make);
    layout(&mut tree, root);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &tree, root, true);
    let mut first = 0usize;
    for (step, offset) in [7.0, 19.0, 33.0, 52.0, 88.0, 91.0].into_iter().enumerate() {
        first = (offset / f64::from(ROW)) as usize;
        if let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind {
            state.scroll_offset.current = offset;
        }
        tree.set_virtual_list_window(list, first..first + 6, make);
        layout(&mut tree, root);
        let damage = window.frame(&gpu, &tree, root, true);
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, 1.0, &tree, root),
            window.width.into(),
            &format!("step {step} (offset {offset}, rows from {first}), damage {damage:?}"),
        );
    }
    let _ = first;
}

#[test]
fn a_visible_row_hidden_or_removed_while_scrolling_leaves_no_ghost() {
    for labels in [false, true] {
        let gpu = Gpu::new();
        let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), labels);
        let mut window = Window::new(&gpu, 1.0);
        window.frame(&gpu, &world.tree, world.root, true);
        let compare = |window: &Window, world: &World, step: &str, damage: &Damage| {
            assert_within(
                &window.pixels(&gpu),
                &fresh(&gpu, 1.0, &world.tree, world.root),
                window.width.into(),
                u8::from(labels),
                &format!("{step} (labels {labels}), damage {damage:?}"),
            );
        };
        // Row 2 is on screen; it hides in the frame that scrolls.
        world.tree.get_mut(world.rows[2]).unwrap().visible = false;
        scroll_to(&mut world, 12.0);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        compare(&window, &world, "hide", &damage);
        // And it comes back, as another is removed outright.
        world.tree.get_mut(world.rows[2]).unwrap().visible = true;
        world.tree.remove(world.rows[3]);
        scroll_to(&mut world, 25.0);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        compare(&window, &world, "show and remove", &damage);
        // A row moves sideways, off the common path, while scrolling.
        world
            .tree
            .get_mut(world.rows[1])
            .unwrap()
            .paint
            .node_transform
            .translate_x
            .current = 9.0;
        scroll_to(&mut world, 31.0);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        compare(&window, &world, "row moved", &damage);
    }
}

/// Scrolls mixed at random with the edits that make a block copy wrong if the
/// tracker misjudges it: rows changing, hiding, moving sideways, going sticky,
/// the view's own background changing, a badge appearing over it.
#[test]
fn random_edits_between_scrolls_match_a_full_draw() {
    for (seed, scale, labels) in [
        (1u64, 1.0, false),
        (2, 1.0, true),
        (3, 2.0, false),
        (4, 2.0, true),
        (5, 1.0, false),
        (6, 1.0, true),
        (7, 2.0, true),
        (8, 1.0, false),
        (9, 1.0, true),
        (10, 2.0, false),
        (11, 1.0, true),
        (12, 1.0, true),
    ] {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move |n: u64| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D) % n
        };
        let gpu = Gpu::new();
        let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), labels);
        let mut window = Window::new(&gpu, scale);
        window.frame(&gpu, &world.tree, world.root, true);
        let mut offset = 0.0f64;
        let mut badge: Option<NodeId> = None;
        for step in 0..80 {
            let mut what = Vec::new();
            for _ in 0..next(3) {
                let row = world.rows[next(world.rows.len() as u64) as usize];
                if world.tree.get(row).is_none() {
                    continue;
                }
                match next(8) {
                    0 => {
                        world.tree.get_mut(row).unwrap().paint.background.current =
                            Color::from_rgba8(next(256) as u8, next(256) as u8, 0, 255);
                        what.push("colour");
                    }
                    1 => {
                        let node = world.tree.get_mut(row).unwrap();
                        node.visible = !node.visible;
                        what.push("visible");
                    }
                    2 => {
                        world
                            .tree
                            .get_mut(row)
                            .unwrap()
                            .paint
                            .node_transform
                            .translate_x
                            .current = next(12) as f64;
                        what.push("sideways");
                    }
                    3 => {
                        world.tree.set_sticky(row, (next(2) == 0).then_some(0.0));
                        what.push("sticky");
                    }
                    4 => {
                        world
                            .tree
                            .get_mut(world.view)
                            .unwrap()
                            .paint
                            .background
                            .current = Color::from_rgba8(next(60) as u8, 20, 24, 255);
                        what.push("view background");
                    }
                    5 => match badge.take() {
                        Some(b) => {
                            world.tree.remove(b);
                            what.push("badge off");
                        }
                        None => {
                            let b = world.tree.insert(
                                NodeKind::Rect,
                                placed(40.0 + next(100) as f32, 30.0 + next(80) as f32, 40.0, 30.0),
                                PaintProperties::new(Color::from_rgba8(255, 255, 0, 255), 0.0, 1.0),
                            );
                            world.tree.add_child(world.root, b);
                            badge = Some(b);
                            what.push("badge on");
                        }
                    },
                    6 => {
                        let opacity = [1.0, 0.5][next(2) as usize];
                        world.tree.get_mut(row).unwrap().paint.opacity.current = opacity;
                        what.push("row opacity");
                    }
                    _ => {}
                }
            }
            let scrolls = next(4) != 0;
            if scrolls {
                offset = (offset + next(70) as f64 - 20.0).clamp(0.0, 900.0);
                what.push("scroll");
            }
            scroll_to(&mut world, offset);
            let damage = window.frame(&gpu, &world.tree, world.root, true);
            assert_within(
                &window.pixels(&gpu),
                &fresh(&gpu, scale, &world.tree, world.root),
                window.width.into(),
                u8::from(labels),
                &format!(
                    "seed {seed}, step {step}, {what:?}, offset {offset}, shift {:?}, damage {damage:?}",
                    window.renderer.last_shift()
                ),
            );
        }
    }
}

/// What a scrolled frame costs with and without the shift, on whatever
/// adapter the tests get (often a software one, where redrawing is dear).
/// `cargo test -p engine-render --release --test scroll_blit -- --ignored --nocapture`
#[test]
#[ignore = "timing, not correctness"]
fn scrolled_frame_cost() {
    let gpu = Gpu::new();
    for extra in [0usize, 6, 20] {
        for scale in [2.0, 4.0] {
            for blit in [true, false] {
                let mut world =
                    scroll_world_with(400, Color::from_rgba8(20, 20, 24, 255), true, extra);
                let mut window = Window::new(&gpu, scale);
                window.frame(&gpu, &world.tree, world.root, true);
                let (mut shifted, mut total) = (0, std::time::Duration::ZERO);
                let frames = 60u32;
                for i in 0..frames {
                    scroll_to(&mut world, f64::from(i) * 7.0 + 7.0);
                    let start = std::time::Instant::now();
                    // Off: a frame that asks for no partial redraw is a full one.
                    window.frame(&gpu, &world.tree, world.root, blit);
                    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                    total += start.elapsed();
                    if window.renderer.last_shift().is_some() {
                        shifted += 1;
                    }
                }
                println!(
                    "{extra:>2} extra texts a row, scale {scale}, {}: {:?} a frame ({shifted}/{frames} shifted)",
                    if blit {
                        "with the shift "
                    } else {
                        "full redraw   "
                    },
                    total / frames
                );
            }
        }
    }
}

// 0.5.4 (#149): a partial redraw's paint walk skips every subtree whose
// extent misses the damage. The extent has to cover what the subtree paints,
// including children that overflow their parent, or a change out there would
// never be drawn.

fn boxed(tree: &mut Tree, parent: NodeId, x: f32, y: f32, w: f32, h: f32, color: Color) -> NodeId {
    let id = tree.insert(
        NodeKind::Rect,
        placed(x, y, w, h),
        PaintProperties::new(color, 0.0, 1.0),
    );
    tree.add_child(parent, id);
    id
}

#[test]
fn a_child_far_outside_its_parent_is_still_redrawn() {
    let gpu = Gpu::new();
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(Color::from_rgba8(25, 25, 30, 255), 0.0, 1.0),
    );
    let parent = boxed(&mut tree, root, 10.0, 10.0, 30.0, 30.0, shade(1));
    let child = boxed(&mut tree, parent, 150.0, 90.0, 40.0, 40.0, shade(2));
    let grandchild = boxed(&mut tree, child, -60.0, 30.0, 20.0, 20.0, shade(3));
    layout(&mut tree, root);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &tree, root, true);
    for (step, id) in [child, grandchild, child, grandchild]
        .into_iter()
        .enumerate()
    {
        tree.get_mut(id).unwrap().paint.background.current =
            Color::from_rgba8(255, (step * 50) as u8, 0, 255);
        layout(&mut tree, root);
        let damage = window.frame(&gpu, &tree, root, true);
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, 1.0, &tree, root),
            window.width.into(),
            &format!("step {step}, damage {damage:?}"),
        );
        assert!(area(&damage) < 5000.0, "only the changed box: {damage:?}");
    }
    // And moving the grandchild further out, past everything.
    tree.get_mut(grandchild)
        .unwrap()
        .paint
        .node_transform
        .translate_x
        .current = 200.0;
    layout(&mut tree, root);
    let damage = window.frame(&gpu, &tree, root, true);
    assert_same(
        &window.pixels(&gpu),
        &fresh(&gpu, 1.0, &tree, root),
        window.width.into(),
        &format!("moved out, damage {damage:?}"),
    );
}

#[test]
fn random_tree_edits_match_a_full_draw() {
    for seed in 1..=16u64 {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move |n: u64| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D) % n
        };
        let gpu = Gpu::new();
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            placed(0.0, 0.0, W, H),
            PaintProperties::new(Color::from_rgba8(25, 25, 30, 255), 0.0, 1.0),
        );
        let mut nodes = vec![root];
        for _ in 0..40 {
            let parent = nodes[next(nodes.len() as u64) as usize];
            // Some far outside any parent: nothing clips them.
            let id = boxed(
                &mut tree,
                parent,
                next(260) as f32 - 40.0,
                next(180) as f32 - 40.0,
                10.0 + next(60) as f32,
                10.0 + next(50) as f32,
                shade(next(40) as usize),
            );
            nodes.push(id);
        }
        layout(&mut tree, root);
        let mut window = Window::new(&gpu, 1.0);
        window.frame(&gpu, &tree, root, true);
        for step in 0..60 {
            let mut what = Vec::new();
            for _ in 0..1 + next(3) {
                let id = nodes[1 + next(nodes.len() as u64 - 1) as usize];
                if tree.get(id).is_none() {
                    continue;
                }
                match next(9) {
                    0 | 1 => {
                        tree.get_mut(id).unwrap().paint.background.current = Color::from_rgba8(
                            next(256) as u8,
                            next(256) as u8,
                            next(256) as u8,
                            255,
                        );
                        what.push("colour");
                    }
                    2 => {
                        let t = &mut tree.get_mut(id).unwrap().paint.node_transform;
                        t.translate_x.current = next(120) as f64 - 60.0;
                        t.translate_y.current = next(120) as f64 - 60.0;
                        what.push("move");
                    }
                    3 => {
                        let node = tree.get_mut(id).unwrap();
                        node.visible = !node.visible;
                        what.push("visible");
                    }
                    4 => {
                        tree.get_mut(id).unwrap().paint.opacity.current =
                            [1.0, 0.5, 0.0][next(3) as usize];
                        what.push("opacity");
                    }
                    5 => {
                        let node = tree.get_mut(id).unwrap();
                        node.paint.clip_children = !node.paint.clip_children;
                        what.push("clip");
                    }
                    6 => {
                        let node = tree.get_mut(id).unwrap();
                        node.layout_style.size.width = length(10.0 + next(70) as f32);
                        node.layout_style.size.height = length(10.0 + next(60) as f32);
                        what.push("resize");
                    }
                    7 => {
                        let node = tree.get_mut(id).unwrap();
                        node.paint.corner_radius.current = next(14) as f64;
                        node.paint.border_width.current = next(4) as f64;
                        what.push("shape");
                    }
                    _ => {
                        tree.get_mut(id).unwrap().z_index = next(3) as i32 - 1;
                        what.push("z");
                    }
                }
            }
            layout(&mut tree, root);
            let damage = window.frame(&gpu, &tree, root, true);
            assert_same(
                &window.pixels(&gpu),
                &fresh(&gpu, 1.0, &tree, root),
                window.width.into(),
                &format!("seed {seed}, step {step}, {what:?}, damage {damage:?}"),
            );
        }
    }
}

// 0.5.4 (#135): the paint walk can time each node it reaches.

#[test]
fn profiling_attributes_the_scene_time_by_kind_and_changes_no_pixel() {
    let gpu = Gpu::new();
    let world = scroll_world(12, Color::from_rgba8(20, 20, 24, 255), true);
    let mut plain = Window::new(&gpu, 1.0);
    plain.frame(&gpu, &world.tree, world.root, true);
    let mut timed = Window::new(&gpu, 1.0);
    timed.renderer.set_profiling(true);
    timed.frame(&gpu, &world.tree, world.root, true);
    assert_same(
        &timed.pixels(&gpu),
        &plain.pixels(&gpu),
        plain.width.into(),
        "profiling is only a clock",
    );
    let profile = timed.renderer.take_profile().expect("profiled");
    let kinds: Vec<&str> = profile.by_kind.iter().map(|(k, _)| *k).collect();
    for kind in ["container", "scroll_view", "box", "text"] {
        assert!(kinds.contains(&kind), "{kind} in {kinds:?}");
    }
    let texts = profile
        .by_kind
        .iter()
        .find(|(k, _)| *k == "text")
        .unwrap()
        .1;
    assert!(texts.reached >= 4, "the visible rows' labels: {texts:?}");
    assert!(profile.time > std::time::Duration::ZERO);
    assert_eq!(
        profile
            .by_kind
            .iter()
            .map(|(_, c)| c.reached)
            .sum::<usize>(),
        profile.reached
    );
    assert!(profile.slowest.len() <= engine_render::PROFILE_SLOWEST);
    assert!(profile.slowest.windows(2).all(|w| w[0].time >= w[1].time));
    // Off again, a frame leaves no profile.
    timed.renderer.set_profiling(false);
    timed.frame(&gpu, &world.tree, world.root, true);
    assert!(timed.renderer.take_profile().is_none());
}

// 0.5.4 (#127): text drawn from the glyph cache.

/// A fresh renderer's full draw of `tree`, with the glyph cache on or off.
fn fresh_cached(gpu: &Gpu, scale: f64, tree: &Tree, root: NodeId, cache: bool) -> Vec<u8> {
    let mut window = Window::new(gpu, scale);
    window.renderer.set_glyph_cache(cache);
    window.frame(gpu, tree, root, false);
    window.pixels(gpu)
}

/// Mean of each `block` x `block` square of the first channel's brightness
/// (padding ignored): text blurred enough to compare where it is and not how
/// its edges fell.
fn blocks(pixels: &[u8], width: u32, height: u32, block: usize) -> Vec<f64> {
    let stride = (width * 4).next_multiple_of(256) as usize;
    let (w, h) = (width as usize / block, height as usize / block);
    let mut out = Vec::with_capacity(w * h);
    for by in 0..h {
        for bx in 0..w {
            let mut sum = 0.0;
            for y in 0..block {
                for x in 0..block {
                    let i = (by * block + y) * stride + (bx * block + x) * 4;
                    sum +=
                        f64::from(pixels[i]) + f64::from(pixels[i + 1]) + f64::from(pixels[i + 2]);
                }
            }
            out.push(sum / (3.0 * (block * block) as f64));
        }
    }
    out
}

#[test]
fn the_glyph_cache_draws_the_same_text_in_the_same_place_if_not_to_the_last_bit() {
    let gpu = Gpu::new();
    for scale in [1.0, 1.5, 2.0] {
        let world = scroll_world_with(10, Color::from_rgba8(20, 20, 24, 255), true, 2);
        let outline = fresh_cached(&gpu, scale, &world.tree, world.root, false);
        let cached = fresh_cached(&gpu, scale, &world.tree, world.root, true);
        let (w, h) = ((260.0 * scale) as u32, (180.0 * scale) as u32);
        // Every channel of every pixel stays within what an antialiased edge can move.
        let stride = (w * 4).next_multiple_of(256) as usize;
        let mut worst = 0u8;
        let mut differing = 0usize;
        for y in 0..h as usize {
            for x in 0..w as usize * 4 {
                let d = outline[y * stride + x].abs_diff(cached[y * stride + x]);
                worst = worst.max(d);
                differing += usize::from(d > 0);
            }
        }
        assert!(
            worst <= 110,
            "scale {scale}: an edge moved by {worst} of 255"
        );
        assert!(
            differing > 0,
            "scale {scale}: the cache should draw glyphs differently"
        );
        // Blurred over 8x8 blocks the two are the same picture.
        let (a, b) = (blocks(&outline, w, h, 8), blocks(&cached, w, h, 8));
        let worst_block = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(
            worst_block < 14.0,
            "scale {scale}: text moved or went missing ({worst_block})"
        );
    }
}

#[test]
fn with_the_glyph_cache_on_partial_redraws_and_scrolls_still_match_a_full_draw() {
    let gpu = Gpu::new();
    for scale in [1.0, 2.0] {
        let mut world = scroll_world(40, Color::from_rgba8(20, 20, 24, 255), true);
        let mut window = Window::new(&gpu, scale);
        window.renderer.set_glyph_cache(true);
        window.renderer.set_scroll_blit_min_nodes(0);
        window.frame(&gpu, &world.tree, world.root, true);
        for (step, offset) in [4.0, 11.0, 30.0, 37.0, 90.0, 61.0].into_iter().enumerate() {
            world
                .tree
                .get_mut(world.rows[step + 1])
                .unwrap()
                .paint
                .background
                .current = Color::from_rgba8(200, (step * 40) as u8, 40, 255);
            scroll_to(&mut world, offset);
            let damage = window.frame(&gpu, &world.tree, world.root, true);
            // Glyph images are placed whole, so a copy or a redraw matches exactly.
            assert_within(
                &window.pixels(&gpu),
                &fresh_cached(&gpu, scale, &world.tree, world.root, true),
                window.width.into(),
                1,
                &format!("scale {scale}, step {step}, damage {damage:?}"),
            );
        }
    }
}

#[test]
fn switching_the_glyph_cache_redraws_the_whole_window() {
    let gpu = Gpu::new();
    let world = scroll_world(10, Color::from_rgba8(20, 20, 24, 255), true);
    let mut window = Window::new(&gpu, 1.0);
    window.frame(&gpu, &world.tree, world.root, true);
    assert_eq!(
        window.frame(&gpu, &world.tree, world.root, true),
        Damage::None
    );
    window.renderer.set_glyph_cache(true);
    assert_eq!(
        window.frame(&gpu, &world.tree, world.root, true),
        Damage::Full
    );
    // Setting it to what it already is changes nothing.
    window.renderer.set_glyph_cache(true);
    assert_eq!(
        window.frame(&gpu, &world.tree, world.root, true),
        Damage::None
    );
}

#[test]
fn many_sizes_and_glyphs_fill_the_cache_without_failing() {
    let gpu = Gpu::new();
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
    );
    for i in 0..60 {
        let size = 6.0 + 1.37 * i as f32;
        let label = tree.insert(
            NodeKind::Text(TextState {
                content: "The quick brown fox 0123456789 \u{e9}\u{f1}\u{fc}".into(),
                font_family: "Roboto".into(),
                font_weight: 400.0,
                font_size: size,
                align: Default::default(),
                line_height: None,
                options: Default::default(),
            }),
            placed(0.0, (i % 12) as f32 * 14.0, W, 14.0),
            PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
        );
        tree.add_child(root, label);
    }
    layout(&mut tree, root);
    let drawn = fresh_cached(&gpu, 1.0, &tree, root, true);
    let lit = drawn.chunks(4).filter(|p| p[0] > 128).count();
    assert!(lit > 500, "text was drawn: {lit} bright pixels");
}
