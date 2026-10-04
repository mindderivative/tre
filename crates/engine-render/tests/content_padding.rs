//! 0.5.1 (#53): `padding` makes a content box, and a kind's own content draws
//! inside it -- a text input's text, a terminal's cells, an image, a path, a
//! canvas's painter coordinates -- as children and (since #44) text already
//! do. Before, each drew from the node's own corner across its whole box, so
//! `set(padding=...)` on them was accepted and did nothing visible.

mod support;
use support::*;

use engine_core::{
    Animated, CanvasState, CellColor, DrawCommand, ImageState, NodeId, NodeKind, PaintProperties,
    PathData, PathState, TerminalState, TextFieldState, Tree,
};
use peniko::Color;
use taffy::geometry::Rect as TaffyRect;
use taffy::prelude::{Style, length};

/// An 80x80 node at (10, 10) with padding left 30, top 20, right 10, bottom
/// 10: its content box is x 40..80, y 30..80.
fn padded(tree: &mut Tree, root: NodeId, kind: NodeKind) -> NodeId {
    let id = tree.insert(
        kind,
        Style {
            padding: TaffyRect {
                left: length(30.0),
                top: length(20.0),
                right: length(10.0),
                bottom: length(10.0),
            },
            ..placed(10.0, 10.0, 80.0, 80.0)
        },
        PaintProperties::new(CLEAR, 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn is_ink(pixel: [u8; 4]) -> bool {
    pixel != rgba(BACKGROUND)
}

async fn frame(tree: &mut Tree, root: NodeId) -> Frame {
    layout(tree, root);
    Frame::of(tree, root).await
}

/// Nothing in the left or top padding (or the right and bottom), and
/// something in the content box.
fn assert_inside_the_content_box(frame: &Frame, what: &str) {
    assert!(
        !frame.any_in(10, 10, 40, 90, is_ink),
        "{what}: the left padding is empty"
    );
    assert!(
        !frame.any_in(10, 10, 90, 30, is_ink),
        "{what}: the top padding is empty"
    );
    assert!(
        frame.any_in(40, 30, 80, 80, is_ink),
        "{what}: drawn in the content box"
    );
}

#[test]
fn a_text_input_draws_inside_its_padding() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = TextFieldState::new("Hi", "Roboto", 400.0, 16.0);
        state.text_tint = Animated::new(WHITE);
        padded(&mut tree, root, NodeKind::TextField(Box::new(state)));
        let frame = frame(&mut tree, root).await;
        assert_inside_the_content_box(&frame, "text input");
    });
}

#[test]
fn a_terminal_draws_its_cells_inside_its_padding() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = TerminalState::new(4, 2, "Hack Nerd Font Mono", 12.0);
        state.cells[0].bg = CellColor::Rgb(RED); // the first cell
        padded(&mut tree, root, NodeKind::Terminal(Box::new(state)));
        let frame = frame(&mut tree, root).await;
        assert_inside_the_content_box(&frame, "terminal");
        assert_eq!(
            frame.at(41, 31),
            rgba(RED),
            "the first cell starts at the content box"
        );
    });
}

#[test]
fn an_image_fits_into_its_content_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut bytes = Vec::new();
        for _ in 0..4 {
            bytes.extend_from_slice(&[0xFF, 0, 0, 0xFF]);
        }
        let image = peniko::ImageData {
            data: peniko::Blob::from(bytes),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 2,
            height: 2,
        };
        padded(&mut tree, root, NodeKind::Image(ImageState::new(image)));
        let frame = frame(&mut tree, root).await;
        assert_inside_the_content_box(&frame, "image");
        assert_eq!(
            frame.at(60, 55),
            rgba(RED),
            "the image fills the content box"
        );
        assert!(
            !frame.any_in(80, 10, 90, 90, is_ink),
            "and the right padding is empty"
        );
    });
}

#[test]
fn a_path_fits_its_view_box_into_the_content_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = PathState::new(PathData::from_svg("M0,0 H10 V10 H0 Z").unwrap());
        state.view_box = Some(peniko::kurbo::Rect::new(0.0, 0.0, 10.0, 10.0));
        let id = padded(&mut tree, root, NodeKind::Path(state));
        tree.get_mut(id).unwrap().paint.background = Animated::new(RED);
        let frame = frame(&mut tree, root).await;
        assert_inside_the_content_box(&frame, "path");
        // A square in a 40x50 content box: 40x40, centered at y 35..75.
        assert_eq!(frame.at(60, 55), rgba(RED), "the fitted square");
        assert_eq!(
            frame.at(60, 32),
            rgba(BACKGROUND),
            "centered, not top-aligned"
        );
    });
}

#[test]
fn a_canvas_paints_from_the_content_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = CanvasState::new();
        state.commands.push(DrawCommand::FillRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: Color::from_rgba8(0xFF, 0, 0, 0xFF),
            gradient: None,
        });
        padded(&mut tree, root, NodeKind::Canvas(state));
        let frame = frame(&mut tree, root).await;
        assert_inside_the_content_box(&frame, "canvas");
        assert_eq!(
            frame.at(45, 35),
            rgba(RED),
            "painter (0, 0) is the content box's corner"
        );
        assert_eq!(frame.at(15, 15), rgba(BACKGROUND), "not the node's corner");
    });
}

/// With no padding nothing moves: each kind paints as it always did.
#[test]
fn unpadded_content_is_where_it_was() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = CanvasState::new();
        state.commands.push(DrawCommand::FillRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: Color::from_rgba8(0xFF, 0, 0, 0xFF),
            gradient: None,
        });
        let id = tree.insert(
            NodeKind::Canvas(state),
            placed(10.0, 10.0, 80.0, 80.0),
            PaintProperties::new(CLEAR, 0.0, 1.0),
        );
        tree.add_child(root, id);
        let frame = frame(&mut tree, root).await;
        assert_eq!(frame.at(15, 15), rgba(RED));
    });
}
