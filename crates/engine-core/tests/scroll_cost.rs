//! 0.5.4 (#105): `cargo test -p engine-core --release --test scroll_cost -- --ignored --nocapture`
use engine_core::{NodeKind, PaintProperties, ScrollViewState, Tree};
use peniko::Color;
use std::time::Instant;
use taffy::prelude::{AvailableSpace, FlexDirection, Size, Style, length};

#[test]
#[ignore = "timing, not correctness"]
fn scrolling_3000_rows() {
    let mut tree = Tree::new();
    let paint = || PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        Style {
            size: Size {
                width: length(400.0),
                height: length(600.0),
            },
            ..Default::default()
        },
        paint(),
    );
    let content = tree.insert(
        NodeKind::Container,
        Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: length(400.0),
                height: taffy::prelude::auto(),
            },
            ..Default::default()
        },
        paint(),
    );
    tree.add_child(view, content);
    for _ in 0..3000 {
        let row = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(400.0),
                    height: length(24.0),
                },
                flex_shrink: 0.0,
                ..Default::default()
            },
            paint(),
        );
        tree.add_child(content, row);
    }
    let avail = Size {
        width: AvailableSpace::Definite(400.0),
        height: AvailableSpace::Definite(600.0),
    };
    tree.compute_layout(view, avail);
    let runs = 500;
    let start = Instant::now();
    for _ in 0..runs {
        tree.scroll_scroll_view_by(view, 3.0);
        tree.compute_layout(view, avail);
    }
    println!("scroll+layout: {:?} per frame", start.elapsed() / runs);
}
