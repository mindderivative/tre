//! M96: pixel proof for the node properties the renderer itself honors --
//! `visible`, `z_index` paint order, and the center-origin node transform.

mod support;
use support::*;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use peniko::Color;
use taffy::prelude::Style;

fn rect(tree: &mut Tree, parent: NodeId, x: f32, y: f32, w: f32, h: f32, fill: Color) -> NodeId {
    let id = tree.insert(
        NodeKind::Rect,
        placed(x, y, w, h),
        PaintProperties::new(fill, 0.0, 0.0, 1.0),
    );
    tree.add_child(parent, id);
    id
}

#[test]
fn a_hidden_node_paints_nothing_with_its_subtree() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let parent = rect(&mut tree, root, 10.0, 10.0, 40.0, 40.0, RED);
        rect(&mut tree, parent, 5.0, 5.0, 10.0, 10.0, GREEN);
        tree.get_mut(parent).unwrap().visible = false;
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(!frame.any_in(0, 0, 100, 100, |p| p != rgba(BACKGROUND)));
    });
}

#[test]
fn a_higher_z_index_paints_on_top_whatever_the_child_order() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let red = rect(&mut tree, root, 20.0, 20.0, 40.0, 40.0, RED);
        rect(&mut tree, root, 20.0, 20.0, 40.0, 40.0, GREEN);
        layout(&mut tree, root);
        assert_eq!(Frame::of(&tree, root).await.at(40, 40), rgba(GREEN));
        tree.get_mut(red).unwrap().z_index = 1;
        assert_eq!(Frame::of(&tree, root).await.at(40, 40), rgba(RED));
    });
}

#[test]
fn a_rotation_turns_the_node_about_its_center() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        // A 60x20 bar across the middle; turned 90 degrees it stands upright.
        let bar = rect(&mut tree, root, 20.0, 40.0, 60.0, 20.0, RED);
        layout(&mut tree, root);
        let flat = Frame::of(&tree, root).await;
        assert_eq!(flat.at(25, 50), rgba(RED));
        assert_eq!(flat.at(50, 25), rgba(BACKGROUND));

        tree.get_mut(bar).unwrap().paint.node_transform.rotation_deg =
            engine_core::Animated::new(90.0);
        let upright = Frame::of(&tree, root).await;
        assert_eq!(
            upright.at(50, 25),
            rgba(RED),
            "now spans the vertical middle"
        );
        assert_eq!(upright.at(25, 50), rgba(BACKGROUND));
    });
}

fn text(tree: &mut Tree, parent: NodeId, style: Style, content: &str) -> NodeId {
    let id = tree.insert(
        NodeKind::Text(engine_core::TextState {
            content: content.to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 400.0,
            font_size: 16.0,
            align: engine_core::TextAlign::Start,
            line_height: None,
            options: engine_core::TextOptions::default(),
        }),
        style,
        PaintProperties::new(WHITE, 0.0, 0.0, 1.0),
    );
    tree.add_child(parent, id);
    id
}

fn options(tree: &mut Tree, id: NodeId, change: impl FnOnce(&mut engine_core::TextOptions)) {
    let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind else {
        unreachable!()
    };
    change(&mut state.options);
}

fn inked(p: [u8; 4]) -> bool {
    p[0] > 0x60
}

const LONG: &str = "Mmmmmmmmmm Mmmmmmmmmm Mmmmmmmmmm Mmmmmmmmmm";

#[test]
fn an_unwrapped_ellipsis_stays_inside_the_box_and_reaches_its_edge() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let label = text(&mut tree, root, placed(5.0, 5.0, 70.0, 30.0), LONG);
        options(&mut tree, label, |o| {
            o.wrap = false;
            o.ellipsis = true;
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            !frame.any_in(76, 0, 100, 100, inked),
            "nothing past the box"
        );
        assert!(
            frame.any_in(62, 5, 75, 30, inked),
            "the ellipsis near its end"
        );
    });
}

#[test]
fn a_clip_without_an_ellipsis_cuts_at_the_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let label = text(&mut tree, root, placed(5.0, 5.0, 70.0, 30.0), LONG);
        options(&mut tree, label, |o| o.wrap = false);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(!frame.any_in(76, 0, 100, 100, inked), "clipped at the box");
        assert!(
            frame.any_in(66, 5, 75, 30, inked),
            "cut glyphs run to the edge"
        );
    });
}

#[test]
fn max_lines_draws_no_line_past_the_limit() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let label = text(&mut tree, root, placed(0.0, 0.0, 100.0, 100.0), LONG);
        layout(&mut tree, root);
        let all = Frame::of(&tree, root).await;
        assert!(
            all.any_in(0, 40, 100, 100, inked),
            "wraps onto a third line"
        );
        options(&mut tree, label, |o| o.max_lines = Some(2));
        let two = Frame::of(&tree, root).await;
        assert!(!two.any_in(0, 40, 100, 100, inked), "only two lines");
    });
}

#[test]
fn synthesized_italics_lean_right() {
    pollster::block_on(async {
        // The leftmost inked column of a tall "l" at its top and bottom.
        async fn lean(italic: bool) -> i32 {
            let (mut tree, root) = scene();
            let label = text(&mut tree, root, placed(40.0, 10.0, 50.0, 40.0), "l");
            options(&mut tree, label, |o| o.italic = italic);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let leftmost = |y: u32| (30..70).find(|&x| inked(frame.at(x, y))).unwrap_or(0) as i32;
            let rows: Vec<u32> = (10..50)
                .filter(|&y| (30..70).any(|x| inked(frame.at(x, y))))
                .collect();
            let (top, bottom) = (*rows.first().unwrap(), *rows.last().unwrap());
            leftmost(top + 1) - leftmost(bottom - 1)
        }
        assert!(lean(false).await.abs() <= 1, "upright");
        assert!(lean(true).await >= 2, "leans right toward the top");
    });
}
