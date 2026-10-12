//! #175: a colour emoji from a bitmap font (CBDT/CBLC, as the Noto Color Emoji
//! package on most Linux distributions, or sbix as on macOS) draws in colour.
//! It used to draw nothing (#168): at the pinned Vello revision glifo hands
//! `vello_gpu` a pixmap for a bitmap glyph, which `vello_gpu` panics on. A
//! bitmap run now always draws through glifo's atlas, which gives it an image
//! `vello_gpu` supports, whatever the glyph-cache switch says.
//!
//! The font is a registered one, so the test never depends on which emoji font
//! a machine has installed. Registration is process-global and cannot be
//! undone, so this is its own test binary (the COLR case is
//! `colour_emoji_colr.rs`), and everything that registers the font is one test,
//! in order.

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextFieldState, TextState, Tree};
use engine_render::{register_font, snapshot_with};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

/// A small subset of Noto Color Emoji in the CBDT/CBLC bitmap format (SIL
/// OFL 1.1): U+2705, U+1F440, U+1F389 and U+1F920.
const NOTO_CBDT: &[u8] = include_bytes!("../assets/fonts/NotoColorEmoji-CBDT-Subset.ttf");

const W: u32 = 200;
const H: u32 = 50;

/// `content` in 32 px Roboto at `scale`, with the process-wide glyph cache
/// `glyph_cache`.
fn render(content: &str, scale: f64, glyph_cache: bool) -> engine_render::Snapshot {
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
    let (w, h) = ((W as f64 * scale) as u32, (H as f64 * scale) as u32);
    snapshot_with(&device, &queue, &tree, root, w, h, scale, 0.0, glyph_cache).unwrap()
}

/// A 32 px text field holding `content`, with the cursor at its end and a
/// selection from `anchor` to there when it is `Some`.
fn render_field(content: &str, anchor: Option<usize>) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("colour emoji bitmap field"));
    let mut tree = Tree::new();
    let mut state = TextFieldState::new(content, "Roboto", 400.0, 32.0);
    state.cursor = content.len();
    state.selection_anchor = anchor;
    let root = tree.insert(
        NodeKind::TextField(Box::new(state)),
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
    snapshot_with(&device, &queue, &tree, root, W, H, 1.0, 0.0, false).unwrap()
}

/// Pixels whose colour is not the text colour's hue: colour glyphs.
fn off_colour(s: &engine_render::Snapshot) -> usize {
    s.rgba
        .chunks(4)
        .filter(|p| p[3] > 200 && (p[0] > 120 || p[1] > 120))
        .count()
}

#[test]
fn a_bitmap_emoji_draws_in_colour_without_panicking() {
    // U+1F389, a party popper: in the subset, not in Roboto.
    let popper = "\u{1F389}";
    let latin = "Hello, world";

    let before = render(popper, 1.0, false);
    assert_eq!(
        off_colour(&before),
        0,
        "no emoji font: the missing-glyph box is in the text colour"
    );
    let latin_before = render(latin, 1.0, false);

    register_font(NOTO_CBDT.to_vec()).expect("a real font");

    // The default: glyph cache off. This is the case that panicked before #175.
    let after = render(popper, 1.0, false);
    assert!(
        off_colour(&after) > 50,
        "a bitmap emoji has colours the text colour doesn't: {} pixels",
        off_colour(&after)
    );

    // With the cache on it is the same atlas path: the same pixels.
    assert_eq!(
        after,
        render(popper, 1.0, true),
        "a bitmap run draws the same with the glyph cache on or off"
    );

    // Scaled: a 2x render is still colour (a larger strike or a scaled one).
    let twice = render(popper, 2.0, false);
    assert!(
        off_colour(&twice) > 4 * 50,
        "a bitmap emoji is still colour at 2x: {} pixels",
        off_colour(&twice)
    );

    // A text field draws its glyphs through its own code (`draw_field`), plain
    // and with a selection (a run split by colour): both must draw the emoji
    // and neither may panic.
    let plain = render_field(popper, None);
    assert!(
        off_colour(&plain) > 50,
        "a bitmap emoji in a text field is colour: {} pixels",
        off_colour(&plain)
    );
    let selected = render_field(popper, Some(0));
    assert!(
        off_colour(&selected) > 50,
        "a bitmap emoji in a selected text field is colour: {} pixels",
        off_colour(&selected)
    );

    // Text that is not an emoji does not change: an outline run still follows
    // the glyph-cache switch (off here), so its pixels are identical.
    assert_eq!(
        latin_before,
        render(latin, 1.0, false),
        "registering a bitmap font leaves outline text alone"
    );
}
