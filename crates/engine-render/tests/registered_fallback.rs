//! 0.5.4 (#111): a registered font fills in the glyphs a node's own family
//! lacks, with no system fonts involved. Registration is process-global and
//! cannot be undone, so this is its own test binary.

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{register_font, snapshot};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const HACK: &[u8] = include_bytes!("../assets/fonts/HackNerdFontMono-Regular.ttf");

fn render(content: &str) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("registered fallback"));
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Text(TextState {
            content: content.to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 400.0,
            font_size: 32.0,
            align: TextAlign::Start,
            line_height: None,
            options: Default::default(),
        }),
        Style {
            size: Size {
                width: length(200.0),
                height: length(50.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(255, 255, 255, 255), 0.0, 1.0),
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(50.0),
        },
    );
    snapshot(&device, &queue, &tree, root, 200, 50, 1.0, 0.0).unwrap()
}

#[test]
fn a_registered_font_supplies_glyphs_the_nodes_family_lacks() {
    // U+E0B0, a Powerline arrow: in Hack Nerd Font, not in Roboto.
    let powerline = "\u{e0b0}";
    let latin = render("Hello");
    let before = render(powerline);

    register_font(HACK.to_vec()).expect("a real font");

    let after = render(powerline);
    assert_ne!(
        before, after,
        "the glyph now comes from the registered font"
    );
    assert_eq!(latin, render("Hello"), "text Roboto covers is untouched");
}
