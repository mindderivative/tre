//! `parley`-based text shaping, feeding `vello_hybrid`'s low-level glyph
//! API (§14 step 4, the typography spike).
//!
//! `parley` shapes text (line-breaking, BiDi, font-fallback) into a
//! `Layout` of positioned glyph runs; `vello_hybrid` only knows how to
//! draw an already-positioned run of glyph ids (`Scene::glyph_run`,
//! backed by `glifo::Glyph { id, x, y }`) -- this module is the glue
//! between the two, which is exactly what §14 step 4 exists to spike:
//! "get real signal on parley's current line-breaking/BiDi/font-fallback
//! behavior before component work depends on it."

use std::sync::Arc;

use engine_core::{TextFieldState, TextState};
use parley::fontique::{Collection, CollectionOptions};
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontContext, FontFamily, FontWeight,
    LayoutContext, PositionedLayoutItem, Selection, StyleProperty,
};
use peniko::kurbo::{Rect, Shape};
use peniko::{Blob, Color};
use vello_hybrid::{Resources, Scene};

const ROBOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Roboto-Medium.ttf");
const NOTO_SANS_ARABIC: &[u8] = include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf");

/// Owns `parley`'s font/layout state across frames -- font discovery and
/// registration are real, one-time costs that shouldn't repeat every
/// frame the way shaping itself reasonably can, so this is built once
/// and threaded through every `build_tree_scene` call. Same "bundle the
/// long-lived state so callers see one object" reasoning `FrameRenderer`
/// already applies to `Renderer` + `Resources`.
///
/// System font discovery is deliberately OFF
/// (`CollectionOptions { system_fonts: false, .. }`): this project's own
/// vendored fonts (`assets/fonts/README.md`) are registered explicitly
/// instead, so text rendering is hermetic and doesn't depend on whatever
/// happens to be installed on the machine running `cargo test` -- the
/// same headless-CI-safe discipline this codebase already applies to
/// GPU/display absence (TRE v1 finding #261).
pub struct TextRenderer {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextRenderer {
    pub fn new() -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        for bytes in [ROBOTO_REGULAR, ROBOTO_MEDIUM, NOTO_SANS_ARABIC] {
            collection.register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
        }
        Self {
            font_cx: FontContext {
                collection,
                source_cache: Default::default(),
            },
            layout_cx: LayoutContext::new(),
        }
    }

    /// Shapes `state.content` at `state.font_family`/`state.font_size`,
    /// breaks it to fit `at.max_width`, and draws every resulting glyph
    /// run into `scene` at `(at.x, at.y)` -- the text node's
    /// taffy-computed top-left corner -- filled with `at.color`.
    /// `resources` is `FrameRenderer`'s own glyph-atlas state
    /// (`FrameRenderer::resources_mut`): glyph atlasing happens here,
    /// during scene construction, not inside `FrameRenderer::render`, so
    /// the same `Resources` instance has to be reachable at both points.
    pub fn draw(
        &mut self,
        scene: &mut Scene,
        resources: &mut Resources,
        state: &TextState,
        at: TextPlacement,
    ) {
        let mut builder =
            self.layout_cx
                .ranged_builder(&mut self.font_cx, &state.content, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(
            &state.font_family,
        )));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(
            state.font_weight,
        )));
        builder.push_default(StyleProperty::FontSize(state.font_size));
        let mut layout = builder.build(&state.content);
        layout.break_all_lines(Some(at.max_width));
        // `break_all_lines`'s glyph offsets alone don't account for
        // paragraph direction -- an RTL run still starts near x=0 until
        // an explicit alignment pass runs. `Alignment::Start` is
        // direction-aware (left for LTR, right for RTL), matching what
        // §14 step 4 actually needs to prove about `parley`'s BiDi
        // handling: not just that RTL glyphs shape in the right visual
        // order, but that the paragraph as a whole sits against the
        // correct edge of its box.
        layout.align(Alignment::Start, AlignmentOptions::default());

        scene.set_paint(at.color);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let glyphs = glyph_run.positioned_glyphs().map(|g| glifo::Glyph {
                    id: g.id,
                    x: g.x + at.x as f32,
                    y: g.y + at.y as f32,
                });
                scene
                    .glyph_run(resources, font)
                    .font_size(font_size)
                    .fill_glyphs(glyphs);
            }
        }
    }

    /// M15 Phase 1 (§5, §16.7): `draw`'s own real editable-field
    /// sibling -- builds the identical kind of `Layout` `draw` does
    /// (same content/font fields, `TextFieldState` mirrors `TextState`
    /// exactly for this reason), but from that *one* `Layout` also
    /// derives real caret/selection-highlight paint geometry via
    /// `parley::{Cursor, Selection}` -- not a second, disconnected
    /// measurement, so the geometry always lines up with the glyphs
    /// actually painted. Real MD3 stacking order: selection highlight
    /// first (behind text), then glyphs, then the caret last (on top)
    /// -- the same order any real text editor paints these three
    /// layers in. Neither the caret nor the highlight is yet a real,
    /// theme-resolved MD3 color (`engine-render` doesn't depend on
    /// `engine-md3`, §4) -- both derive from `at.color`, the same
    /// "real but not yet theme-aware" scope `Checkbox`'s own hardcoded
    /// white checkmark (M14 Phase 1) already established.
    pub fn draw_field(
        &mut self,
        scene: &mut Scene,
        resources: &mut Resources,
        state: &TextFieldState,
        at: TextPlacement,
        show_caret: bool,
    ) {
        // M17 Phase 2 (§8): a real, in-progress IME composition is
        // spliced into the *displayed* text at `cursor` -- purely for
        // painting, `state.content` itself stays uncommitted the whole
        // time (`TextFieldState.preedit`'s own doc comment). Real IME
        // caret behavior: while composing, the caret sits at the end of
        // the in-progress composition, not at the real, frozen `cursor`
        // underneath it.
        let (display_content, preedit_range, caret_at) = match &state.preedit {
            Some(preedit) if !preedit.is_empty() => {
                let mut combined = state.content.clone();
                combined.insert_str(state.cursor, preedit);
                let end = state.cursor + preedit.len();
                (combined, Some(state.cursor..end), end)
            }
            _ => (state.content.clone(), None, state.cursor),
        };

        let mut builder =
            self.layout_cx
                .ranged_builder(&mut self.font_cx, &display_content, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(
            &state.font_family,
        )));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(
            state.font_weight,
        )));
        builder.push_default(StyleProperty::FontSize(state.font_size));
        let mut layout = builder.build(&display_content);
        layout.break_all_lines(Some(at.max_width));
        layout.align(Alignment::Start, AlignmentOptions::default());

        // Selection highlight, painted first (behind the glyphs below).
        // Real selection and a real active composition are mutually
        // exclusive in practice (an IME owns keyboard input entirely
        // while composing, confirmed via direct source read of
        // `winit::window::Window::set_ime_allowed`'s own doc comment),
        // so this stays keyed on `state.cursor` unconditionally.
        if let Some(anchor) = state.selection_anchor
            && anchor != state.cursor
        {
            let anchor_cursor = Cursor::from_byte_index(&layout, anchor, Affinity::Downstream);
            let focus_cursor = Cursor::from_byte_index(&layout, state.cursor, Affinity::Downstream);
            let selection = Selection::new(anchor_cursor, focus_cursor);
            scene.set_paint(crate::with_opacity(at.color, 0.3));
            for (bounds, _line_idx) in selection.geometry(&layout) {
                let rect = Rect::new(
                    bounds.x0 + at.x,
                    bounds.y0 + at.y,
                    bounds.x1 + at.x,
                    bounds.y1 + at.y,
                );
                scene.fill_path(&rect.to_path(0.1));
            }
        }

        scene.set_paint(at.color);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let glyphs = glyph_run.positioned_glyphs().map(|g| glifo::Glyph {
                    id: g.id,
                    x: g.x + at.x as f32,
                    y: g.y + at.y as f32,
                });
                scene
                    .glyph_run(resources, font)
                    .font_size(font_size)
                    .fill_glyphs(glyphs);
            }
        }

        // M17 Phase 2 (§8): a real preedit underline, painted on top of
        // the glyphs above (a composition preview is visually "live" the
        // same way a caret is) -- reuses the identical `Selection::
        // geometry` mechanism the selection highlight above already
        // does, since a `[start, end)` byte range's own real geometry
        // is the same shape either way; only a thin line at each rect's
        // own bottom edge, not a fill.
        if let Some(range) = preedit_range {
            let start_cursor = Cursor::from_byte_index(&layout, range.start, Affinity::Downstream);
            let end_cursor = Cursor::from_byte_index(&layout, range.end, Affinity::Downstream);
            let preedit_selection = Selection::new(start_cursor, end_cursor);
            scene.set_paint(at.color);
            for (bounds, _line_idx) in preedit_selection.geometry(&layout) {
                let underline = Rect::new(
                    bounds.x0 + at.x,
                    bounds.y1 + at.y - 1.0,
                    bounds.x1 + at.x,
                    bounds.y1 + at.y,
                );
                scene.fill_path(&underline.to_path(0.1));
            }
        }

        // Caret, painted last (on top of everything above) -- only when
        // this field is the `Tree`'s own real, live focused node
        // (`paint_node`'s own real caller decides `show_caret`).
        // `caret_at` is the real, in-progress composition's own end
        // while a preedit is active, `state.cursor` otherwise.
        if show_caret {
            let cursor = Cursor::from_byte_index(&layout, caret_at, Affinity::Downstream);
            let bounds = cursor.geometry(&layout, 1.5);
            let rect = Rect::new(
                bounds.x0 + at.x,
                bounds.y0 + at.y,
                bounds.x1 + at.x,
                bounds.y1 + at.y,
            );
            scene.set_paint(at.color);
            scene.fill_path(&rect.to_path(0.1));
        }
    }
}

/// Where and how wide to draw one text node -- bundled so
/// `TextRenderer::draw` stays under clippy's argument-count lint without
/// losing any of these genuinely-distinct-per-call values.
pub struct TextPlacement {
    /// Node-local top-left corner, under whatever transform the caller's
    /// `Scene` currently has set (M5 Phase 1, §11.9) -- `paint_node`
    /// always passes `(0.0, 0.0)` today (a text node paints at its own
    /// origin), kept as real fields rather than hardcoded so a future
    /// caller with genuine local padding/inset doesn't need a shape
    /// change here.
    pub x: f64,
    pub y: f64,
    /// The node's taffy-computed box width -- what `parley` wraps to.
    pub max_width: f32,
    pub color: Color,
}
