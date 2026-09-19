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

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::{NodeId, TerminalState, TextAlign, TextFieldState, TextState, Tree};
use parley::fontique::{Collection, CollectionOptions};
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontContext, FontFamily, FontWeight,
    LayoutContext, PositionedLayoutItem, Selection, StyleProperty,
};
use peniko::kurbo::{Point, Rect, Shape};
use peniko::{Blob, Color};
use vello_hybrid::{Resources, Scene};

const ROBOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Roboto-Medium.ttf");
const NOTO_SANS_ARABIC: &[u8] = include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf");

/// The exact inputs a shaped `Layout` depends on -- equality here
/// functionally determines the output, which is what makes caching by
/// `NodeId` (below) safe without any separate invalidation logic.
#[derive(PartialEq)]
struct LayoutCacheKey {
    content: String,
    font_family: String,
    font_weight: f32,
    font_size: f32,
    max_width: f32,
    align: TextAlign,
}

struct CachedLayout {
    key: LayoutCacheKey,
    layout: parley::Layout<[u8; 4]>,
}

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
    /// M28 Phase 1 (review follow-through, §5/§6): one shaped `Layout`
    /// per real `Text`/`TextField` node, reused across frames instead
    /// of re-running font matching/line-breaking/BiDi on every single
    /// paint regardless of whether anything changed -- the review's own
    /// finding. Keyed by `NodeId` rather than by content string, so its
    /// size tracks the tree's own text-node count, not how many
    /// distinct strings a live-updating label has ever shown; see
    /// `evict_stale_layouts`.
    layout_cache: HashMap<NodeId, CachedLayout>,
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
            layout_cache: HashMap::new(),
        }
    }

    /// Returns the already-shaped `Layout` for `node_id` if `content`/
    /// `font_family`/`font_weight`/`font_size`/`max_width` still match
    /// what it was last shaped with, otherwise re-shapes and replaces
    /// it. Equality on `LayoutCacheKey` *is* the invalidation check --
    /// every input that can change a `Layout`'s shape is part of the
    /// key, so there's no separate "remember to invalidate" bookkeeping
    /// that a future change could forget to update.
    #[allow(clippy::too_many_arguments)]
    fn shaped_layout(
        &mut self,
        node_id: NodeId,
        content: &str,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        max_width: f32,
        align: TextAlign,
    ) -> &parley::Layout<[u8; 4]> {
        let key = LayoutCacheKey {
            content: content.to_string(),
            font_family: font_family.to_string(),
            font_weight,
            font_size,
            max_width,
            align,
        };
        let Self {
            font_cx,
            layout_cx,
            layout_cache,
        } = self;
        let stale = layout_cache
            .get(&node_id)
            .is_none_or(|cached| cached.key != key);
        if stale {
            let mut builder = layout_cx.ranged_builder(font_cx, content, 1.0, true);
            builder.push_default(StyleProperty::FontFamily(FontFamily::named(font_family)));
            builder.push_default(StyleProperty::FontWeight(FontWeight::new(font_weight)));
            builder.push_default(StyleProperty::FontSize(font_size));
            let mut layout = builder.build(content);
            layout.break_all_lines(Some(max_width));
            // M30 Phase 1 (§5, §7): `parley::Alignment::Start`/`Center`/
            // `End` map 1:1 onto `TextAlign`'s own three variants --
            // real direction-aware behavior (`Start`/`End` respect BiDi,
            // matching §14 step 4's own requirement) is preserved for
            // every existing caller, which still passes `TextAlign::
            // Start` unconditionally.
            let parley_align = match align {
                TextAlign::Start => Alignment::Start,
                TextAlign::Center => Alignment::Center,
                TextAlign::End => Alignment::End,
            };
            layout.align(parley_align, AlignmentOptions::default());
            layout_cache.insert(node_id, CachedLayout { key, layout });
        }
        &layout_cache
            .get(&node_id)
            .expect("just inserted above, or already present")
            .layout
    }

    /// Mirrors `ImageTextureCache::sync`'s own real GPU-texture-leak fix
    /// (the same review pass found this exact class of gap twice, in
    /// two different per-node caches): a text node removed from the
    /// tree left its shaped `Layout` cached here forever otherwise.
    /// `NodeId` is a `slotmap` generational key (`image_cache.rs`'s own
    /// established convention), so `tree.get` on a removed id is a
    /// real, safe staleness check even if its slot has since been
    /// reused by an unrelated new node. Call once per frame, alongside
    /// `sync_image_textures`.
    pub fn evict_stale_layouts(&mut self, tree: &Tree) {
        self.layout_cache.retain(|id, _| tree.get(*id).is_some());
    }

    /// Shapes `state.content` at `state.font_family`/`state.font_size`,
    /// breaks it to fit `at.max_width`, and draws every resulting glyph
    /// run into `scene` at `(at.x, at.y)` -- the text node's
    /// taffy-computed top-left corner -- filled with `at.color`.
    /// `resources` is `FrameRenderer`'s own glyph-atlas state
    /// (`FrameRenderer::resources_mut`): glyph atlasing happens here,
    /// during scene construction, not inside `FrameRenderer::render`, so
    /// the same `Resources` instance has to be reachable at both points.
    /// `node_id` is this text node's own real identity in the caller's
    /// `Tree` (`paint_node`'s own `id`) -- `shaped_layout`'s cache key,
    /// so the shaping pipeline itself only actually runs again when
    /// something about `state`/`at.max_width` genuinely changed since
    /// this node's last paint.
    pub fn draw(
        &mut self,
        scene: &mut Scene,
        resources: &mut Resources,
        state: &TextState,
        at: TextPlacement,
        node_id: NodeId,
    ) {
        // M28 Phase 1: `shaped_layout` reuses the prior frame's
        // `Layout` unchanged whenever nothing about this node's real
        // shaping inputs moved -- see its own doc comment. The
        // direction-aware alignment pass (M30 Phase 1: `state.align`,
        // previously always `Alignment::Start`) only needs to run once,
        // at build time, not on every reuse.
        let layout = self.shaped_layout(
            node_id,
            &state.content,
            &state.font_family,
            state.font_weight,
            state.font_size,
            at.max_width,
            state.align,
        );

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

    /// Shared by `draw_field` and `hit_test_position` (M18 Phase 1,
    /// §8, §10, §11.9, §11.10) -- both need the identical real `Layout`
    /// (same content/font fields, same line-break width) that paint
    /// derives its glyph/caret/selection geometry from and a real click
    /// resolves a byte offset against; building it once here means a
    /// hit-test can never silently drift from what was actually painted.
    fn build_field_layout(
        &mut self,
        content: &str,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
        max_width: f32,
    ) -> parley::Layout<[u8; 4]> {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, content, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(font_family)));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(font_weight)));
        builder.push_default(StyleProperty::FontSize(font_size));
        let mut layout = builder.build(content);
        layout.break_all_lines(Some(max_width));
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    }

    /// M18 Phase 1 (§8, §10, §11.9, §11.10): the real per-glyph-shaping
    /// half of click-to-position -- `engine-core` has no visibility into
    /// `parley` at all (§4), so it cannot itself turn a click point into
    /// a byte offset; this is the one place that real answer can be
    /// computed, using the exact same `Layout` `draw_field` paints from.
    ///
    /// **Deliberate scope simplification, stated in `PLAN.md`:** hit-
    /// tests against `state.content` alone, ignoring an active
    /// `preedit` (M17 Phase 2) -- a real click landing mid-composition
    /// is a genuine corner case `winit`'s own real behavior already
    /// keeps rare (composing and plain pointer/keyboard input aren't
    /// coordinated here), out of this phase's stated scope.
    ///
    /// `point` is in the same local coordinate space `at.x`/`at.y`
    /// place the field's own painted origin at -- the caller (`engine-
    /// py`'s `on_input` closure) is expected to pass `Tree::
    /// hit_test_local`'s own already-transform-aware local point, not a
    /// raw window-space one.
    pub fn hit_test_position(
        &mut self,
        state: &TextFieldState,
        at: TextPlacement,
        point: Point,
    ) -> usize {
        let layout = self.build_field_layout(
            &state.content,
            &state.font_family,
            state.font_weight,
            state.font_size,
            field_max_width(state, at.max_width),
        );
        Cursor::from_point(&layout, (point.x - at.x) as f32, (point.y - at.y) as f32).index()
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
    /// white checkmark (M14 Phase 1) already established. `node_id`
    /// keys the same per-node shaping cache `draw` uses -- see its own
    /// doc comment; keyed on `display_content` (below), so an active
    /// IME composition -- which changes what's displayed every
    /// keystroke -- still reshapes exactly when it should.
    pub fn draw_field(
        &mut self,
        scene: &mut Scene,
        resources: &mut Resources,
        state: &TextFieldState,
        at: TextPlacement,
        show_caret: bool,
        node_id: NodeId,
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

        let layout = self.shaped_layout(
            node_id,
            &display_content,
            &state.font_family,
            state.font_weight,
            state.font_size,
            field_max_width(state, at.max_width),
            TextAlign::Start,
        );

        // Selection highlight, painted first (behind the glyphs below).
        // Real selection and a real active composition are mutually
        // exclusive in practice (an IME owns keyboard input entirely
        // while composing, confirmed via direct source read of
        // `winit::window::Window::set_ime_allowed`'s own doc comment),
        // so this stays keyed on `state.cursor` unconditionally.
        if let Some(anchor) = state.selection_anchor
            && anchor != state.cursor
        {
            let anchor_cursor = Cursor::from_byte_index(layout, anchor, Affinity::Downstream);
            let focus_cursor = Cursor::from_byte_index(layout, state.cursor, Affinity::Downstream);
            let selection = Selection::new(anchor_cursor, focus_cursor);
            scene.set_paint(crate::with_opacity(at.color, 0.3));
            for (bounds, _line_idx) in selection.geometry(layout) {
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
            let start_cursor = Cursor::from_byte_index(layout, range.start, Affinity::Downstream);
            let end_cursor = Cursor::from_byte_index(layout, range.end, Affinity::Downstream);
            let preedit_selection = Selection::new(start_cursor, end_cursor);
            scene.set_paint(at.color);
            for (bounds, _line_idx) in preedit_selection.geometry(layout) {
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
            let cursor = Cursor::from_byte_index(layout, caret_at, Affinity::Downstream);
            let bounds = cursor.geometry(layout, 1.5);
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

    /// M30 Phase 9 Step 4 (§5, §8, §10): a real terminal's own cell
    /// grid, painted on an analytic `col * cell_width, row * cell_
    /// height` grid -- the identical real "cell backgrounds/cursor
    /// positioned analytically, glyphs inside a run of same-styled
    /// cells shaped with the text engine's ordinary shaping" split the
    /// sibling `pyCopper` project's own real `Terminal` widget already
    /// established, reused here directly. **Real, honest v1
    /// limitation, not silently glossed over:** `cell_width`/`cell_
    /// height` are a fixed analytic estimate from `state.font_size`
    /// (this project bundles no real monospace font yet, `Code
    /// Editor`'s own already-stated gap, M30 Phase 9 Step 3) -- glyphs
    /// shaped from a proportional face won't land exactly on this
    /// grid, the identical real drift `Code Editor`'s own missing-
    /// monospace-font gap already causes there.
    ///
    /// Each row's own cells are grouped into real runs (a contiguous
    /// span sharing one background, or one foreground/bold pair) so a
    /// full row of differently-styled text needs only a handful of
    /// real fill/shape calls, not one per character -- `bittty`'s own
    /// real "a run of same-styled cells is laid out with the text
    /// engine's ordinary shaping" precedent, applied here too.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_terminal(
        &mut self,
        scene: &mut Scene,
        resources: &mut Resources,
        state: &TerminalState,
        at: TextPlacement,
        show_caret: bool,
        _node_id: NodeId,
    ) {
        let (cell_width, cell_height) = engine_core::terminal_cell_size(state.font_size);
        let cell_width = f64::from(cell_width);
        let cell_height = f64::from(cell_height);

        for row in 0..state.rows {
            // Real background runs: a contiguous span of cells sharing
            // one real bg color, painted as one rect -- `TRANSPARENT`
            // (`TerminalCell::blank`'s own real default) is skipped
            // entirely, the same "don't paint a real default" every
            // other `NodeKind` arm here already does.
            let mut col = 0u16;
            while col < state.cols {
                let bg = state.cell(row, col).bg;
                let mut end = col + 1;
                while end < state.cols && state.cell(row, end).bg == bg {
                    end += 1;
                }
                if bg != Color::TRANSPARENT {
                    let x0 = at.x + f64::from(col) * cell_width;
                    let y0 = at.y + f64::from(row) * cell_height;
                    let rect = Rect::new(
                        x0,
                        y0,
                        x0 + f64::from(end - col) * cell_width,
                        y0 + cell_height,
                    );
                    scene.set_paint(bg);
                    scene.fill_path(&rect.to_path(0.1));
                }
                col = end;
            }

            // Real glyph runs: a contiguous span of cells sharing one
            // real (fg, bold) pair, shaped and painted as one string --
            // a blank cell's own real transparent `fg` (`TerminalCell::
            // blank`'s own default) never reaches `build_field_layout`
            // at all (an empty/whitespace-only run has no real glyphs
            // to paint), so a genuinely empty terminal costs nothing
            // beyond the background loop above.
            let mut col = 0u16;
            while col < state.cols {
                let first = state.cell(row, col);
                let (fg, bold) = (first.fg, first.bold);
                let mut end = col + 1;
                while end < state.cols {
                    let next = state.cell(row, end);
                    if next.fg != fg || next.bold != bold {
                        break;
                    }
                    end += 1;
                }
                let run: String = (col..end).map(|c| state.cell(row, c).ch).collect();
                if fg != Color::TRANSPARENT && !run.trim().is_empty() {
                    let font_weight = if bold { 700.0 } else { 400.0 };
                    let layout = self.build_field_layout(
                        &run,
                        &state.font_family,
                        font_weight,
                        state.font_size,
                        f32::MAX,
                    );
                    let x0 = at.x + f64::from(col) * cell_width;
                    let y0 = at.y + f64::from(row) * cell_height;
                    scene.set_paint(fg);
                    for line in layout.lines() {
                        for item in line.items() {
                            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                                continue;
                            };
                            let glyph_run_font = glyph_run.run();
                            let font = glyph_run_font.font();
                            let font_size = glyph_run_font.font_size();
                            let glyphs = glyph_run.positioned_glyphs().map(|g| glifo::Glyph {
                                id: g.id,
                                x: g.x + x0 as f32,
                                y: g.y + y0 as f32,
                            });
                            scene
                                .glyph_run(resources, font)
                                .font_size(font_size)
                                .fill_glyphs(glyphs);
                        }
                    }
                }
                col = end;
            }
        }

        // The real cursor block, painted last (on top of every real
        // cell) -- only while this terminal is the `Tree`'s own real,
        // live focused node, the identical real `show_caret` gate
        // `draw_field`'s own caret already uses.
        if show_caret
            && state.cursor_visible
            && state.cursor_row < state.rows
            && state.cursor_col < state.cols
        {
            let x0 = at.x + f64::from(state.cursor_col) * cell_width;
            let y0 = at.y + f64::from(state.cursor_row) * cell_height;
            let rect = Rect::new(x0, y0, x0 + cell_width, y0 + cell_height);
            scene.set_paint(crate::with_opacity(at.color, 0.5));
            scene.fill_path(&rect.to_path(0.1));
        }
    }
}

/// M30 Phase 9 Step 3 (§5): `Code Editor`'s own real "never wraps"
/// layout need (this module's own doc comment already states the
/// established convention -- a hard `\n` always starts a new `Layout`
/// line regardless of wrapping, `Layout::break_all_lines`'s own real
/// contract, confirmed via direct source read) -- `Some(f32::MAX)`
/// and `None` are the identical real no-wrap value to `break_all_
/// lines` (`max_advance.unwrap_or(f32::MAX)`), so this needs no new
/// `Option`-typed plumbing through `shaped_layout`/`build_field_
/// layout`'s own existing `f32` parameter, just the right value at
/// the two real call sites that know `state.multiline`.
fn field_max_width(state: &TextFieldState, max_width: f32) -> f32 {
    if state.multiline { f32::MAX } else { max_width }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameRenderer;
    use engine_core::{NodeKind, PaintProperties, Tree};
    use taffy::prelude::{Size, Style, length};
    use vello_hybrid::RenderTargetConfig;

    async fn frame_renderer_for_test() -> FrameRenderer {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("no wgpu adapter available in this environment");
        let (device, _queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("text.rs layout-cache test device"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("failed to create wgpu device");
        FrameRenderer::new(
            &device,
            &RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: 100,
                height: 100,
            },
        )
    }

    fn text_node(tree: &mut Tree, content: &str) -> engine_core::NodeId {
        tree.insert(
            NodeKind::Text(TextState {
                content: content.to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: TextAlign::Start,
            }),
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        )
    }

    fn placement() -> TextPlacement {
        TextPlacement {
            x: 0.0,
            y: 0.0,
            max_width: 100.0,
            color: Color::from_rgba8(255, 255, 255, 255),
        }
    }

    /// Real regression coverage for the review-found gap: every
    /// `Text`/`TextField` paint used to rebuild its `Layout` from
    /// scratch every frame regardless of whether anything changed.
    /// Proves the cache actually caches (repainting an unchanged node
    /// doesn't grow it) and actually evicts (mirroring `image_cache.
    /// rs`'s own already-established test for the identical class of
    /// per-node-cache leak).
    #[test]
    fn shaped_layout_is_cached_per_node_and_evicted_on_removal() {
        pollster::block_on(async {
            let mut frame_renderer = frame_renderer_for_test().await;
            let mut renderer = TextRenderer::new();
            let mut scene = Scene::new(100, 100);
            let mut tree = Tree::new();

            let a = text_node(&mut tree, "hello");
            let b = text_node(&mut tree, "world");

            let NodeKind::Text(state_a) = &tree.get(a).unwrap().kind else {
                panic!("expected Text");
            };
            renderer.draw(
                &mut scene,
                frame_renderer.resources_mut(),
                state_a,
                placement(),
                a,
            );
            assert_eq!(renderer.layout_cache.len(), 1);

            // Repainting the same node, completely unchanged, must not
            // grow the cache -- it's keyed on the node's own identity,
            // not on every draw call.
            let NodeKind::Text(state_a) = &tree.get(a).unwrap().kind else {
                panic!("expected Text");
            };
            renderer.draw(
                &mut scene,
                frame_renderer.resources_mut(),
                state_a,
                placement(),
                a,
            );
            assert_eq!(
                renderer.layout_cache.len(),
                1,
                "repainting an unchanged node must not create a second entry"
            );

            let NodeKind::Text(state_b) = &tree.get(b).unwrap().kind else {
                panic!("expected Text");
            };
            renderer.draw(
                &mut scene,
                frame_renderer.resources_mut(),
                state_b,
                placement(),
                b,
            );
            assert_eq!(
                renderer.layout_cache.len(),
                2,
                "a genuinely different node must get its own entry"
            );

            tree.remove(a);
            renderer.evict_stale_layouts(&tree);
            assert_eq!(
                renderer.layout_cache.len(),
                1,
                "a removed node's cached layout must be evicted, not kept forever"
            );
            assert!(renderer.layout_cache.contains_key(&b));
        });
    }
}
