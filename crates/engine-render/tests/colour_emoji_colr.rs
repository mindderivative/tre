//! #168: a colour emoji from a COLR font draws in colour. The font is a
//! registered one, so the test never depends on which emoji font a machine
//! has installed. Registration is process-global and cannot be undone, so
//! this is its own test binary (the bitmap case is `colour_emoji_bitmap.rs`).

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{register_font, snapshot};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

/// A small subset of Noto Color Emoji in the COLR format (SIL OFL 1.1): it
/// holds U+2705, U+1F440, U+1F389 and U+1F920.
const NOTO_COLR: &[u8] = include_bytes!("../assets/fonts/NotoColorEmoji-COLR-Subset.ttf");

const W: u32 = 200;
const H: u32 = 50;

fn render(content: &str) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("colour emoji colr"));
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
                width: length(W as f32),
                height: length(H as f32),
            },
            ..Default::default()
        },
        // Blue text: an emoji's own reds and yellows are then not the text's.
        PaintProperties::new(Color::from_rgba8(0x20, 0x20, 0xE0, 0xFF), 0.0, 1.0),
    );
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(W as f32),
            height: AvailableSpace::Definite(H as f32),
        },
    );
    snapshot(&device, &queue, &tree, root, W, H, 1.0, 0.0).unwrap()
}

/// Opaque pixels with a red or green component the blue text colour lacks.
fn off_colour(s: &engine_render::Snapshot) -> usize {
    s.rgba
        .chunks(4)
        .filter(|p| p[3] > 200 && (p[0] > 120 || p[1] > 120))
        .count()
}

#[test]
fn a_colr_emoji_draws_in_colour() {
    // U+1F389, a party popper: in the subset, not in Roboto.
    let popper = "\u{1F389}";
    let before = render(popper);
    assert_eq!(
        off_colour(&before),
        0,
        "no emoji font: the missing-glyph box is in the text colour"
    );

    register_font(NOTO_COLR.to_vec()).expect("a real font");

    let after = render(popper);
    assert!(
        off_colour(&after) > 50,
        "a colour emoji has colours the text colour doesn't: {} pixels",
        off_colour(&after)
    );
}
