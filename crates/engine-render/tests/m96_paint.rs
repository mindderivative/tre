//! M96: pixel proof for the node properties the renderer itself honors --
//! `visible`, `z_index` paint order, and the center-origin node transform.

mod support;
use support::*;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use peniko::Color;

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
