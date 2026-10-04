//! 0.5.4 (#129): gradients on more than a box's fill: canvas painter calls,
//! borders, a path's fill and stroke, and a text node's glyphs. Each is
//! resolved against the shape's own bounds.

mod support;

use engine_core::{
    Animated, CanvasState, DrawCommand, Gradient, GradientShape, GradientStop, NodeId, NodeKind,
    PaintProperties, PathData, PathState, TextState, Tree,
};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use peniko::kurbo::BezPath;
use support::{Frame, SIZE, WHITE, placed};

const BG: Color = Color::from_rgba8(0, 0, 40, 255);

fn stop(offset: f32, r: u8) -> GradientStop {
    GradientStop {
        offset,
        color: Color::from_rgba8(r, 0, 0, 255),
    }
}

/// Black on the left to red on the right.
fn ramp() -> Gradient {
    Gradient::new(
        GradientShape::Linear { angle_deg: 90.0 },
        vec![stop(0.0, 0), stop(1.0, 255)],
    )
    .unwrap()
}

fn root(tree: &mut Tree) -> NodeId {
    tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, f32::from(SIZE), f32::from(SIZE)),
        PaintProperties::new(BG, 0.0, 1.0),
    )
}

#[allow(clippy::too_many_arguments)]
fn child(
    tree: &mut Tree,
    parent: NodeId,
    kind: NodeKind,
    paint: PaintProperties,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> NodeId {
    let id = tree.insert(kind, placed(x, y, w, h), paint);
    tree.add_child(parent, id);
    id
}

fn frame(tree: &mut Tree, root: NodeId) -> Frame {
    support::layout(tree, root);
    pollster::block_on(Frame::of(tree, root))
}

fn canvas(commands: Vec<DrawCommand>) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    let mut state = CanvasState::new();
    state.commands = commands;
    child(
        &mut tree,
        r,
        NodeKind::Canvas(state),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        0.0,
        0.0,
        100.0,
        100.0,
    );
    (tree, r)
}

#[test]
fn a_canvas_rect_gradient_spans_the_rect_not_the_canvas() {
    let (mut tree, r) = canvas(vec![DrawCommand::FillRect {
        x: 20.0,
        y: 20.0,
        width: 60.0,
        height: 60.0,
        color: Color::from_rgba8(0, 255, 0, 255),
        gradient: Some(ramp()),
    }]);
    let f = frame(&mut tree, r);
    let (left, mid, right) = (f.at(22, 50), f.at(50, 50), f.at(77, 50));
    assert!(left[0] < 25, "dark at the rect's left edge: {left:?}");
    assert!(mid[0].abs_diff(128) < 16, "half in the middle: {mid:?}");
    assert!(right[0] > 230, "red at its right edge: {right:?}");
    assert!(mid[1] < 10, "not the flat green: {mid:?}");
    assert_eq!(f.at(10, 50)[2], 40, "outside the rect is the background");
}

#[test]
fn a_canvas_circle_and_stroke_gradient_follow_their_own_bounds() {
    let mut path = BezPath::new();
    path.move_to((10.0, 90.0));
    path.line_to((90.0, 90.0));
    let (mut tree, r) = canvas(vec![
        DrawCommand::FillCircle {
            cx: 50.0,
            cy: 40.0,
            radius: 30.0,
            color: Color::from_rgba8(0, 255, 0, 255),
            gradient: Some(ramp()),
        },
        DrawCommand::StrokePath {
            path,
            color: Color::from_rgba8(0, 255, 0, 255),
            width: 6.0,
            gradient: Some(ramp()),
        },
    ]);
    let f = frame(&mut tree, r);
    let (left, right) = (f.at(24, 40), f.at(76, 40));
    assert!(
        left[0] < 50 && right[0] > 200,
        "circle ramps: {left:?} {right:?}"
    );
    let (left, right) = (f.at(14, 90), f.at(86, 90));
    assert!(
        left[0] < 50 && right[0] > 200,
        "stroke ramps: {left:?} {right:?}"
    );
}

#[test]
fn a_gradient_border_ramps_along_the_box() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
    paint.border_width = Animated::new(10.0);
    paint.border_color = Animated::new(Color::from_rgba8(0, 255, 0, 255));
    paint.border_gradient = Some(Box::new(ramp()));
    child(&mut tree, r, NodeKind::Rect, paint, 0.0, 0.0, 100.0, 100.0);
    let f = frame(&mut tree, r);
    let (left, mid, right) = (f.at(8, 3), f.at(50, 3), f.at(92, 3));
    assert!(left[0] < 30, "{left:?}");
    assert!(mid[0].abs_diff(128) < 20, "{mid:?}");
    assert!(right[0] > 225, "{right:?}");
    assert!(mid[1] < 10, "not the flat green: {mid:?}");
    // The fill inside is untouched.
    assert_eq!(f.at(50, 50)[2], 40);
}

#[test]
fn a_rounded_gradient_border_ramps_too() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 12.0, 1.0);
    paint.border_width = Animated::new(6.0);
    paint.border_gradient = Some(Box::new(ramp()));
    child(&mut tree, r, NodeKind::Rect, paint, 0.0, 0.0, 100.0, 100.0);
    let f = frame(&mut tree, r);
    let (left, right) = (f.at(2, 50), f.at(97, 50));
    assert!(left[0] < 40 && right[0] > 215, "{left:?} {right:?}");
}

#[test]
fn a_path_fill_and_stroke_take_gradients() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 255, 0, 255), 0.0, 1.0);
    paint.gradient = Some(Box::new(Animated::new(ramp())));
    paint.border_width = Animated::new(4.0);
    paint.border_gradient = Some(Box::new(ramp()));
    child(
        &mut tree,
        r,
        NodeKind::Path(PathState::new(
            PathData::from_svg("M10,10 H90 V90 H10 Z").unwrap(),
        )),
        paint,
        0.0,
        0.0,
        100.0,
        100.0,
    );
    let f = frame(&mut tree, r);
    let (left, right) = (f.at(20, 50), f.at(80, 50));
    assert!(
        left[0] < right[0] && right[0] > 170 && left[0] < 80,
        "fill: {left:?} {right:?}"
    );
    assert!(f.at(20, 50)[1] < 10, "not the flat green");
}

fn text_node(tree: &mut Tree, parent: NodeId, gradient: Option<Gradient>) -> NodeId {
    let mut paint = PaintProperties::new(WHITE, 0.0, 1.0);
    paint.gradient = gradient.map(|g| Box::new(Animated::new(g)));
    child(
        tree,
        parent,
        NodeKind::Text(TextState {
            content: "MMMMMMMM".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 700.0,
            font_size: 20.0,
            align: engine_core::TextAlign::Start,
            line_height: None,
            options: engine_core::TextOptions::default(),
        }),
        paint,
        0.0,
        20.0,
        100.0,
        50.0,
    )
}

/// The brightest red in the columns `from..to`.
fn peak_red(f: &Frame, from: u32, to: u32) -> u8 {
    (from..to)
        .flat_map(|x| (20..70).map(move |y| (x, y)))
        .map(|(x, y)| f.at(x, y)[0])
        .max()
        .unwrap()
}

#[test]
fn a_text_gradient_colours_the_glyphs_across_the_node() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    text_node(&mut tree, r, Some(ramp()));
    let f = frame(&mut tree, r);
    let (left, right) = (peak_red(&f, 0, 25), peak_red(&f, 70, 100));
    assert!(left < 120, "glyphs at the left are dark: {left}");
    assert!(right > 150, "glyphs at the right are red: {right}");
    // And not white anywhere: the gradient replaced the flat colour.
    assert!(
        (0..100).all(|x| (20..70).all(|y| f.at(x, y)[1] < 40)),
        "no white glyph pixel"
    );
}

#[test]
fn a_text_node_with_no_gradient_is_still_its_flat_colour() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    text_node(&mut tree, r, None);
    let f = frame(&mut tree, r);
    assert!(peak_red(&f, 0, 25) > 230);
}

#[test]
fn changing_a_gradient_damages_where_it_is() {
    let (mut tree, r) = canvas(vec![DrawCommand::FillRect {
        x: 20.0,
        y: 20.0,
        width: 40.0,
        height: 40.0,
        color: WHITE,
        gradient: Some(ramp()),
    }]);
    support::layout(&mut tree, r);
    let mut text = TextRenderer::new();
    let mut tracker = DamageTracker::new();
    let (w, h) = (SIZE, SIZE);
    assert_eq!(tracker.damage(&tree, r, w, h, &mut text), Damage::Full);
    assert_eq!(tracker.damage(&tree, r, w, h, &mut text), Damage::None);
    let canvas_id = tree.content_children(r)[0];
    let NodeKind::Canvas(state) = &mut tree.get_mut(canvas_id).unwrap().kind else {
        unreachable!()
    };
    let DrawCommand::FillRect { gradient, .. } = &mut state.commands[0] else {
        unreachable!()
    };
    *gradient = Some(
        Gradient::new(
            GradientShape::Linear { angle_deg: 0.0 },
            vec![stop(0.0, 0), stop(1.0, 255)],
        )
        .unwrap(),
    );
    assert_ne!(tracker.damage(&tree, r, w, h, &mut text), Damage::None);
    // And a border gradient is seen to change too.
    assert_eq!(tracker.damage(&tree, r, w, h, &mut text), Damage::None);
    tree.get_mut(canvas_id).unwrap().paint.border_gradient = Some(Box::new(ramp()));
    assert_ne!(tracker.damage(&tree, r, w, h, &mut text), Damage::None);
}

#[test]
fn text_with_a_coloured_span_still_paints_its_other_runs_with_the_gradient() {
    let mut tree = Tree::new();
    let r = root(&mut tree);
    let id = text_node(&mut tree, r, Some(ramp()));
    if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
        state.options.spans = vec![engine_core::TextSpan {
            start: 0,
            end: 2,
            color: Some(Color::from_rgba8(0, 255, 0, 255)),
            ..Default::default()
        }];
    }
    let f = frame(&mut tree, r);
    // The first glyphs are the span's green; the later ones are the ramp's red,
    // not a flat colour.
    let green = (0..25).any(|x| (20..70).any(|y| f.at(x, y)[1] > 200));
    assert!(green, "the span keeps its colour");
    assert!(
        peak_red(&f, 70, 100) > 150,
        "the unspanned end is on the ramp"
    );
    assert!(
        peak_red(&f, 30, 45) < peak_red(&f, 70, 100),
        "and it is a ramp"
    );
}
