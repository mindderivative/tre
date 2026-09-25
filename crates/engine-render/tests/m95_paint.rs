//! M95 Phase 3: pixel proof for the paint and animation building blocks --
//! a path's view-box fit, a trimmed stroke, a mid-morph frame, shadows,
//! per-corner radii, a color's own alpha, group opacity, a text input's
//! placeholder, a scroll view's scrollbar color, a terminal's palette,
//! and a ripple built only from primitives matching the built-in one.
//! Same headless render-to-texture-then-readback harness as
//! `icon_paint.rs`.

use engine_core::{
    CellColor, CornerRadii, Interpolate, NodeId, NodeKind, PaintProperties, PathData, PathState,
    RippleState, SCROLLBAR_MARGIN, ScrollViewState, Shadow, Shadows, TerminalCell, TerminalState,
    TextFieldState, Tree,
};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{Size, Style, length};

mod support;
use support::*;

fn path_node(tree: &mut Tree, parent: NodeId, style: Style, data: &str, fill: Color) -> NodeId {
    let id = tree.insert(
        NodeKind::Path(PathState::new(PathData::from_svg(data).unwrap())),
        style,
        PaintProperties::new(fill, 0.0, 0.0, 1.0),
    );
    tree.add_child(parent, id);
    id
}

fn with_path(tree: &mut Tree, id: NodeId, change: impl FnOnce(&mut PathState)) {
    let NodeKind::Path(state) = &mut tree.get_mut(id).unwrap().kind else {
        unreachable!()
    };
    change(state);
}

#[test]
fn a_path_fits_its_view_box_uniformly_and_centered() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let path = path_node(
            &mut tree,
            root,
            placed(0.0, 0.0, 100.0, 50.0),
            "M0,0 H10 V10 H0 Z",
            RED,
        );
        with_path(&mut tree, path, |s| {
            s.view_box = Some(peniko::kurbo::Rect::new(0.0, 0.0, 10.0, 10.0));
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // A 10x10 view box in a 100x50 box: scaled 5x, centered at x 25..75.
        assert_eq!(frame.at(50, 25), rgba(RED), "the fitted square");
        assert_eq!(frame.at(10, 25), rgba(BACKGROUND), "the left letterbox");
        assert_eq!(frame.at(90, 25), rgba(BACKGROUND), "the right letterbox");
        assert_eq!(frame.at(50, 75), rgba(BACKGROUND), "below the path's box");
    });
}

#[test]
fn a_trimmed_stroke_paints_only_its_fraction() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let path = path_node(
            &mut tree,
            root,
            placed(0.0, 0.0, 100.0, 100.0),
            "M10,50 L90,50",
            CLEAR,
        );
        tree.get_mut(path).unwrap().paint.border_color.current = RED;
        tree.get_mut(path).unwrap().paint.border_width.current = 10.0;
        with_path(&mut tree, path, |s| s.trim_end.current = 0.5);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(30, 50), rgba(RED), "inside the first half");
        assert_eq!(frame.at(75, 50), rgba(BACKGROUND), "past the trim");
        assert_eq!(
            frame.at(30, 20),
            rgba(BACKGROUND),
            "the path is stroked, not filled"
        );
    });
}

#[test]
fn a_mid_morph_frame_paints_between_the_two_paths() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let path = path_node(
            &mut tree,
            root,
            placed(0.0, 0.0, 100.0, 100.0),
            "M0,0 H20 V20 H0 Z",
            RED,
        );
        with_path(&mut tree, path, |s| {
            let from = s.data.current.clone();
            let to = PathData::from_svg("M80,0 H100 V20 H80 Z").unwrap();
            s.data.current = from.interpolate(&to, 0.5);
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(
            frame.at(50, 10),
            rgba(RED),
            "halfway, the square sits at x 40..60"
        );
        assert_eq!(frame.at(10, 10), rgba(BACKGROUND), "it has left the start");
        assert_eq!(
            frame.at(90, 10),
            rgba(BACKGROUND),
            "it hasn't reached the end"
        );
    });
}

#[test]
fn shadows_paint_under_the_node_offset_and_spread() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let card = tree.insert(
            NodeKind::Rect,
            placed(20.0, 20.0, 40.0, 40.0),
            PaintProperties::new(WHITE, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, card);
        tree.get_mut(card).unwrap().paint.shadows.current = Shadows(vec![Shadow {
            color: BLACK,
            offset_x: 0.0,
            offset_y: 30.0,
            blur: 0.0,
            spread: 4.0,
        }]);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(
            frame.at(40, 40),
            rgba(WHITE),
            "the node paints over its shadow"
        );
        assert_eq!(frame.at(40, 80), rgba(BLACK), "the offset shadow below it");
        assert_eq!(frame.at(62, 80), rgba(BLACK), "spread widens the shadow");
        assert_eq!(frame.at(40, 97), rgba(BACKGROUND), "past the shadow");
    });
}

#[test]
fn four_corner_radii_round_each_corner_on_its_own() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let card = tree.insert(
            NodeKind::Rect,
            placed(20.0, 20.0, 60.0, 60.0),
            PaintProperties::new(RED, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, card);
        tree.get_mut(card).unwrap().paint.corner_radii_override =
            Some(engine_core::Animated::new(CornerRadii([
                30.0, 0.0, 0.0, 0.0,
            ])));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(22, 22), rgba(BACKGROUND), "the rounded top-left");
        assert_eq!(frame.at(78, 22), rgba(RED), "the square top-right");
        assert_eq!(frame.at(22, 78), rgba(RED), "the square bottom-left");
    });
}

#[test]
fn a_colors_own_alpha_renders() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let card = tree.insert(
            NodeKind::Rect,
            placed(20.0, 20.0, 60.0, 60.0),
            PaintProperties::new(Color::from_rgba8(0xFF, 0x00, 0x00, 0x80), 0.0, 0.0, 1.0),
        );
        tree.add_child(root, card);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // Half red over 0x11: about 0x88 red, 0x08 green and blue.
        let pixel = frame.at(50, 50);
        assert!(
            close(pixel, [0x88, 0x08, 0x08, 0xFF], 3),
            "blended, got {pixel:?}"
        );
    });
}

#[test]
fn opacity_fades_a_node_and_its_children_as_one() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let group = tree.insert(
            NodeKind::Rect,
            placed(20.0, 20.0, 60.0, 60.0),
            PaintProperties::new(CLEAR, 0.0, 0.0, 0.5),
        );
        tree.add_child(root, group);
        let child = tree.insert(
            NodeKind::Rect,
            placed(0.0, 0.0, 60.0, 60.0),
            PaintProperties::new(RED, 0.0, 0.0, 1.0),
        );
        tree.add_child(group, child);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let pixel = frame.at(50, 50);
        assert!(
            close(pixel, [0x88, 0x08, 0x08, 0xFF], 3),
            "the child fades with its parent, got {pixel:?}"
        );
    });
}

#[test]
fn an_empty_text_input_shows_its_placeholder_in_its_color() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = TextFieldState::new("", "Roboto", 400.0, 20.0);
        state.placeholder = "WWW".to_string();
        state.placeholder_fill = Some(GREEN);
        let field = tree.insert(
            NodeKind::TextField(state),
            placed(0.0, 0.0, 100.0, 40.0),
            PaintProperties::new(BLACK, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, field);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            frame.any_in(0, 0, 100, 40, |[r, g, b, _]| g > 150 && r < 80 && b < 80),
            "the placeholder's glyphs paint in placeholder_fill"
        );
    });
}

#[test]
fn a_scroll_view_paints_its_scrollbar_fill_and_width() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = ScrollViewState::new(false);
        state.scrollbar_fill = Some(RED);
        state.scrollbar_width = 10.0;
        let view = tree.insert(
            NodeKind::ScrollView(state),
            placed(0.0, 0.0, 100.0, 100.0),
            PaintProperties::new(CLEAR, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, view);
        let content = tree.insert(
            NodeKind::Rect,
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(400.0),
                },
                ..Default::default()
            },
            PaintProperties::new(CLEAR, 0.0, 0.0, 1.0),
        );
        tree.add_child(view, content);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let x = (100.0 - SCROLLBAR_MARGIN - 5.0) as u32;
        let y = (SCROLLBAR_MARGIN + 5.0) as u32;
        assert_eq!(frame.at(x, y), rgba(RED), "a 10px red thumb");
        assert_eq!(frame.at(x - 8, y), rgba(BACKGROUND), "and no wider");
    });
}

#[test]
fn a_terminal_paints_indexed_colors_from_its_palette() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = TerminalState::new(4, 1, "Hack Nerd Font Mono", 24.0);
        state.palette.ansi[2] = GREEN;
        for cell in state.cells.iter_mut() {
            *cell = TerminalCell {
                ch: '\u{2588}',
                fg: CellColor::Indexed(2),
                ..TerminalCell::blank()
            };
        }
        let terminal = tree.insert(
            NodeKind::Terminal(state),
            placed(0.0, 0.0, 100.0, 40.0),
            PaintProperties::new(BLACK, 0.0, 0.0, 1.0),
        );
        tree.add_child(root, terminal);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            frame.any_in(0, 0, 100, 40, |[r, g, b, _]| g > 200 && r < 60 && b < 60),
            "cells colored by index 2 paint in the palette's own green"
        );
    });
}

/// The proof ripple: a box that clips its children, holding a circle
/// `path` at the press point whose group opacity is the ripple's -- built
/// only from M95 primitives -- paints the same pixels as the engine's own
/// built-in ripple at the same radius and opacity.
#[test]
fn a_ripple_built_from_primitives_matches_the_built_in_one() {
    pollster::block_on(async {
        let origin = Point::new(30.0, 30.0);
        let radius = 30.0;
        let opacity = 0.12;

        let built_in = {
            let (mut tree, root) = scene();
            let card = tree.insert(
                NodeKind::Rect,
                placed(10.0, 10.0, 80.0, 80.0),
                PaintProperties::new(WHITE, 0.0, 0.0, 1.0),
            );
            tree.add_child(root, card);
            let interaction = tree.interaction_mut(card).unwrap();
            interaction.tint = BLACK;
            let mut ripple = RippleState::new(
                origin,
                radius,
                opacity,
                std::time::Duration::from_millis(1),
                std::time::Instant::now(),
            );
            ripple.radius.current = radius;
            ripple.radius.active = None;
            ripple.opacity.current = opacity;
            ripple.opacity.active = None;
            interaction.ripples.push(ripple);
            layout(&mut tree, root);
            Frame::of(&tree, root).await
        };

        let primitive = {
            let (mut tree, root) = scene();
            let card = tree.insert(
                NodeKind::Rect,
                placed(10.0, 10.0, 80.0, 80.0),
                PaintProperties::new(WHITE, 0.0, 0.0, 1.0),
            );
            tree.add_child(root, card);
            tree.get_mut(card).unwrap().paint.clip_children = true;
            // The circle in the card's own coordinates: the press point
            // (30, 30) in the window is (20, 20) in the card.
            let (cx, cy) = (origin.x - 10.0, origin.y - 10.0);
            let circle = format!(
                "M{},{cy} A{radius},{radius} 0 1 0 {},{cy} A{radius},{radius} 0 1 0 {},{cy} Z",
                cx - radius,
                cx + radius,
                cx - radius,
            );
            let ink = path_node(
                &mut tree,
                card,
                placed(0.0, 0.0, 80.0, 80.0),
                &circle,
                BLACK,
            );
            tree.get_mut(ink).unwrap().paint.opacity.current = opacity;
            layout(&mut tree, root);
            Frame::of(&tree, root).await
        };

        for (x, y, what) in [
            (30, 30, "the ripple's center"),
            (50, 30, "inside the ripple"),
            (75, 75, "the card outside the ripple"),
            (5, 5, "outside the card, where the clip holds the ripple"),
        ] {
            let (a, b) = (built_in.at(x, y), primitive.at(x, y));
            assert!(close(a, b, 2), "{what}: built-in {a:?}, primitive {b:?}");
        }
        // And the ripple really is there: tinted, not the plain card.
        assert!(built_in.at(30, 30)[0] < 0xF0, "the ripple tints the card");
    });
}
