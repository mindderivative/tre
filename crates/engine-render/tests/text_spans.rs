//! 0.5.4 (#112): style spans on a text node -- colour, weight, italic,
//! underline and strikethrough over a range of its text.

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextOptions, TextSpan, TextState, Tree};
use engine_render::{Damage, DamageTracker, TextRenderer, snapshot};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const W: u32 = 300;
const H: u32 = 50;
const BASE: Color = Color::from_rgba8(255, 255, 255, 255);
const RED: Color = Color::from_rgba8(255, 0, 0, 255);

fn tree(content: &str, spans: Vec<TextSpan>) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Text(TextState {
            content: content.to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 400.0,
            font_size: 28.0,
            align: TextAlign::Start,
            line_height: None,
            options: TextOptions {
                spans,
                ..Default::default()
            },
        }),
        Style {
            size: Size {
                width: length(W as f32),
                height: length(H as f32),
            },
            ..Default::default()
        },
        PaintProperties::new(BASE, 0.0, 1.0),
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(W as f32),
            height: AvailableSpace::Definite(H as f32),
        },
    );
    (tree, root)
}

fn render(content: &str, spans: Vec<TextSpan>) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("text spans"));
    let (tree, root) = tree(content, spans);
    snapshot(&device, &queue, &tree, root, W, H, 1.0, 0.0).unwrap()
}

fn span(start: usize, end: usize) -> TextSpan {
    TextSpan {
        start,
        end,
        ..Default::default()
    }
}

/// `(ink pixels, reddish ink pixels)` in columns `x0..x1`.
fn ink_in(s: &engine_render::Snapshot, x0: u32, x1: u32) -> (usize, usize) {
    let (mut all, mut red) = (0, 0);
    for y in 0..s.height {
        for x in x0..x1 {
            let i = ((y * s.width + x) * 4) as usize;
            let p = &s.rgba[i..i + 4];
            if p[3] > 128 {
                all += 1;
                if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                    red += 1;
                }
            }
        }
    }
    (all, red)
}

#[test]
fn a_colour_span_colours_only_its_range() {
    let plain = render("Hello world", vec![]);
    let spanned = render(
        "Hello world",
        vec![TextSpan {
            color: Some(RED),
            ..span(0, 5)
        }],
    );
    let (_, plain_red) = ink_in(&plain, 0, W);
    assert_eq!(plain_red, 0, "no span, no red");
    let (_, red_left) = ink_in(&spanned, 0, 70);
    assert!(red_left > 100, "'Hello' is red: {red_left}");
    let (ink_right, red_right) = ink_in(&spanned, 100, W);
    assert!(
        ink_right > 100 && red_right == 0,
        "'world' keeps the node's colour"
    );
}

#[test]
fn a_bold_span_has_more_ink_than_the_same_text_regular() {
    // Regular and medium are the bundled weights; 500 is the heavier.
    let regular = render("Hello world", vec![]);
    let heavy = render(
        "Hello world",
        vec![TextSpan {
            weight: Some(500.0),
            ..span(0, 11)
        }],
    );
    assert!(
        ink_in(&heavy, 0, W).0 > ink_in(&regular, 0, W).0,
        "a heavier weight inks more"
    );
}

#[test]
fn an_underline_draws_a_rule_under_its_range_only() {
    let plain = render("Hello world", vec![]);
    let underlined = render(
        "Hello world",
        vec![TextSpan {
            underline: true,
            ..span(0, 5)
        }],
    );
    // The widest single row of ink: a rule is a row of consecutive pixels.
    let longest_run = |s: &engine_render::Snapshot, x0: u32, x1: u32| -> u32 {
        let mut best = 0;
        for y in 0..s.height {
            let mut run = 0;
            for x in x0..x1 {
                let i = ((y * s.width + x) * 4) as usize;
                if s.rgba[i + 3] > 200 {
                    run += 1;
                    best = best.max(run);
                } else {
                    run = 0;
                }
            }
        }
        best
    };
    assert!(
        longest_run(&plain, 0, W) < 25,
        "glyphs have no long horizontal runs"
    );
    assert!(longest_run(&underlined, 0, 70) > 45, "a rule under 'Hello'");
    assert!(longest_run(&underlined, 110, W) < 25, "none under 'world'");
}

#[test]
fn a_strikethrough_draws_a_rule_through_the_middle() {
    let struck = render(
        "Hello",
        vec![TextSpan {
            strikethrough: true,
            ..span(0, 5)
        }],
    );
    let plain = render("Hello", vec![]);
    let row_ink = |s: &engine_render::Snapshot, y: u32| -> usize {
        (0..W)
            .filter(|x| s.rgba[((y * s.width + x) * 4 + 3) as usize] > 200)
            .count()
    };
    // Find the row with the most extra ink: it is across the text's middle.
    let (best_row, extra) = (0..H)
        .map(|y| (y, row_ink(&struck, y).saturating_sub(row_ink(&plain, y))))
        .max_by_key(|(_, e)| *e)
        .unwrap();
    assert!(
        extra > 40,
        "a rule's worth of extra ink on row {best_row}: {extra}"
    );
    assert!(
        (8..30).contains(&best_row),
        "through the text, not below it: {best_row}"
    );
}

#[test]
fn spans_outside_the_text_or_inside_a_character_do_not_panic() {
    // "é" is two bytes: a span ending inside it is clamped to a boundary.
    let s = render(
        "aébc",
        vec![
            TextSpan {
                color: Some(RED),
                ..span(0, 2)
            },
            TextSpan {
                color: Some(RED),
                ..span(3, 400)
            },
            span(10, 20),
            span(5, 5),
        ],
    );
    assert_eq!((s.width, s.height), (W, H));
}

#[test]
fn a_later_span_wins_where_they_overlap() {
    let blue = Color::from_rgba8(0, 0, 255, 255);
    let s = render(
        "Hello",
        vec![
            TextSpan {
                color: Some(RED),
                ..span(0, 5)
            },
            TextSpan {
                color: Some(blue),
                ..span(0, 5)
            },
        ],
    );
    assert_eq!(ink_in(&s, 0, W).1, 0, "red is overridden");
}

#[test]
fn spans_are_part_of_what_a_text_node_damages() {
    let (mut tree, root) = tree("Hello world", vec![]);
    let mut tracker = DamageTracker::new();
    let mut text = TextRenderer::new();
    let size = (W as u16, H as u16);
    assert_eq!(
        tracker.damage(&tree, root, size.0, size.1, &mut text),
        Damage::Full
    );
    assert_eq!(
        tracker.damage(&tree, root, size.0, size.1, &mut text),
        Damage::None
    );
    if let NodeKind::Text(state) = &mut tree.get_mut(root).unwrap().kind {
        state.options.spans = vec![TextSpan {
            underline: true,
            ..span(0, 5)
        }];
    }
    assert_ne!(
        tracker.damage(&tree, root, size.0, size.1, &mut text),
        Damage::None
    );
}

#[test]
fn a_selection_paints_a_highlight_behind_its_glyphs_and_damages_the_node() {
    let plain = render("Hello world", vec![]);
    let (mut t, root) = tree("Hello world", vec![]);
    let mut tracker = DamageTracker::new();
    let mut text = TextRenderer::new();
    let size = (W as u16, H as u16);
    tracker.damage(&t, root, size.0, size.1, &mut text);
    assert_eq!(
        tracker.damage(&t, root, size.0, size.1, &mut text),
        Damage::None
    );
    if let NodeKind::Text(state) = &mut t.get_mut(root).unwrap().kind {
        state.options.selection = Some((0, 5));
    }
    assert_ne!(
        tracker.damage(&t, root, size.0, size.1, &mut text),
        Damage::None,
        "a selection change repaints the node"
    );
    let (device, queue) = pollster::block_on(support::device("selection"));
    let selected = snapshot(&device, &queue, &t, root, W, H, 1.0, 0.0).unwrap();
    // The highlight is translucent text colour over the whole of 'Hello':
    // pixels with alpha where the plain frame has none, inside its box.
    let tinted = |s: &engine_render::Snapshot| -> usize {
        (0..H)
            .flat_map(|y| (0..70u32).map(move |x| (x, y)))
            .filter(|(x, y)| s.rgba[((y * s.width + x) * 4 + 3) as usize] > 0)
            .count()
    };
    assert!(
        tinted(&selected) > tinted(&plain) + 500,
        "a highlight rectangle"
    );
    // Past the selection the frame is the plain one.
    let row = (W * 4) as usize;
    let same = (0..H as usize).all(|y| {
        let (from, to) = (y * row + 100 * 4, (y + 1) * row);
        selected.rgba[from..to] == plain.rgba[from..to]
    });
    assert!(same, "nothing past 'Hello' is highlighted");
}

#[test]
fn a_selection_does_not_reshape_the_text() {
    let a = TextOptions::default();
    let b = TextOptions {
        selection: Some((1, 4)),
        selectable: true,
        ..Default::default()
    };
    assert!(a.same_layout(&b), "selection state is not layout");
    let c = TextOptions {
        spans: vec![span(0, 1)],
        ..Default::default()
    };
    assert!(!a.same_layout(&c), "spans are layout");
}

/// The rightmost column with ink.
fn right_edge(s: &engine_render::Snapshot) -> u32 {
    (0..s.width)
        .rev()
        .find(|&x| (0..s.height).any(|y| s.rgba[((y * s.width + x) * 4 + 3) as usize] > 128))
        .unwrap_or(0)
}

#[test]
fn a_size_span_resizes_its_range() {
    let plain = render("Hello world", vec![]);
    let small = render(
        "Hello world",
        vec![TextSpan {
            font_size: Some(14.0),
            ..span(0, 11)
        }],
    );
    let (plain_ink, _) = ink_in(&plain, 0, W);
    let (small_ink, _) = ink_in(&small, 0, W);
    assert!(small_ink * 2 < plain_ink, "{small_ink} vs {plain_ink}");
    // And only its range: a small word followed by full-size ones.
    let part = render(
        "Hello world",
        vec![TextSpan {
            font_size: Some(14.0),
            ..span(0, 5)
        }],
    );
    let (all, _) = ink_in(&part, 0, W);
    assert!(
        all < plain_ink && all * 2 > plain_ink,
        "{all} vs {plain_ink}"
    );
}

#[test]
fn a_family_span_changes_the_face_of_its_range() {
    let text = "iiiiii iiiiii";
    let plain = render(text, vec![]);
    let mono = render(
        text,
        vec![TextSpan {
            font_family: Some("Hack Nerd Font Mono".to_string()),
            ..span(7, 13)
        }],
    );
    // Monospaced 'i's are as wide as any glyph; Roboto's are narrow.
    assert!(right_edge(&mono) > right_edge(&plain) + 15);
}

#[test]
fn a_size_or_family_change_damages_the_text_and_a_link_does_not() {
    let (mut tree, root) = tree("Hello world", vec![span(0, 5)]);
    let mut text = TextRenderer::new();
    let mut tracker = DamageTracker::new();
    let (w, h) = (W as u16, H as u16);
    assert_eq!(tracker.damage(&tree, root, w, h, &mut text), Damage::Full);
    assert_eq!(tracker.damage(&tree, root, w, h, &mut text), Damage::None);
    let set = |tree: &mut Tree, f: &dyn Fn(&mut TextSpan)| {
        let NodeKind::Text(state) = &mut tree.get_mut(root).unwrap().kind else {
            unreachable!()
        };
        f(&mut state.options.spans[0]);
    };
    set(&mut tree, &|s| s.font_size = Some(40.0));
    assert_ne!(tracker.damage(&tree, root, w, h, &mut text), Damage::None);
    set(&mut tree, &|s| {
        s.font_family = Some("Hack Nerd Font Mono".into())
    });
    assert_ne!(tracker.damage(&tree, root, w, h, &mut text), Damage::None);
    // A link changes nothing that is drawn.
    set(&mut tree, &|s| s.link = Some("https://example.com".into()));
    assert_eq!(tracker.damage(&tree, root, w, h, &mut text), Damage::None);
}

#[test]
fn unspanned_text_follows_the_nodes_colour_when_it_changes() {
    use engine_core::Animated;
    let (device, queue) = pollster::block_on(support::device("text spans"));
    let (mut tree, root) = tree(
        "Hello world",
        vec![TextSpan {
            color: Some(RED),
            ..span(0, 5)
        }],
    );
    // The node's own colour is green now; 'world' follows it, 'Hello' stays red.
    tree.get_mut(root).unwrap().paint.background = Animated::new(Color::from_rgba8(0, 255, 0, 255));
    let s = snapshot(&device, &queue, &tree, root, W, H, 1.0, 0.0).unwrap();
    let green = |x0: u32, x1: u32| {
        (0..s.height)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let i = ((y * s.width + x) * 4) as usize;
                s.rgba[i + 3] > 128 && s.rgba[i + 1] > 200 && s.rgba[i] < 80
            })
            .count()
    };
    assert!(green(100, W) > 100, "'world' is the node's green");
    assert_eq!(green(0, 70), 0, "'Hello' keeps its red");
}
