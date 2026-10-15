//! 0.5.6.2 (#176, #177): a text node takes its text's own size on a side marked
//! `FitContent`, whatever the parent's `align_items`; an unmarked side stays the
//! zero a text node has always had. The text is measured with a stand-in (10 px a
//! character, 20 px a line), so the layout is what is under test, not a font.

use engine_core::{FitContent, NodeId, NodeKind, PaintProperties, TextAlign, TextState, Tree};
use peniko::Color;
use taffy::prelude::{AlignItems, AvailableSpace, FlexDirection, Size, Style, length};

const LABEL: &str = "Julia set, drawn by a fill shader";

/// 10 px a character, 20 px a line; wraps greedily at spaces within `max_width`
/// (`None` is one line). A word wider than the space gets a line of its own.
fn measure(state: &TextState, max_width: Option<f32>) -> (f32, f32) {
    let limit = max_width.unwrap_or(f32::MAX);
    let (mut widest, mut lines, mut line) = (0.0_f32, 1.0_f32, 0.0_f32);
    for (i, word) in state.content.split(' ').enumerate() {
        let w = word.chars().count() as f32 * 10.0;
        let with_space = if i == 0 { w } else { line + 10.0 + w };
        if i > 0 && with_space > limit {
            lines += 1.0;
            line = w;
        } else {
            line = with_space;
        }
        widest = widest.max(line);
    }
    (widest, lines * 20.0)
}

fn paint() -> PaintProperties {
    PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0)
}

fn text(content: &str, wrap: bool) -> NodeKind {
    let mut state = TextState {
        content: content.to_string(),
        font_family: "Roboto".to_string(),
        font_weight: 400.0,
        font_size: 22.0,
        align: TextAlign::Start,
        line_height: None,
        options: Default::default(),
    };
    state.options.wrap = wrap;
    NodeKind::Text(state)
}

/// A vertical box of `box_width` x 300 holding one text node; returns the text.
fn scene(
    align: Option<AlignItems>,
    box_width: f32,
    fit: FitContent,
    wrap: bool,
) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    tree.set_text_measure(Box::new(measure));
    let panel = tree.insert(
        NodeKind::Container,
        Style {
            flex_direction: FlexDirection::Column,
            align_items: align,
            padding: taffy::Rect {
                left: length(20.0),
                right: length(20.0),
                top: length(20.0),
                bottom: length(20.0),
            },
            size: Size {
                width: length(box_width),
                height: length(300.0),
            },
            ..Default::default()
        },
        paint(),
    );
    let label = tree.insert(text(LABEL, wrap), Style::default(), paint());
    tree.add_child(panel, label);
    tree.set_fit_content(label, fit);
    tree.compute_layout(
        panel,
        Size {
            width: AvailableSpace::Definite(box_width),
            height: AvailableSpace::Definite(300.0),
        },
    );
    (tree, panel, label)
}

const BOTH: FitContent = FitContent {
    width: true,
    height: true,
};

#[test]
fn fit_content_text_is_one_line_whatever_the_alignment() {
    // 33 characters at 10 px: 330 wide, one 20 px line, in a 480 px box (440
    // inside the padding). The three alignments of #177: centred and flex-start
    // take the text's own width; the default, stretch, widens the item to the
    // whole 440 (CSS's behaviour) -- one line either way.
    for (align, width) in [
        (None, 440.0),
        (Some(AlignItems::CENTER), 330.0),
        (Some(AlignItems::FLEX_START), 330.0),
    ] {
        let (tree, _, label) = scene(align, 480.0, BOTH, true);
        let size = tree.layout(label).size;
        assert_eq!((size.width, size.height), (width, 20.0), "{align:?}");
    }
}

#[test]
fn fit_content_text_wraps_within_the_space_it_is_offered() {
    // A 200 px box leaves 160: the text no longer fits one line, so it takes
    // the offered width and grows taller.
    for align in [Some(AlignItems::CENTER), Some(AlignItems::FLEX_START)] {
        let (tree, _, label) = scene(align, 200.0, BOTH, true);
        let size = tree.layout(label).size;
        assert!(size.width <= 160.0, "{align:?}: {} wide", size.width);
        assert!(size.height > 20.0, "{align:?}: {} tall", size.height);
        // Painted in that width the text takes as many lines as the height says.
        let (_, lines) = measure(
            match &tree.get(label).unwrap().kind {
                NodeKind::Text(state) => state,
                _ => unreachable!(),
            },
            Some(size.width),
        );
        assert_eq!(size.height, lines, "{align:?}");
    }
}

#[test]
fn text_that_does_not_wrap_stays_on_one_line_even_when_it_is_too_wide() {
    let (tree, _, label) = scene(Some(AlignItems::CENTER), 200.0, BOTH, false);
    let size = tree.layout(label).size;
    assert_eq!((size.width, size.height), (330.0, 20.0));
}

#[test]
fn a_side_that_is_not_fit_content_is_the_zero_it_always_was() {
    // Unmarked: the legacy zero by zero under `center`.
    let (tree, _, label) = scene(Some(AlignItems::CENTER), 480.0, FitContent::default(), true);
    let size = tree.layout(label).size;
    assert_eq!((size.width, size.height), (0.0, 0.0));
    // Width only: wide, but no height of its own.
    let width_only = FitContent {
        width: true,
        height: false,
    };
    let (tree, _, label) = scene(Some(AlignItems::CENTER), 480.0, width_only, true);
    let size = tree.layout(label).size;
    assert_eq!((size.width, size.height), (330.0, 0.0));
}

#[test]
fn changing_the_text_lays_a_fit_content_node_out_again() {
    let (mut tree, panel, label) = scene(Some(AlignItems::CENTER), 480.0, BOTH, true);
    assert_eq!(tree.layout(label).size.width, 330.0);
    if let NodeKind::Text(state) = &mut tree.get_mut(label).unwrap().kind {
        state.content = "Short".to_string();
    }
    tree.relayout(label);
    tree.compute_layout(
        panel,
        Size {
            width: AvailableSpace::Definite(480.0),
            height: AvailableSpace::Definite(300.0),
        },
    );
    assert_eq!(tree.layout(label).size.width, 50.0);
}
