//! 0.5.4 (#108): `snapshot` renders a laid-out tree offscreen and returns its
//! pixels.

mod support;

use engine_core::{NodeKind, PaintProperties, Tree};
use engine_render::snapshot;
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const BG: [u8; 4] = [0x11, 0x22, 0x33, 0xFF];
const RED: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];

fn scene(w: f32, h: f32) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(w),
                height: length(h),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(BG[0], BG[1], BG[2], 255), 0.0, 1.0),
    );
    let boxed = tree.insert(
        NodeKind::Rect,
        Style {
            position: Position::Absolute,
            inset: TaffyRect {
                left: length(10.0),
                top: length(5.0),
                right: auto(),
                bottom: auto(),
            },
            size: Size {
                width: length(20.0),
                height: length(10.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(255, 0, 0, 255), 0.0, 1.0),
    );
    tree.add_child(root, boxed);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(w),
            height: AvailableSpace::Definite(h),
        },
    );
    (tree, root)
}

fn px(s: &engine_render::Snapshot, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * s.width + x) * 4) as usize;
    [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2], s.rgba[i + 3]]
}

#[test]
fn a_snapshot_has_the_trees_pixels_with_no_row_padding() {
    let (device, queue) = pollster::block_on(support::device("snapshot"));
    // 50 wide: 200 bytes a row, which needs padding to 256 on the GPU.
    let (tree, root) = scene(50.0, 30.0);
    let s = snapshot(&device, &queue, &tree, root, 50, 30, 1.0, 0.0).unwrap();
    assert_eq!((s.width, s.height), (50, 30));
    assert_eq!(s.rgba.len(), 50 * 30 * 4);
    assert_eq!(px(&s, 0, 0), BG);
    assert_eq!(px(&s, 49, 29), BG);
    assert_eq!(px(&s, 15, 8), RED);
    assert_eq!(px(&s, 9, 8), BG);
    assert_eq!(px(&s, 30, 8), BG);
}

#[test]
fn a_snapshot_at_scale_2_is_twice_the_pixels() {
    let (device, queue) = pollster::block_on(support::device("snapshot"));
    let (tree, root) = scene(50.0, 30.0);
    let s = snapshot(&device, &queue, &tree, root, 100, 60, 2.0, 0.0).unwrap();
    assert_eq!(px(&s, 30, 16), RED);
    assert_eq!(px(&s, 19, 16), BG);
    assert_eq!(px(&s, 60, 16), BG);
}

#[test]
fn sizes_the_gpu_cannot_render_are_errors() {
    let (device, queue) = pollster::block_on(support::device("snapshot"));
    let (tree, root) = scene(50.0, 30.0);
    assert!(snapshot(&device, &queue, &tree, root, 0, 30, 1.0, 0.0).is_err());
    assert!(snapshot(&device, &queue, &tree, root, 50, 70_000, 1.0, 0.0).is_err());
}
