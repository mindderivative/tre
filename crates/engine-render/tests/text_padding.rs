//! 0.5.1 (#44): a text node's `padding` insets its glyphs. Layout already
//! reserved the space -- taffy sizes the node padding-included -- but the
//! renderer drew text at the node's own corner, across its full width.
//! Same coarse "ink is here, not there" signal as `text_align.rs`: an exact
//! pixel match is brittle for anti-aliased glyphs.

mod support;
use support::*;

use engine_core::{NodeId, NodeKind, PaintProperties, TextAlign, TextState, Tree};
use taffy::geometry::Rect as TaffyRect;
use taffy::prelude::{Style, length};

/// A white 20px text node over `BACKGROUND`, placed at the origin with the
/// given size and padding `(left, top, right, bottom)`.
fn padded_text(
    tree: &mut Tree,
    root: NodeId,
    content: &str,
    size: (f32, f32),
    padding: (f32, f32, f32, f32),
) -> NodeId {
    let id = tree.insert(
        NodeKind::Text(TextState {
            content: content.to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 400.0,
            font_size: 20.0,
            align: TextAlign::Start,
            line_height: None,
            options: Default::default(),
        }),
        Style {
            padding: TaffyRect {
                left: length(padding.0),
                top: length(padding.1),
                right: length(padding.2),
                bottom: length(padding.3),
            },
            ..placed(0.0, 0.0, size.0, size.1)
        },
        PaintProperties::new(WHITE, 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn is_ink(pixel: [u8; 4]) -> bool {
    pixel != rgba(BACKGROUND)
}

/// The glyphs start inside the left and top padding, not at the node's corner.
#[test]
fn text_starts_inside_its_padding() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        padded_text(&mut tree, root, "Hi", (100.0, 60.0), (40.0, 25.0, 0.0, 0.0));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            !frame.any_in(0, 0, 40, 60, is_ink),
            "nothing in the 40px left padding"
        );
        assert!(
            !frame.any_in(0, 0, 100, 25, is_ink),
            "nothing in the 25px top padding"
        );
        assert!(
            frame.any_in(40, 25, 100, 60, is_ink),
            "the text is drawn in the content box"
        );
    });
}

/// Wrapping uses the content width: the right padding stays empty.
#[test]
fn text_wraps_within_the_content_width() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        // 10px left and 30px right padding leave a 60px content box. One
        // "WW" is about 38px, so two on a line (about 81px) don't fit and
        // the words wrap; at the node's full 100px width they would.
        padded_text(
            &mut tree,
            root,
            "WW WW WW",
            (100.0, 100.0),
            (10.0, 0.0, 30.0, 0.0),
        );
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            !frame.any_in(72, 0, 100, 100, is_ink),
            "nothing in the 30px right padding"
        );
        assert!(
            frame.any_in(10, 0, 70, 22, is_ink),
            "the first line is in the content box"
        );
        assert!(
            frame.any_in(10, 26, 70, 100, is_ink),
            "the words wrapped onto more lines"
        );
    });
}

/// Zero padding draws exactly where it always did: at the node's corner.
#[test]
fn unpadded_text_is_unchanged() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        padded_text(&mut tree, root, "Hi", (100.0, 60.0), (0.0, 0.0, 0.0, 0.0));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(frame.any_in(0, 0, 30, 30, is_ink), "ink at the corner");
    });
}
