//! 0.5.1 (#45): every kind with a box paints its own background, border,
//! and rounded corners, as a box does. Before, `fill`, `stroke_color`,
//! `stroke_width`, and `corner_radius` were accepted on every kind but
//! painted only on a box (and a few of them on a text input, a terminal, and
//! a path): `set` succeeded, `get` read the value back, and nothing showed.
//! A path keeps `stroke_*` as its outline and has no box.

mod support;
use support::*;

use engine_core::{
    Animated, CanvasState, ImageState, ItemExtent, NodeId, NodeKind, PaintProperties,
    ScrollViewState, TerminalState, TextAlign, TextFieldState, TextState, Tree, VirtualListState,
};
use peniko::Color;

const FILL: Color = RED;
const BORDER: Color = GREEN;

fn image(color: [u8; 4]) -> NodeKind {
    let mut bytes = Vec::new();
    for _ in 0..4 {
        bytes.extend_from_slice(&color);
    }
    NodeKind::Image(ImageState::new(peniko::ImageData {
        data: peniko::Blob::from(bytes),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 2,
        height: 2,
    }))
}

fn text() -> NodeKind {
    NodeKind::Text(TextState {
        content: String::new(),
        font_family: "Roboto".to_string(),
        font_weight: 400.0,
        font_size: 16.0,
        align: TextAlign::Start,
        line_height: None,
        options: Default::default(),
    })
}

/// An 80x80 node at (10, 10) of the given kind, with this paint.
fn boxed(
    tree: &mut Tree,
    root: NodeId,
    kind: NodeKind,
    fill: Color,
    border_width: f64,
    radius: f64,
) -> NodeId {
    let id = tree.insert(
        kind,
        placed(10.0, 10.0, 80.0, 80.0),
        PaintProperties::new(fill, radius, 1.0),
    );
    tree.add_child(root, id);
    let paint = &mut tree.get_mut(id).unwrap().paint;
    paint.border_color = Animated::new(BORDER);
    paint.border_width = Animated::new(border_width);
    id
}

async fn frame(tree: &mut Tree, root: NodeId) -> Frame {
    layout(tree, root);
    Frame::of(tree, root).await
}

fn is(pixel: [u8; 4], color: Color) -> bool {
    pixel == rgba(color)
}

/// Kinds with no fill of their own to show through: the node's `fill` is the
/// background behind its content.
fn kinds_with_a_background() -> Vec<(&'static str, NodeKind)> {
    vec![
        ("image", image([0, 0, 0, 0])), // fully transparent: the fill shows through
        ("canvas", NodeKind::Canvas(CanvasState::new())),
        (
            "scroll_view",
            NodeKind::ScrollView(ScrollViewState::new(false)),
        ),
        (
            "virtual_list",
            NodeKind::VirtualList(VirtualListState::new(0, ItemExtent::Fixed(20.0))),
        ),
    ]
}

/// Every kind: the border paints on the node's own edge.
fn every_kind() -> Vec<(&'static str, NodeKind)> {
    let mut kinds = kinds_with_a_background();
    kinds.extend([
        ("text", text()),
        (
            "text_input",
            NodeKind::TextField(Box::new(TextFieldState::new("", "Roboto", 400.0, 16.0))),
        ),
        (
            "terminal",
            NodeKind::Terminal(Box::new(TerminalState::new(
                4,
                2,
                "Hack Nerd Font Mono",
                12.0,
            ))),
        ),
    ]);
    kinds
}

#[test]
fn fill_paints_behind_the_content() {
    pollster::block_on(async {
        for (name, kind) in kinds_with_a_background() {
            let (mut tree, root) = scene();
            boxed(&mut tree, root, kind, FILL, 0.0, 0.0);
            let frame = frame(&mut tree, root).await;
            assert!(
                is(frame.at(50, 50), FILL),
                "{name}: the fill shows at the center"
            );
            assert!(
                is(frame.at(5, 5), BACKGROUND),
                "{name}: and only inside the node"
            );
        }
    });
}

#[test]
fn stroke_paints_on_the_nodes_edge() {
    pollster::block_on(async {
        for (name, kind) in every_kind() {
            let (mut tree, root) = scene();
            boxed(&mut tree, root, kind, CLEAR, 4.0, 0.0);
            let frame = frame(&mut tree, root).await;
            assert!(
                is(frame.at(12, 50), BORDER),
                "{name}: the left edge is the border"
            );
            assert!(
                is(frame.at(50, 12), BORDER),
                "{name}: the top edge is the border"
            );
            assert!(
                is(frame.at(50, 50), BACKGROUND),
                "{name}: the inside is left alone"
            );
        }
    });
}

#[test]
fn corner_radius_rounds_the_fill_and_the_border() {
    pollster::block_on(async {
        // A fill rounds the same way on every kind that has one.
        for (name, kind) in kinds_with_a_background() {
            let (mut tree, root) = scene();
            boxed(&mut tree, root, kind, FILL, 0.0, 30.0);
            let frame = frame(&mut tree, root).await;
            assert!(is(frame.at(50, 50), FILL), "{name}: the fill is there");
            assert!(
                is(frame.at(11, 11), BACKGROUND),
                "{name}: the corner is rounded off"
            );
        }
        // And a border follows the radius: the corner pixel is empty, the
        // straight part of the edge is not.
        for (name, kind) in every_kind() {
            let (mut tree, root) = scene();
            boxed(&mut tree, root, kind, CLEAR, 4.0, 30.0);
            let frame = frame(&mut tree, root).await;
            assert!(is(frame.at(50, 11), BORDER), "{name}: the straight edge");
            assert!(
                is(frame.at(11, 11), BACKGROUND),
                "{name}: nothing in the rounded-off corner"
            );
        }
    });
}

/// An opaque image is clipped to its own rounded corners.
#[test]
fn an_image_is_clipped_to_its_corner_radius() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        boxed(&mut tree, root, image([0xFF, 0, 0, 0xFF]), CLEAR, 0.0, 30.0);
        let frame = frame(&mut tree, root).await;
        assert!(is(frame.at(50, 50), RED), "the image fills the node");
        assert!(
            is(frame.at(11, 11), BACKGROUND),
            "its corner is clipped to the radius"
        );
    });
}

/// The ordinary case: with no fill, border, or radius, no kind paints
/// anything of its own.
#[test]
fn nothing_else_changes() {
    pollster::block_on(async {
        for (name, kind) in every_kind() {
            let (mut tree, root) = scene();
            boxed(&mut tree, root, kind, CLEAR, 0.0, 0.0);
            let frame = frame(&mut tree, root).await;
            assert!(
                !frame.any_in(0, 0, 100, 100, |p| p != rgba(BACKGROUND)),
                "{name}: with no fill, border, or radius, nothing is painted"
            );
        }
    });
}
