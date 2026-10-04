//! 0.5.4 (#153): a text node's accessibility runs follow its shaped lines:
//! one run per line, with each character's position and width, read back
//! through AccessKit's own consumer.

mod support;

use std::collections::HashMap;

use engine_core::{
    NodeId, NodeKind, PaintProperties, TextAlign, TextOptions, TextSpan, TextState, Tree,
    to_access_id,
};
use engine_render::TextRenderer;
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const CONTENT: &str = "The quick brown fox jumps over the lazy dog";

fn tree(width: f32, spans: Vec<TextSpan>) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let id = tree.insert(
        NodeKind::Text(TextState {
            content: CONTENT.to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 400.0,
            font_size: 20.0,
            align: TextAlign::Start,
            line_height: None,
            options: TextOptions {
                selectable: true,
                spans,
                ..Default::default()
            },
        }),
        Style {
            size: Size {
                width: length(width),
                height: length(200.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
    );
    tree.compute_layout(
        id,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(200.0),
        },
    );
    (tree, id)
}

fn lines_of(tree: &Tree, id: NodeId, width: f32) -> Vec<engine_core::AccessLine> {
    let NodeKind::Text(state) = &tree.get(id).unwrap().kind else {
        unreachable!()
    };
    let at = engine_render::TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: width,
        color: Color::TRANSPARENT,
    };
    TextRenderer::new().access_lines(id, state, at)
}

#[test]
fn a_wrapped_paragraph_has_a_line_each_covering_its_text_with_a_position_per_character() {
    let (tree, id) = tree(150.0, vec![]);
    let lines = lines_of(&tree, id, 150.0);
    assert!(lines.len() >= 3, "wrapped to {} lines", lines.len());
    // The lines tile the content.
    assert_eq!(lines[0].start, 0);
    for pair in lines.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "no gap between lines");
        assert!(
            pair[1].y0 >= pair[0].y1 - 2.0,
            "lines run down the page: {:?} then {:?}",
            (pair[0].y0, pair[0].y1),
            (pair[1].y0, pair[1].y1)
        );
    }
    assert_eq!(lines.last().unwrap().end, CONTENT.len());
    for line in &lines {
        assert_eq!(
            line.chars.len(),
            CONTENT[line.start..line.end].chars().count()
        );
        assert!(
            line.chars.windows(2).all(|c| c[1].0 >= c[0].0),
            "left to right"
        );
        assert!(
            line.chars.iter().any(|c| c.1 > 1.0),
            "characters have width"
        );
        assert!(line.x1 > line.x0 && line.y1 > line.y0);
    }
}

#[test]
fn the_consumer_sees_a_run_per_line_and_selects_across_them() {
    let (mut tree, id) = tree(150.0, vec![]);
    tree.set_text_access_lines(HashMap::from([(id, lines_of(&tree, id, 150.0))]));
    let lines = lines_of(&tree, id, 150.0);
    // Select from inside the first line to inside the last.
    tree.set_text_selection(id, 4, lines.last().unwrap().start + 3);
    let update = tree.build_access_update(id);
    let runs = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::TextRun)
        .count();
    assert_eq!(runs, lines.len());
    let (_, run) = update
        .nodes
        .iter()
        .find(|(_, n)| n.role() == accesskit::Role::TextRun)
        .unwrap();
    assert_eq!(
        run.character_positions().map(<[f32]>::len),
        Some(run.character_lengths().len())
    );
    assert_eq!(
        run.character_widths().map(<[f32]>::len),
        Some(run.character_lengths().len())
    );
    assert_eq!(
        run.character_positions().unwrap()[0],
        0.0,
        "positions are within the run"
    );
    let consumer = accesskit_consumer::Tree::new(update, true);
    let root = consumer.state().root();
    assert_eq!(
        root.document_range().text().replace('\n', " ").trim_end(),
        CONTENT
    );
    let selected = root.text_selection().expect("selection").text();
    let want = &CONTENT[4..lines.last().unwrap().start + 3];
    assert_eq!(selected.replace('\n', ""), want.replace('\n', ""));
}

#[test]
fn a_link_across_a_line_break_is_one_link_of_two_runs() {
    let start = CONTENT.find("brown").unwrap();
    let end = CONTENT.find("lazy").unwrap();
    let (mut tree, id) = tree(
        150.0,
        vec![TextSpan {
            start,
            end,
            link: Some("https://example.com".to_string()),
            ..Default::default()
        }],
    );
    tree.set_text_access_lines(HashMap::from([(id, lines_of(&tree, id, 150.0))]));
    let update = tree.build_access_update(id);
    let links: Vec<_> = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::Link)
        .collect();
    assert_eq!(links.len(), 1);
    let link = links[0].1.clone();
    assert!(
        link.children().len() >= 2,
        "the link spans lines: {}",
        link.children().len()
    );
    assert_eq!(link.url(), Some("https://example.com"));
    // A press on the link's second run resolves to the link's text on that line.
    let second = link.children()[1];
    assert!(matches!(
        tree.resolve_text_part(second),
        Some(engine_core::TextPart::Run { owner, .. }) if owner == id
    ));
    let bounds = link.bounds().unwrap();
    assert!(
        bounds.y1 - bounds.y0 > 20.0,
        "the link's box covers both lines"
    );
    // The container holds the link where it is, between plain runs.
    let container = update
        .nodes
        .iter()
        .find(|(i, _)| *i == to_access_id(id))
        .unwrap();
    assert!(container.1.children().contains(&links[0].0));
}

#[test]
fn text_with_no_lines_given_keeps_the_one_run_shape() {
    let (tree, id) = tree(150.0, vec![]);
    let update = tree.build_access_update(id);
    let runs = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::TextRun)
        .count();
    assert_eq!(runs, 1);
}

#[test]
fn stale_lines_that_no_longer_fit_the_text_are_ignored() {
    let (mut tree, id) = tree(150.0, vec![]);
    let lines = lines_of(&tree, id, 150.0);
    tree.set_text_access_lines(HashMap::from([(id, lines)]));
    if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
        state.content = "short".to_string();
    }
    let update = tree.build_access_update(id);
    let runs: Vec<_> = update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::TextRun)
        .collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].1.value(), Some("short"));
}
