//! 0.5.4 (#111): the process-wide opt-in to the machine's installed fonts.
//!
//! Off, text is hermetic: only the bundled and registered fonts, the same on
//! every machine. On, the glyphs those lack (CJK, Hebrew, colour emoji) come
//! from whatever is installed. The switch is process-global, so everything that
//! flips it lives in one test, in order.

mod support;

use engine_core::{NodeKind, PaintProperties, TextAlign, TextState, Tree};
use engine_render::{set_system_fonts, snapshot, system_fonts};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const W: u32 = 260;
const H: u32 = 60;

fn text_tree(content: &str, family: &str) -> (Tree, engine_core::NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Text(TextState {
            content: content.to_string(),
            font_family: family.to_string(),
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
    (tree, root)
}

fn render(content: &str, family: &str) -> engine_render::Snapshot {
    let (device, queue) = pollster::block_on(support::device("system fonts"));
    let (tree, root) = text_tree(content, family);
    snapshot(&device, &queue, &tree, root, W, H, 1.0, 0.0).unwrap()
}

/// How many pixels carry any ink (alpha above zero).
fn ink(s: &engine_render::Snapshot) -> usize {
    s.rgba.chunks(4).filter(|p| p[3] > 0).count()
}

/// Pixels whose colour is not the text colour's hue: colour glyphs.
fn off_colour(s: &engine_render::Snapshot) -> usize {
    s.rgba
        .chunks(4)
        .filter(|p| p[3] > 200 && (p[0] > 120 || p[1] > 120))
        .count()
}

#[test]
fn system_fonts_are_off_by_default_and_opt_in_adds_the_glyphs_bundled_fonts_lack() {
    assert!(!system_fonts(), "off by default");

    // Off: CJK, Hebrew and an emoji have no bundled font, so they draw as the
    // fallback's missing-glyph box or nothing -- the same on every machine.
    let cjk_off = render("漢字かな", "Roboto");
    let hebrew_off = render("שלום", "Roboto");
    let emoji_off = render("😀", "Roboto");
    let again = render("漢字かな", "Roboto");
    assert_eq!(cjk_off, again, "hermetic: the same twice");

    set_system_fonts(true);
    assert!(system_fonts());
    let cjk_on = render("漢字かな", "Roboto");
    let hebrew_on = render("שלום", "Roboto");
    let emoji_on = render("😀", "Roboto");
    set_system_fonts(false);
    assert!(!system_fonts());
    let cjk_back = render("漢字かな", "Roboto");

    // Turning it off again returns to the hermetic result exactly.
    assert_eq!(cjk_off, cjk_back, "off again is the hermetic frame");

    // What the machine can supply shows up as different pixels. A machine
    // with no CJK font or no emoji font changes nothing, and that is fine: the
    // test then proves only the off half.
    println!(
        "ink off/on: cjk {}/{}, hebrew {}/{}, emoji {}/{}",
        ink(&cjk_off),
        ink(&cjk_on),
        ink(&hebrew_off),
        ink(&hebrew_on),
        ink(&emoji_off),
        ink(&emoji_on)
    );
    if cjk_on == cjk_off {
        eprintln!("no system CJK font here: skipping the fallback assertions");
        return;
    }
    assert!(
        ink(&cjk_on) > ink(&cjk_off),
        "real glyphs have more ink than tofu"
    );
    // A colour emoji is only checked where the machine's emoji font can draw
    // one. A bitmap-only font (CBDT, as most Linux distributions' Noto Color
    // Emoji; sbix on macOS) draws nothing yet (#168): `colour_emoji_colr.rs`
    // checks colour from a registered COLR font on every machine.
    if ink(&emoji_on) > 0 && emoji_on != emoji_off {
        assert!(
            off_colour(&emoji_on) > off_colour(&emoji_off) + 50,
            "a colour emoji has colours the text colour doesn't"
        );
    } else if emoji_on != emoji_off {
        eprintln!("the system's emoji font is bitmap-only: it draws nothing (#168)");
    }
}
