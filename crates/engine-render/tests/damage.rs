//! 0.4.0 M4: `DamageTracker` reports a rect covering every kind of change,
//! and nothing for an unchanged frame. CPU only -- it compares trees, it
//! never renders. Each test makes one frame the tracker has seen, changes
//! one thing, and checks the damage covers the change and stays near it.

use std::time::{Duration, Instant};

use engine_core::{
    CanvasState, DrawCommand, ImageState, MotionCurve, NodeId, NodeKind, PaintProperties,
    ScrollViewState, TextFieldState, TextState, Tree,
};
use engine_render::{Damage, DamageTracker, MAX_RECTS, TextRenderer};
use peniko::Color;
use peniko::kurbo::Rect;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const W: u16 = 400;
const H: u16 = 300;
const GREY: Color = Color::from_rgba8(0x40, 0x40, 0x40, 0xFF);
const RED: Color = Color::from_rgba8(0xFF, 0, 0, 0xFF);
const BLUE: Color = Color::from_rgba8(0, 0, 0xFF, 0xFF);

struct Scene {
    tree: Tree,
    root: NodeId,
    tracker: DamageTracker,
    text: TextRenderer,
}

impl Scene {
    fn new() -> Self {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(f32::from(W)),
                    height: length(f32::from(H)),
                },
                ..Default::default()
            },
            PaintProperties::new(GREY, 0.0, 1.0),
        );
        Self {
            tree,
            root,
            tracker: DamageTracker::new(),
            text: TextRenderer::new(),
        }
    }

    fn add(&mut self, parent: NodeId, kind: NodeKind, x: f32, y: f32, w: f32, h: f32) -> NodeId {
        let style = Style {
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
        };
        let id = self
            .tree
            .insert(kind, style, PaintProperties::new(RED, 0.0, 1.0));
        self.tree.add_child(parent, id);
        id
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) -> NodeId {
        self.add(self.root, NodeKind::Rect, x, y, w, h)
    }

    /// Lays the tree out and asks for this frame's damage.
    fn frame(&mut self) -> Damage {
        self.tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(f32::from(W)),
                height: AvailableSpace::Definite(f32::from(H)),
            },
        );
        self.tracker
            .damage(&self.tree, self.root, W, H, &mut self.text)
    }

    /// A first frame, which is always `Full`, then one with no changes.
    fn settle(&mut self) {
        assert_eq!(
            self.frame(),
            Damage::Full,
            "a first frame redraws everything"
        );
        assert_eq!(
            self.frame(),
            Damage::None,
            "an unchanged frame damages nothing"
        );
    }
}

fn rects(damage: &Damage) -> &[Rect] {
    match damage {
        Damage::Rects(rects) => rects,
        other => panic!("expected damage rects, got {other:?}"),
    }
}

/// Some damage rect contains `area`.
fn covers(damage: &Damage, area: Rect) -> bool {
    rects(damage)
        .iter()
        .any(|r| r.contains_rect(area) || r.union(area) == *r)
}

/// Every damage rect lies within `area` grown by `slack` pixels.
fn within(damage: &Damage, area: Rect, slack: f64) -> bool {
    let limit = area.inflate(slack, slack);
    rects(damage).iter().all(|r| limit.union(*r) == limit)
}

#[test]
fn an_unchanged_frame_has_no_damage_and_a_first_frame_is_full() {
    let mut s = Scene::new();
    s.rect(10.0, 10.0, 20.0, 20.0);
    s.settle();
    assert_eq!(s.frame(), Damage::None);
}

#[test]
fn a_paint_change_damages_the_node_and_nothing_far_from_it() {
    let mut s = Scene::new();
    let node = s.rect(50.0, 60.0, 30.0, 20.0);
    s.rect(300.0, 200.0, 40.0, 40.0);
    s.settle();
    s.tree.get_mut(node).unwrap().paint.background.current = BLUE;
    let damage = s.frame();
    assert!(covers(&damage, Rect::new(50.0, 60.0, 80.0, 80.0)));
    assert!(within(&damage, Rect::new(50.0, 60.0, 80.0, 80.0), 4.0));
}

#[test]
fn a_move_damages_where_the_node_was_and_where_it_is() {
    let mut s = Scene::new();
    let node = s.rect(20.0, 20.0, 30.0, 30.0);
    s.settle();
    let mut style = s.tree.get(node).unwrap().layout_style.clone();
    style.inset.left = length(200.0);
    s.tree.set_layout_style(node, style);
    let damage = s.frame();
    assert!(
        covers(&damage, Rect::new(20.0, 20.0, 50.0, 50.0)),
        "{damage:?}"
    );
    assert!(
        covers(&damage, Rect::new(200.0, 20.0, 230.0, 50.0)),
        "{damage:?}"
    );
}

#[test]
fn a_running_animation_damages_every_frame_until_it_arrives() {
    let mut s = Scene::new();
    let node = s.rect(40.0, 40.0, 20.0, 20.0);
    s.settle();
    let start = Instant::now();
    s.tree.get_mut(node).unwrap().paint.background.animate_to(
        BLUE,
        Duration::from_millis(100),
        MotionCurve::Linear,
        start,
    );
    s.tree.tick_all(start + Duration::from_millis(50));
    assert!(covers(&s.frame(), Rect::new(40.0, 40.0, 60.0, 60.0)));
    s.tree.tick_all(start + Duration::from_millis(150));
    assert!(covers(&s.frame(), Rect::new(40.0, 40.0, 60.0, 60.0)));
    s.tree.tick_all(start + Duration::from_millis(200));
    assert_eq!(s.frame(), Damage::None, "arrived: nothing more to redraw");
}

#[test]
fn a_shadow_widens_the_damage_past_the_box() {
    let mut s = Scene::new();
    let node = s.rect(100.0, 100.0, 40.0, 40.0);
    s.tree.get_mut(node).unwrap().paint.shadows.current =
        engine_core::Shadows(vec![engine_core::Shadow {
            color: Color::from_rgba8(0, 0, 0, 0x80),
            offset_x: 0.0,
            offset_y: 20.0,
            blur: 10.0,
            spread: 4.0,
        }]);
    s.settle();
    s.tree.get_mut(node).unwrap().paint.background.current = BLUE;
    // The shadow reaches 20 + 4 + 2*10 pixels below the box.
    assert!(covers(&s.frame(), Rect::new(100.0, 100.0, 140.0, 184.0)));
}

#[test]
fn a_transform_damages_where_the_node_is_drawn() {
    let mut s = Scene::new();
    let node = s.rect(10.0, 10.0, 20.0, 20.0);
    s.settle();
    s.tree
        .get_mut(node)
        .unwrap()
        .paint
        .node_transform
        .translate_x
        .current = 150.0;
    let damage = s.frame();
    assert!(covers(&damage, Rect::new(10.0, 10.0, 30.0, 30.0)));
    assert!(covers(&damage, Rect::new(160.0, 10.0, 180.0, 30.0)));
}

#[test]
fn removing_or_hiding_a_node_damages_where_it_was() {
    let mut s = Scene::new();
    let gone = s.rect(10.0, 10.0, 20.0, 20.0);
    let hidden = s.rect(200.0, 100.0, 20.0, 20.0);
    s.settle();
    s.tree.detach(s.root, gone);
    s.tree.get_mut(hidden).unwrap().visible = false;
    let damage = s.frame();
    assert!(covers(&damage, Rect::new(10.0, 10.0, 30.0, 30.0)));
    assert!(covers(&damage, Rect::new(200.0, 100.0, 220.0, 120.0)));
}

#[test]
fn reordering_overlapping_siblings_damages_them() {
    let mut s = Scene::new();
    let below = s.rect(50.0, 50.0, 40.0, 40.0);
    s.rect(70.0, 70.0, 40.0, 40.0);
    s.settle();
    s.tree.get_mut(below).unwrap().z_index = 1; // now painted on top
    assert!(covers(&s.frame(), Rect::new(70.0, 70.0, 90.0, 90.0)));
}

#[test]
fn an_ancestors_opacity_damages_its_subtree() {
    let mut s = Scene::new();
    let parent = s.rect(20.0, 20.0, 200.0, 150.0);
    s.add(parent, NodeKind::Rect, 100.0, 100.0, 30.0, 30.0);
    s.settle();
    s.tree.get_mut(parent).unwrap().paint.opacity.current = 0.5;
    assert!(covers(&s.frame(), Rect::new(120.0, 120.0, 150.0, 150.0)));
}

#[test]
fn text_that_overflows_its_box_is_damaged_where_it_paints() {
    let mut s = Scene::new();
    let label = s.add(
        s.root,
        NodeKind::Text(TextState {
            content: "a long label that wraps onto several lines well below its box".into(),
            font_family: "Roboto".into(),
            font_weight: 400.0,
            font_size: 16.0,
            align: Default::default(),
            line_height: None,
            options: Default::default(),
        }),
        20.0,
        20.0,
        80.0,
        16.0,
    );
    s.settle();
    if let NodeKind::Text(state) = &mut s.tree.get_mut(label).unwrap().kind {
        state.content = "another long label that wraps onto several lines below its box".into();
    }
    // Several wrapped lines paint well below the 16px box.
    assert!(covers(&s.frame(), Rect::new(20.0, 20.0, 100.0, 60.0)));
}

#[test]
fn focusing_a_text_input_damages_it_for_the_caret() {
    let mut s = Scene::new();
    let field = s.add(
        s.root,
        NodeKind::TextField(TextFieldState::new("hello", "Roboto", 400.0, 14.0)),
        20.0,
        200.0,
        120.0,
        24.0,
    );
    s.settle();
    s.tree.set_focus_to(field);
    let damage = s.frame();
    assert!(covers(&damage, Rect::new(20.0, 200.0, 140.0, 224.0)));
    // A single-line input takes its whole row, and no more.
    assert!(within(
        &damage,
        Rect::new(0.0, 200.0, f64::from(W), 224.0),
        4.0
    ));
}

#[test]
fn a_new_image_frame_damages_the_image() {
    let frame = |byte: u8| peniko::ImageData {
        data: peniko::Blob::from(vec![byte; 4 * 4 * 4]),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 4,
        height: 4,
    };
    let mut s = Scene::new();
    let image = s.add(
        s.root,
        NodeKind::Image(ImageState::new(frame(0))),
        60.0,
        60.0,
        40.0,
        40.0,
    );
    s.settle();
    if let NodeKind::Image(state) = &mut s.tree.get_mut(image).unwrap().kind {
        state.image = frame(0); // the same bytes, but a new frame
    }
    assert!(covers(&s.frame(), Rect::new(60.0, 60.0, 100.0, 100.0)));
}

#[test]
fn a_canvas_redraw_damages_what_it_draws_past_its_box() {
    let mut s = Scene::new();
    let canvas = s.add(
        s.root,
        NodeKind::Canvas(CanvasState::new()),
        10.0,
        10.0,
        20.0,
        20.0,
    );
    s.settle();
    s.tree.set_canvas_content(
        canvas,
        vec![DrawCommand::FillRect {
            x: 50.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
            color: BLUE,
        }],
        None,
    );
    assert!(covers(&s.frame(), Rect::new(60.0, 60.0, 70.0, 70.0)));
}

#[test]
fn scrolling_damages_only_the_viewport() {
    let mut s = Scene::new();
    let view = s.add(
        s.root,
        NodeKind::ScrollView(ScrollViewState::new(false)),
        50.0,
        50.0,
        100.0,
        100.0,
    );
    let content = s.add(view, NodeKind::Rect, 0.0, 0.0, 100.0, 400.0);
    s.tree.get_mut(content).unwrap().layout_style.position = Position::Relative;
    s.settle();
    if let NodeKind::ScrollView(state) = &mut s.tree.get_mut(view).unwrap().kind {
        state.scroll.current = 40.0;
    }
    let damage = s.frame();
    assert!(
        covers(&damage, Rect::new(50.0, 50.0, 150.0, 150.0)),
        "{damage:?}"
    );
    assert!(
        within(&damage, Rect::new(50.0, 50.0, 150.0, 150.0), 4.0),
        "{damage:?}"
    );
}

#[test]
fn a_clipping_parent_limits_its_childs_damage() {
    let mut s = Scene::new();
    let clip = s.rect(0.0, 0.0, 100.0, 100.0);
    s.tree.get_mut(clip).unwrap().paint.clip_children = true;
    let child = s.add(clip, NodeKind::Rect, 50.0, 50.0, 300.0, 200.0);
    s.settle();
    s.tree.get_mut(child).unwrap().paint.background.current = BLUE;
    let damage = s.frame();
    assert!(covers(&damage, Rect::new(50.0, 50.0, 100.0, 100.0)));
    assert!(within(&damage, Rect::new(0.0, 0.0, 100.0, 100.0), 0.0));
}

#[test]
fn scattered_changes_merge_into_a_few_rects() {
    let mut s = Scene::new();
    let nodes: Vec<NodeId> = (0..10)
        .map(|i| s.rect(10.0 + 38.0 * i as f32, 10.0 + 25.0 * i as f32, 6.0, 6.0))
        .collect();
    s.settle();
    for node in &nodes {
        s.tree.get_mut(*node).unwrap().paint.background.current = BLUE;
    }
    let damage = s.frame();
    assert!(rects(&damage).len() <= MAX_RECTS, "{damage:?}");
    for i in 0..nodes.len() {
        let (x, y) = (10.0 + 38.0 * i as f64, 10.0 + 25.0 * i as f64);
        assert!(covers(&damage, Rect::new(x, y, x + 6.0, y + 6.0)));
    }
    let found = rects(&damage);
    for i in 0..found.len() {
        for j in i + 1..found.len() {
            assert!(!found[i].overlaps(found[j]), "merged rects don't overlap");
        }
    }
}

#[test]
fn a_large_change_redraws_everything() {
    let mut s = Scene::new();
    s.rect(10.0, 10.0, 20.0, 20.0);
    s.settle();
    s.tree.get_mut(s.root).unwrap().paint.background.current = BLUE;
    assert_eq!(s.frame(), Damage::Full);
}

#[test]
fn a_resize_or_reset_redraws_everything() {
    let mut s = Scene::new();
    s.rect(10.0, 10.0, 20.0, 20.0);
    s.settle();
    s.tracker.reset();
    assert_eq!(s.frame(), Damage::Full, "after reset");
    assert_eq!(s.frame(), Damage::None);
    let resized = s.tracker.damage(&s.tree, s.root, W + 1, H, &mut s.text);
    assert_eq!(resized, Damage::Full, "a new window size");
}

#[test]
fn hundreds_of_changes_across_the_window_redraw_it_all_quickly() {
    // A whole grid animating: the cap keeps the merge from going cubic.
    let mut s = Scene::new();
    let nodes: Vec<NodeId> = (0..600)
        .map(|i| {
            s.rect(
                4.0 + 16.0 * (i % 24) as f32,
                4.0 + 12.0 * (i / 24) as f32,
                6.0,
                6.0,
            )
        })
        .collect();
    s.settle();
    for node in &nodes {
        s.tree.get_mut(*node).unwrap().paint.background.current = BLUE;
    }
    let started = Instant::now();
    assert_eq!(s.frame(), Damage::Full);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "600 changes took {:?}",
        started.elapsed()
    );
}

#[test]
fn many_changes_in_one_corner_stay_a_partial_redraw() {
    let mut s = Scene::new();
    let nodes: Vec<NodeId> = (0..100)
        .map(|i| {
            s.rect(
                10.0 + 12.0 * (i % 10) as f32,
                10.0 + 9.0 * (i / 10) as f32,
                4.0,
                4.0,
            )
        })
        .collect();
    s.settle();
    for node in &nodes {
        s.tree.get_mut(*node).unwrap().paint.background.current = BLUE;
    }
    let damage = s.frame();
    assert_eq!(rects(&damage).len(), 1, "{damage:?}");
    assert!(covers(&damage, Rect::new(10.0, 10.0, 122.0, 95.0)));
    assert!(within(&damage, Rect::new(10.0, 10.0, 122.0, 95.0), 3.0));
}
