//! #168: a bitmap-only emoji font (CBDT/CBLC, as the Noto Color Emoji package
//! on most Linux distributions, or sbix as on macOS) draws nothing yet: the
//! pinned `vello_gpu` cannot draw a glyph that is a picture. This pins that
//! down so it is a known limit, not a silent surprise: no panic, no colour.
//! When it starts to draw, this test fails: update the colour-emoji section
//! of `docs/guide/text.md` and turn this into a colour assertion.
//!
//! Enabling glifo's `png` feature does not help at this revision: the glyph
//! is then drawn as a pixmap and `vello_gpu` panics ("pixmap image sources
//! are not supported by Vello GPU"). Registration is process-global, so this
//! is its own test binary (the COLR case is `colour_emoji_colr.rs`).

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{register_font, snapshot};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

/// A small subset of Noto Color Emoji in the CBDT/CBLC bitmap format (SIL
/// OFL 1.1): U+2705, U+1F440, U+1F389 and U+1F920.
const NOTO_CBDT: &[u8] = include_bytes!("../assets/fonts/NotoColorEmoji-CBDT-Subset.ttf");

const W: u32 = 200;
const H: u32 = 50;

fn render(content: &str) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("colour emoji bitmap"));
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

fn off_colour(s: &engine_render::Snapshot) -> usize {
    s.rgba
        .chunks(4)
        .filter(|p| p[3] > 200 && (p[0] > 120 || p[1] > 120))
        .count()
}

#[test]
fn a_bitmap_only_emoji_font_draws_no_colour_and_does_not_panic() {
    let popper = "\u{1F389}";
    register_font(NOTO_CBDT.to_vec()).expect("a real font");

    let drawn = render(popper);
    assert_eq!(
        off_colour(&drawn),
        0,
        "known limit (#168): a bitmap emoji draws no colour; if this fails, bitmap \
         glyphs now draw -- update the docs and assert the colour instead"
    );
}
