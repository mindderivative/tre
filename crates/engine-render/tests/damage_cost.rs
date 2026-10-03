//! 0.5.4 (#104): what the damage walk costs per frame as the tree grows.
//! `cargo test -p engine-render --release --test damage_cost -- --ignored --nocapture`

use std::time::Instant;

use engine_core::{NodeId, NodeKind, PaintProperties, Tree};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

/// A root with `n` rect nodes in rows of 96.
fn grid(n: usize) -> (Tree, NodeId, Vec<NodeId>) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            flex_wrap: taffy::FlexWrap::Wrap,
            size: Size {
                width: length(96.0 * 10.0),
                height: length((n / 96 + 1) as f32 * 10.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
    );
    let ids = (0..n)
        .map(|_| {
            let id = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(10.0),
                        height: length(10.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(255, 0, 0, 255), 0.0, 1.0),
            );
            tree.add_child(root, id);
            id
        })
        .collect();
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(960.0),
            height: AvailableSpace::Definite((n / 96 + 1) as f32 * 10.0),
        },
    );
    (tree, root, ids)
}

#[test]
#[ignore = "timing, not correctness"]
fn damage_walk_cost() {
    for n in [2_304usize, 9_216] {
        let (mut tree, root, ids) = grid(n);
        let (w, h) = (960u16, ((n / 96 + 1) * 10) as u16);
        let mut text = TextRenderer::new();
        let mut tracker = DamageTracker::new();
        assert_eq!(tracker.damage(&tree, root, w, h, &mut text), Damage::Full);
        let runs = 200;
        let start = Instant::now();
        for i in 0..runs {
            // One node changes colour each frame.
            tree.get_mut(ids[i % n]).unwrap().paint.background.current =
                Color::from_rgba8((i % 250) as u8, 9, 9, 255);
            let _ = tracker.damage(&tree, root, w, h, &mut text);
        }
        let per = start.elapsed() / runs as u32;
        println!(
            "{n} nodes: {per:?} per frame ({:?} per node)",
            per / n as u32
        );
    }
}
