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
use std::ops::Range;
use std::sync::Arc;

use engine_core::{
    NodeId, NodeKind, TerminalCell, TerminalState, TextAlign, TextFieldState, TextState, Tree,
};
use parley::fontique::{Collection, CollectionOptions};
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontContext, FontFamily, FontWeight,
    LayoutContext, PositionedLayoutItem, Selection, StyleProperty,
};
use peniko::kurbo::{Affine, Point, Rect, Shape};
use peniko::{Blob, Color};
use vello_hybrid::{Resources, Scene};

const ROBOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Roboto-Medium.ttf");
const NOTO_SANS_ARABIC: &[u8] = include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf");
/// M32 Phase 1 (§5, §8, §10): the real bundled monospace face --
/// `assets/fonts/README.md` has the full real attribution. Real family
/// name confirmed by direct read of the font's own `name` table (Python
/// `fontTools.ttLib.TTFont(...)['name']`, nameID 1), not assumed from
/// the filename: `"Hack Nerd Font Mono"` (below).
const HACK_NERD_FONT_MONO: &[u8] = include_bytes!("../assets/fonts/HackNerdFontMono-Regular.ttf");
/// The real family name `Hack Nerd Font Mono`'s own `name` table
/// reports -- what every `FontFamily::named(...)` call below must pass
/// to actually resolve to this bundled face.
pub const MONOSPACE_FONT_FAMILY: &str = "Hack Nerd Font Mono";

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
    /// M62 Phase 1 (§7.1, §16.3): part of the real cache-invalidation
    /// key for the identical reason every other shaping input already
    /// is -- a changed `StyleProperty::LineHeight` push changes the
    /// real glyph-run vertical advance `Layout::lines()` reports.
    line_height: Option<f32>,
    /// M31 Phase 4 (§5, §8): real per-byte-range syntax coloring, part
    /// of the real cache-invalidation key for the identical reason
    /// every other shaping input already is -- a different real
    /// `StyleProperty::Brush` push changes what `Glyph::style_index`
    /// (and, through it, `Layout::styles()[..].brush`) each real glyph
    /// resolves to, which `draw_field`'s own paint loop reads directly
    /// (`shaped_layout`'s own real build below).
    spans: Vec<(Range<usize>, Color)>,
    /// M31 Phase 4 (§5, §8): the real default brush every glyph
    /// outside every real span resolves to -- without pushing this
    /// explicitly, an un-spanned glyph's own real `style_index` would
    /// point at `Style::default()`'s own `[u8; 4]::default()` brush
    /// (`[0, 0, 0, 0]`, fully transparent), not `at.color`. Part of
    /// the cache key since a real theme change changes it.
    default_color: Color,
}

struct CachedLayout {
    key: LayoutCacheKey,
    layout: parley::Layout<[u8; 4]>,
}

/// M64 (§8, §11.3): `draw_terminal`'s own per-run shaping cache key --
/// the real shaping inputs only (`content`/`font_family`/`font_weight`/
/// `font_size`), the identical subset `build_field_layout` itself takes
/// (no `align`/`spans`/`default_color`: `build_field_layout` always
/// aligns `Start` and never pushes per-span brushes, and a run's own
/// foreground color/italic-shear/underline are applied at paint time,
/// not baked into shaping -- see `draw_terminal`'s own real call site).
#[derive(PartialEq)]
struct TerminalRunKey {
    content: String,
    font_family: String,
    font_weight: f32,
    font_size: f32,
}

struct CachedTerminalRun {
    key: TerminalRunKey,
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
    /// M32 Phase 1 (§5, §8, §10): real per-`(font_family, font_size)`
    /// monospace cell metrics, memoized -- see `monospace_cell_size`.
    /// Unbounded like `layout_cache` was before `evict_stale_layouts`
    /// existed, but real, honest, and low-risk here: keyed by a font
    /// size in bits, not a `NodeId`, so its size tracks how many
    /// distinct `(family, size)` combinations an app has ever actually
    /// used -- a handful in practice (an app doesn't animate its own
    /// terminal's font size every frame), never one entry per node.
    monospace_cell_cache: HashMap<(String, u32), (f32, f32)>,
    /// M64 (§8, §11.3): `draw_terminal`'s own real per-run shaping
    /// cache -- terminal rows/runs have no stable `NodeId` of their
    /// own (only the whole terminal widget does), so this is keyed by
    /// a real compound `(NodeId, row, col)` tuple instead of the bare
    /// `NodeId` `layout_cache` uses. `col` is the run's own starting
    /// column -- a stable real slot for as long as the terminal's own
    /// content at that position hasn't scrolled/changed, not a
    /// synthetic per-row run index that would shift every later run on
    /// the same row whenever an earlier one changed shape. See
    /// `evict_stale_layouts` for real eviction (both "the terminal
    /// node itself was removed" and "this row/col fell outside the
    /// terminal's own current bounds after a resize shrunk it").
    terminal_run_cache: HashMap<(NodeId, u16, u16), CachedTerminalRun>,
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
        for bytes in [
            ROBOTO_REGULAR,
            ROBOTO_MEDIUM,
            NOTO_SANS_ARABIC,
            HACK_NERD_FONT_MONO,
        ] {
            collection.register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
        }
        Self {
            font_cx: FontContext {
                collection,
                source_cache: Default::default(),
            },
            layout_cx: LayoutContext::new(),
            layout_cache: HashMap::new(),
            monospace_cell_cache: HashMap::new(),
            terminal_run_cache: HashMap::new(),
        }
    }

    /// M32 Phase 1 (§5, §8, §10): the real per-font-size monospace cell
    /// size `draw_terminal` positions every cell on, replacing the old
    /// `engine_core::terminal_cell_size` analytic estimate (`font_size *
    /// 0.6`/`* 1.3`) now that a real bundled monospace face exists to
    /// measure. Shapes a single `"M"` glyph through the exact same
    /// `build_field_layout` every other real text path in this module
    /// already uses (zero new shaping logic) and reads back its real
    /// `Layout::width()`/`Layout::height()` -- for a genuinely monospace
    /// face every glyph shares one real advance width, so measuring any
    /// single glyph gives the exact real per-cell width, and shaping a
    /// single line gives the exact real per-cell height (ascent +
    /// descent + line gap, the same real metrics a terminal emulator's
    /// own cell grid is built from). Cached by `(font_family, font_size)`
    /// -- `draw_terminal` calls this every frame, and re-shaping `"M"`
    /// on every single frame for a value that only changes when the app
    /// changes `font_size`/`font_family` would be real, avoidable work,
    /// the identical "cache what a frame doesn't need to redo" reasoning
    /// `layout_cache` above already established.
    pub fn monospace_cell_size(&mut self, font_family: &str, font_size: f32) -> (f32, f32) {
        let key = (font_family.to_string(), font_size.to_bits());
        if let Some(&size) = self.monospace_cell_cache.get(&key) {
            return size;
        }
        let layout = self.build_field_layout("M", font_family, 400.0, font_size, f32::MAX);
        let size = (layout.width(), layout.height());
        self.monospace_cell_cache.insert(key, size);
        size
    }

    /// Returns the already-shaped `Layout` for `node_id` if `content`/
    /// `font_family`/`font_weight`/`font_size`/`max_width` still match
    /// what it was last shaped with, otherwise re-shapes and replaces
    /// it. Comparing against the cached key's own fields (below) *is*
    /// the invalidation check -- every input that can change a
    /// `Layout`'s shape is part of the comparison, so there's no
    /// separate "remember to invalidate" bookkeeping that a future
    /// change could forget to update. M66 (§8): the comparison itself
    /// is against the *borrowed* new inputs -- no owned `LayoutCacheKey`
    /// is ever allocated on the cache-hit path (the common case: an
    /// unchanged node, reshaped every paint before this), only inside
    /// the real stale/miss branch where a reshape is happening anyway.
    /// Before this, a full `content`/`spans` clone was paid on *every*
    /// call regardless, just to build a key that got thrown away the
    /// instant the comparison confirmed nothing changed -- found and
    /// adversarially verified by a `/review-project` pass.
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
        line_height: Option<f32>,
        spans: &[(Range<usize>, Color)],
        default_color: Color,
    ) -> &parley::Layout<[u8; 4]> {
        let stale = self.layout_cache.get(&node_id).is_none_or(|cached| {
            cached.key.content != content
                || cached.key.font_family != font_family
                || cached.key.font_weight != font_weight
                || cached.key.font_size != font_size
                || cached.key.max_width != max_width
                || cached.key.align != align
                || cached.key.line_height != line_height
                || cached.key.spans != spans
                || cached.key.default_color != default_color
        });
        let Self {
            font_cx,
            layout_cx,
            layout_cache,
            monospace_cell_cache: _,
            terminal_run_cache: _,
        } = self;
        if stale {
            let mut builder = layout_cx.ranged_builder(font_cx, content, 1.0, true);
            builder.push_default(StyleProperty::FontFamily(FontFamily::named(font_family)));
            builder.push_default(StyleProperty::FontWeight(FontWeight::new(font_weight)));
            builder.push_default(StyleProperty::FontSize(font_size));
            // M62 Phase 1 (§7.1, §16.3): `None` deliberately pushes
            // nothing at all here, rather than an explicit `parley::
            // LineHeight::MetricsRelative(1.0)` -- `parley`'s own real
            // default (verified via direct source read) is already
            // exactly that, so an un-set `TextState.line_height` must
            // produce byte-for-byte the same `Layout` as before this
            // field existed, not merely an equivalent one.
            if let Some(ratio) = line_height {
                builder.push_default(StyleProperty::LineHeight(
                    parley::LineHeight::FontSizeRelative(ratio),
                ));
            }
            // M31 Phase 4 (§5, §8): a real default brush covering the
            // *whole* content, then a real per-range override for each
            // real syntax span -- `draw_field`'s own paint loop reads
            // each individual glyph's own real, resolved brush back via
            // `Glyph::style_index`/`Layout::styles()` (confirmed real,
            // public API via direct source read: `parley::Cluster::
            // first_style` reads the identical way), so every glyph
            // needs a real, meaningful brush value, not just the ones
            // inside a real span.
            let default_rgba = default_color.to_rgba8();
            builder.push_default(StyleProperty::Brush([
                default_rgba.r,
                default_rgba.g,
                default_rgba.b,
                default_rgba.a,
            ]));
            for (range, color) in spans {
                let rgba = color.to_rgba8();
                builder.push(
                    StyleProperty::Brush([rgba.r, rgba.g, rgba.b, rgba.a]),
                    range.clone(),
                );
            }
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
            layout_cache.insert(
                node_id,
                CachedLayout {
                    key: LayoutCacheKey {
                        content: content.to_string(),
                        font_family: font_family.to_string(),
                        font_weight,
                        font_size,
                        max_width,
                        align,
                        line_height,
                        spans: spans.to_vec(),
                        default_color,
                    },
                    layout,
                },
            );
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
        // M64 (§8, §11.3): the terminal run cache's own real eviction
        // -- drop every entry whose terminal node no longer exists at
        // all (the identical "NodeId removed" case `layout_cache`
        // above already handles), plus every entry whose own `(row,
        // col)` has fallen outside the terminal's own current real
        // `rows`/`cols` bounds after a resize shrunk it. `layout_cache`
        // has no equivalent second case -- a plain `Text`/`TextField`
        // node has no internal sub-grid of its own that can shrink out
        // from under a cached sub-key the way a terminal's rows/cols
        // can.
        self.terminal_run_cache.retain(|(id, row, col), _| {
            let Some(node) = tree.get(*id) else {
                return false;
            };
            let NodeKind::Terminal(state) = &node.kind else {
                return false;
            };
            *row < state.rows && *col < state.cols
        });
    }

    /// `draw_terminal`'s own real per-run cached shape -- the identical
    /// "equality on the key is the whole invalidation check" contract
    /// `shaped_layout` establishes, applied to a compound `(NodeId,
    /// row, col)` key since one terminal `NodeId` covers many
    /// independently-cacheable runs rather than one `Layout` per node.
    /// Compares the cached key's own fields against the *borrowed*
    /// new inputs before ever allocating an owned `TerminalRunKey` --
    /// deliberately not `shaped_layout`'s own pattern (M66, tracked
    /// separately, is the real fix for that one), written this way
    /// from the start since this is new code, not a change to
    /// already-shipped behavior.
    #[allow(clippy::too_many_arguments)]
    fn shaped_terminal_run(
        &mut self,
        node_id: NodeId,
        row: u16,
        col: u16,
        content: &str,
        font_family: &str,
        font_weight: f32,
        font_size: f32,
    ) -> &parley::Layout<[u8; 4]> {
        let cache_key = (node_id, row, col);
        let stale = self
            .terminal_run_cache
            .get(&cache_key)
            .is_none_or(|cached| {
                cached.key.content != content
                    || cached.key.font_family != font_family
                    || cached.key.font_weight != font_weight
                    || cached.key.font_size != font_size
            });
        if stale {
            let layout =
                self.build_field_layout(content, font_family, font_weight, font_size, f32::MAX);
            self.terminal_run_cache.insert(
                cache_key,
                CachedTerminalRun {
                    key: TerminalRunKey {
                        content: content.to_string(),
                        font_family: font_family.to_string(),
                        font_weight,
                        font_size,
                    },
                    layout,
                },
            );
        }
        &self
            .terminal_run_cache
            .get(&cache_key)
            .expect("just inserted above, or already present")
            .layout
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
            state.line_height,
            &[],
            at.color,
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
        // M31 Phase 5 (§5, §8) then M31 Phase 3 (§5, §8): a real click
        // resolves against whatever is actually *painted* -- folded
        // ranges collapsed to a marker, then whitespace substituted on
        // top of that -- so the layout built here must match the
        // identical real chain `draw_field` builds, and the real
        // display-space byte offset `Cursor::from_point` returns has
        // to be mapped back through *both* steps, in reverse order,
        // into `state.content`'s own real byte space before this
        // returns it.
        let folded_content = elide_folded_ranges(&state.content, &state.folded_ranges);
        let content = if state.show_whitespace {
            substitute_whitespace(&folded_content)
        } else {
            folded_content.clone()
        };
        let layout = self.build_field_layout(
            &content,
            &state.font_family,
            state.font_weight,
            state.font_size,
            field_max_width(state, at.max_width),
        );
        let display_offset =
            Cursor::from_point(&layout, (point.x - at.x) as f32, (point.y - at.y) as f32).index();
        let folded_offset = if state.show_whitespace {
            from_display_offset(&folded_content, display_offset)
        } else {
            display_offset
        };
        from_display_offset_folded(state.content.len(), &state.folded_ranges, folded_offset)
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

        // M31 Phase 5 (§5, §8) then M31 Phase 3 (§5, §8): two real,
        // paint-only transforms, chained in this order -- content
        // folding first (collapsing whole real byte ranges to one
        // marker), then visible whitespace glyphs on whatever real
        // text that folding left behind. Both are skipped while a
        // real preedit is active (a vanishingly rare combination in
        // practice; the preedit splice above keeps its own already-
        // correct byte offsets untouched in that case). Each real
        // offset used below to query the final `layout` has to be
        // mapped through *both* steps, in the same order, or cursor/
        // selection/caret/spans would silently desync from what's
        // actually painted.
        let folded_content = elide_folded_ranges(&state.content, &state.folded_ranges);
        let to_display = |offset: usize| -> usize {
            if preedit_range.is_some() {
                return offset;
            }
            let folded =
                to_display_offset_folded(state.content.len(), &state.folded_ranges, offset);
            if state.show_whitespace {
                to_display_offset(&folded_content, folded)
            } else {
                folded
            }
        };

        let display_content = if preedit_range.is_none() {
            if state.show_whitespace {
                substitute_whitespace(&folded_content)
            } else {
                folded_content.clone()
            }
        } else {
            display_content
        };

        let cursor_for_layout = to_display(state.cursor);
        let anchor_for_layout = state.selection_anchor.map(to_display);
        let caret_at = to_display(caret_at);

        // M31 Phase 4 (§5, §8): the app's own real syntax spans,
        // remapped through the identical real chained offset map
        // whenever folding/whitespace substitution shifted `display_
        // content`'s own byte layout -- every real feature addressing
        // the same real `display_content` has to agree on its offsets.
        let display_spans: Vec<(Range<usize>, Color)> = state
            .syntax_spans
            .iter()
            .map(|(range, color)| (to_display(range.start)..to_display(range.end), *color))
            .collect();

        let layout = self.shaped_layout(
            node_id,
            &display_content,
            &state.font_family,
            state.font_weight,
            state.font_size,
            field_max_width(state, at.max_width),
            TextAlign::Start,
            // M62 Phase 1 (§7.1, §16.3): `TextFieldState` is a distinct
            // real struct from `TextState` and outside this milestone's
            // own scope (MD3's typography *type scale* targets display
            // text, not editable fields) -- always `None`, the exact
            // pre-M62 behavior, never wired to any real per-field value.
            None,
            &display_spans,
            at.color,
        );

        // Selection highlight, painted first (behind the glyphs below).
        // Real selection and a real active composition are mutually
        // exclusive in practice (an IME owns keyboard input entirely
        // while composing, confirmed via direct source read of
        // `winit::window::Window::set_ime_allowed`'s own doc comment),
        // so this stays keyed on `cursor_for_layout` unconditionally --
        // `state.cursor` itself, remapped through `to_display_offset`
        // whenever whitespace substitution is active (M31 Phase 3).
        if let Some(anchor) = anchor_for_layout
            && anchor != cursor_for_layout
        {
            let anchor_cursor = Cursor::from_byte_index(layout, anchor, Affinity::Downstream);
            let focus_cursor =
                Cursor::from_byte_index(layout, cursor_for_layout, Affinity::Downstream);
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

        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                // M31 Phase 4 (§5, §8): real per-glyph syntax coloring
                // -- each real `parley::Glyph` (from `positioned_
                // glyphs()`) carries its own real `style_index` into
                // `layout.styles()`, confirmed via direct source read
                // (the identical real lookup `parley::Cluster::first_
                // style` itself does; no public per-run brush read-back
                // exists, so this reads it per-glyph instead). Batches
                // consecutive glyphs that resolve to the same real
                // color into one real `fill_glyphs` call -- mirrors the
                // identical real "background/glyph run" batching
                // `TextRenderer::draw_terminal` already uses, not one
                // draw call per glyph.
                let styles = layout.styles();
                let mut batch: Vec<glifo::Glyph> = Vec::new();
                let mut batch_color = at.color;
                for g in glyph_run.positioned_glyphs() {
                    let [r, gr, b, a] = styles[g.style_index as usize].brush;
                    let color = Color::from_rgba8(r, gr, b, a);
                    if !batch.is_empty() && color != batch_color {
                        scene.set_paint(batch_color);
                        scene
                            .glyph_run(resources, font)
                            .font_size(font_size)
                            .fill_glyphs(std::mem::take(&mut batch).into_iter());
                    }
                    batch_color = color;
                    batch.push(glifo::Glyph {
                        id: g.id,
                        x: g.x + at.x as f32,
                        y: g.y + at.y as f32,
                    });
                }
                if !batch.is_empty() {
                    scene.set_paint(batch_color);
                    scene
                        .glyph_run(resources, font)
                        .font_size(font_size)
                        .fill_glyphs(batch.into_iter());
                }
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
    /// established, reused here directly. **M32 Phase 1 (§5, §8, §10):**
    /// `cell_width`/`cell_height` are now the real per-`state.font_
    /// family`/`state.font_size` measured monospace metrics
    /// (`monospace_cell_size`, cached), not the old fixed `font_size *
    /// 0.6`/`* 1.3` analytic estimate -- a genuinely monospace bundled
    /// face (`Hack Nerd Font Mono`) means every glyph really does share
    /// one advance width, so the grid glyphs are shaped onto now
    /// reflects the real font actually being painted, not a guess.
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
        node_id: NodeId,
    ) {
        let (cell_width, cell_height) =
            self.monospace_cell_size(&state.font_family, state.font_size);
        let cell_width = f64::from(cell_width);
        let cell_height = f64::from(cell_height);

        // M32 Phase 6 (§4, §5, §8): the real, normalized selection
        // range (if any) -- computed once, outside the row loop, the
        // identical real "collapsed (start == end) means no real
        // selection" contract `Tree::terminal_selected_text` already
        // established.
        let selection = match (state.selection_start, state.selection_end) {
            (Some(a), Some(b)) if a != b => Some(if a <= b { (a, b) } else { (b, a) }),
            _ => None,
        };

        for row in 0..state.rows {
            // Real background runs: a contiguous span of cells sharing
            // one real *effective* bg color, painted as one rect --
            // `TRANSPARENT` (`TerminalCell::blank`'s own real default)
            // is skipped entirely, the same "don't paint a real
            // default" every other `NodeKind` arm here already does.
            // M39 Phase 4 (§5, §7): "effective" now accounts for a
            // real `inverse` cell -- see `terminal_cell_effective_
            // colors`'s own doc comment.
            let mut col = 0u16;
            while col < state.cols {
                let bg = terminal_cell_effective_colors(state.cell(row, col)).1;
                let mut end = col + 1;
                while end < state.cols
                    && terminal_cell_effective_colors(state.cell(row, end)).1 == bg
                {
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

            // M32 Phase 6 (§4, §5, §8): the real selection highlight
            // for this row, painted after real cell backgrounds (so it
            // genuinely tints them, the same real "highlight over
            // whatever's already there" layering `draw_field`'s own
            // selection painting already established) but before the
            // glyph runs below (so real text still reads on top of it,
            // matching `draw_field`'s own explicit real stacking
            // order). Real *linear* selection: the middle row of a
            // multi-row range highlights its own whole width; the
            // first/last rows highlight only their own real column
            // span.
            if let Some(((start_row, start_col), (end_row, end_col))) = selection
                && row >= start_row
                && row <= end_row
            {
                let col_start = if row == start_row { start_col } else { 0 };
                let col_end = if row == end_row { end_col } else { state.cols };
                if col_end > col_start {
                    let x0 = at.x + f64::from(col_start) * cell_width;
                    let y0 = at.y + f64::from(row) * cell_height;
                    let rect = Rect::new(
                        x0,
                        y0,
                        x0 + f64::from(col_end - col_start) * cell_width,
                        y0 + cell_height,
                    );
                    scene.set_paint(crate::with_opacity(at.color, 0.3));
                    scene.fill_path(&rect.to_path(0.1));
                }
            }

            // Real glyph runs: a contiguous span of cells sharing one
            // real (effective ink color, bold, italic, underline)
            // tuple, shaped and painted as one string -- a blank
            // cell's own real transparent effective foreground
            // (`TerminalCell::blank`'s own default) never reaches
            // `build_field_layout` at all (an empty/whitespace-only
            // run has no real glyphs to paint), so a genuinely empty
            // terminal costs nothing beyond the background loop above.
            // M39 Phase 4 (§5, §7): "effective ink color" now folds in
            // both `inverse` (via `terminal_cell_effective_colors`)
            // and `dim` (a real, reduced-opacity tint applied once
            // here, so two adjacent cells that only differ by `dim`
            // naturally break into separate runs -- their resolved
            // colors are simply no longer equal).
            let ink = |cell: &TerminalCell| -> Color {
                let fg = terminal_cell_effective_colors(cell).0;
                if cell.dim {
                    crate::with_opacity(fg, 0.6)
                } else {
                    fg
                }
            };
            let mut col = 0u16;
            while col < state.cols {
                let first = state.cell(row, col);
                let fg = ink(first);
                let (bold, italic, underline) = (first.bold, first.italic, first.underline);
                let mut end = col + 1;
                while end < state.cols {
                    let next = state.cell(row, end);
                    if ink(next) != fg
                        || next.bold != bold
                        || next.italic != italic
                        || next.underline != underline
                    {
                        break;
                    }
                    end += 1;
                }
                let run: String = (col..end).map(|c| state.cell(row, c).ch).collect();
                if fg != Color::TRANSPARENT && !run.trim().is_empty() {
                    let font_weight = if bold { 700.0 } else { 400.0 };
                    let layout = self.shaped_terminal_run(
                        node_id,
                        row,
                        col,
                        &run,
                        &state.font_family,
                        font_weight,
                        state.font_size,
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
                            let mut builder = scene.glyph_run(resources, font).font_size(font_size);
                            if italic {
                                // M39 Phase 4 (§5, §7): a real synthetic-
                                // italic shear -- the bundled monospace
                                // face (`Hack Nerd Font Mono`) has no
                                // real italic variant of its own, so
                                // this leans the glyph outlines directly
                                // instead, `glifo::GlyphRunBuilder::
                                // glyph_transform`'s own doc comment's
                                // exact sanctioned technique ("Use
                                // `Affine::skew` with a horizontal-only
                                // skew to simulate italic text"). 20
                                // degrees is `kurbo::Affine::skew`'s own
                                // doc example angle; negated on the x
                                // axis for this codebase's own real
                                // y-down screen space (that doc example
                                // assumes Y-up) -- a positive shear
                                // there leans the glyph's *top* right,
                                // which in y-down space needs a negative
                                // `skew_x` to lean the same visual
                                // direction (smaller/negative `y` is
                                // "up" here).
                                let shear = -(20f64.to_radians().tan());
                                builder = builder.glyph_transform(Affine::skew(shear, 0.0));
                            }
                            builder.fill_glyphs(glyphs);
                        }
                    }
                    if underline {
                        // A real, analytic underline rule -- positioned
                        // as a fixed fraction of the real measured
                        // `cell_height` (near the row's own bottom,
                        // roughly where a monospace face's own real
                        // baseline sits), not derived from the font's
                        // own true underline-position/thickness metrics
                        // (`skrifa`'s `OS/2`/`post` tables would have
                        // that) -- a real, stated v1 approximation, the
                        // same "analytic grid, not exact font metrics"
                        // scope this whole function's own cell grid
                        // already operates at for backgrounds/cursor.
                        let underline_y = y0 + cell_height * 0.85;
                        let underline_h = (cell_height * 0.06).max(1.0);
                        let rule = Rect::new(
                            x0,
                            underline_y,
                            x0 + f64::from(end - col) * cell_width,
                            underline_y + underline_h,
                        );
                        scene.set_paint(fg);
                        scene.fill_path(&rule.to_path(0.1));
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

    /// M32 Phase 6 (§4, §5, §8): the real, pure-geometry half of mouse
    /// text selection -- turns a real local point (the same local
    /// coordinate space `draw_terminal`'s own `at.x`/`at.y` place a
    /// terminal's own painted origin at) into the real `(row, col)`
    /// cell it lands on, using the identical real `monospace_cell_size`
    /// metrics `draw_terminal` positions every cell on -- so a real
    /// click always resolves to the exact cell it's visually over, by
    /// construction, never a coordinate space that could drift from
    /// what's actually painted. Much simpler than `TextField`'s own
    /// per-glyph `hit_test_position`: every real cell in this grid
    /// shares one uniform width/height, so this is plain division, not
    /// a real `parley::Cursor::from_point` shaping-aware lookup.
    /// Clamps to the real, valid `0..rows`/`0..cols` range -- a real
    /// click/drag past a terminal's own edge (a real, plausible drag
    /// overshoot) still resolves to its nearest real edge cell, the
    /// same real "clamp, don't reject" contract `hit_test_position`'s
    /// own far-past-the-end case already established for `TextField`.
    pub fn terminal_hit_cell(&mut self, state: &TerminalState, point: Point) -> (u16, u16) {
        let (cell_width, cell_height) =
            self.monospace_cell_size(&state.font_family, state.font_size);
        let col = (point.x / f64::from(cell_width)).floor().max(0.0) as u16;
        let row = (point.y / f64::from(cell_height)).floor().max(0.0) as u16;
        (
            row.min(state.rows.saturating_sub(1)),
            col.min(state.cols.saturating_sub(1)),
        )
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

/// M39 Phase 4 (§5, §7): `draw_terminal`'s own real `inverse`
/// resolution -- a real `vt100` "reverse video" cell paints with its
/// own foreground and background swapped, real MD3-neutral terminal
/// emulator behavior this function makes real for both the background-
/// run loop and the glyph-run loop (both need the identical swap, so
/// this is the one real place it's computed, not duplicated).
///
/// **Real, stated v1 simplification:** a swapped foreground can land on
/// `Color::TRANSPARENT` when the cell's own real background was never
/// set (`TerminalCell`'s own `vt100::Color::Default` sentinel,
/// `engine-py::terminal.rs`'s own `screen_cell_to_terminal_cell`) --
/// painting a genuinely invisible glyph there would be a real, visible
/// bug (inverse text silently vanishing), not a defensible edge case,
/// so this falls back to `Color::BLACK` instead: always legible against
/// whatever the swapped-in background block now is, even though it
/// isn't a true theme-aware resolution of "the terminal's own real
/// background color" (`engine-core` has no theme awareness at all,
/// §4, and threading the container's own `PaintProperties.background`
/// through this per-cell path is real, further plumbing this pass
/// doesn't need for a real, legible result).
fn terminal_cell_effective_colors(cell: &TerminalCell) -> (Color, Color) {
    if !cell.inverse {
        return (cell.fg, cell.bg);
    }
    let fg = if cell.bg == Color::TRANSPARENT {
        Color::BLACK
    } else {
        cell.bg
    };
    (fg, cell.fg)
}

/// M31 Phase 3 (§5, §8): the real substitute for each whitespace
/// character `TextFieldState.show_whitespace` asks to make visible --
/// middle dot for space (`·`, U+00B7) and a rightward arrow for tab
/// (`→`, U+2192), the same real convention VS Code/Sublime Text use.
/// A real, honest v1 simplification: a tab paints as one arrow glyph,
/// not a real glyph spanning to the next tab stop's own column (this
/// codebase tracks no tab-stop width anywhere).
fn whitespace_glyph(c: char) -> char {
    match c {
        ' ' => '\u{B7}',
        '\t' => '\u{2192}',
        other => other,
    }
}

/// `draw_field`'s own real, paint-only transform -- `content` itself
/// is never touched (`TextFieldState.show_whitespace`'s own doc
/// comment); this only ever changes what gets shaped and painted.
fn substitute_whitespace(content: &str) -> String {
    content.chars().map(whitespace_glyph).collect()
}

/// Maps a real byte offset into `content` to the corresponding byte
/// offset into `substitute_whitespace(content)` -- correct because the
/// substitution is exactly one real char in for one real char out,
/// even though `·`/`→` are multi-byte in UTF-8 while the space/tab
/// they replace are one byte each, so `content`'s own real cursor/
/// selection byte offsets can't be used against the substituted
/// `Layout` directly without this.
fn to_display_offset(content: &str, original_offset: usize) -> usize {
    content
        .char_indices()
        .take_while(|&(i, _)| i < original_offset)
        .map(|(_, c)| whitespace_glyph(c).len_utf8())
        .sum()
}

/// `to_display_offset`'s own real inverse -- `hit_test_position`'s own
/// real need, translating a real click's resolved *display*-space byte
/// offset back into `content`'s real byte space before it's stored as
/// `TextFieldState.cursor`.
fn from_display_offset(content: &str, display_offset: usize) -> usize {
    let mut acc = 0;
    for (i, c) in content.char_indices() {
        if acc >= display_offset {
            return i;
        }
        acc += whitespace_glyph(c).len_utf8();
    }
    content.len()
}

/// M31 Phase 5 (§5, §8): real, paint-only content folding -- the real
/// visible marker a folded range collapses to (`⋯`, U+22EF MIDLINE
/// HORIZONTAL ELLIPSIS, the same real "something is hidden here" glyph
/// convention VS Code/Sublime Text both use, not a silent vanish).
const FOLD_MARKER: char = '\u{22EF}';

/// One real stretch of `content` on the real content-to-display
/// mapping every fold-aware function below walks identically --
/// either passed through byte-for-byte (`Verbatim`) or collapsed to
/// one real `FOLD_MARKER` glyph (`Folded`). Factored out once so
/// `elide_folded_ranges`/`to_display_offset_folded`/`from_display_
/// offset_folded` can never silently disagree about where a fold's
/// own real boundaries fall.
enum FoldSegment {
    Verbatim(Range<usize>),
    Folded(Range<usize>),
}

/// Real, defensive normalization of `folded` against `content_len` --
/// `engine-core` never validates `TextFieldState.folded_ranges`
/// itself (`syntax_spans`'s own identical real "the app's own
/// concern" contract), so a malformed real range (out of order,
/// overlapping, out of bounds) is skipped here rather than corrupting
/// every real offset computed downstream of it.
fn fold_segments(content_len: usize, folded: &[Range<usize>]) -> Vec<FoldSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    for range in folded {
        if range.start < cursor || range.end <= range.start || range.end > content_len {
            continue;
        }
        if range.start > cursor {
            segments.push(FoldSegment::Verbatim(cursor..range.start));
        }
        segments.push(FoldSegment::Folded(range.clone()));
        cursor = range.end;
    }
    if cursor < content_len {
        segments.push(FoldSegment::Verbatim(cursor..content_len));
    }
    segments
}

/// `draw_field`'s own real, paint-only transform -- `state.content`
/// itself is never touched (`TextFieldState.folded_ranges`'s own doc
/// comment); every real folded byte range collapses into one real
/// `FOLD_MARKER` glyph.
fn elide_folded_ranges(content: &str, folded: &[Range<usize>]) -> String {
    let mut out = String::with_capacity(content.len());
    for segment in fold_segments(content.len(), folded) {
        match segment {
            FoldSegment::Verbatim(range) => out.push_str(&content[range]),
            FoldSegment::Folded(_) => out.push(FOLD_MARKER),
        }
    }
    out
}

/// Maps a real byte offset into `content` to the corresponding byte
/// offset into `elide_folded_ranges(content, folded)`. A real clamp
/// for an offset landing *inside* a real folded range: resolves to
/// right after that fold's own real marker, the same "can't usefully
/// distinguish a position inside genuinely hidden content" reasoning
/// `to_display_offset`'s own real one-char-in-one-char-out design
/// never has to make. M38 Phase 3 (§5, §8): `engine-core`'s own
/// `Tree::snap_out_of_fold` now applies the identical convention
/// directly to `TextFieldState.cursor` for `Home`/`End`/`ArrowUp`/
/// `ArrowDown`, so this clamp is no longer the *only* real defense --
/// but it's still a genuinely necessary fallback for every other path
/// that can still place `cursor` inside a fold (a real click via
/// `Tree::set_text_field_cursor`'s own hit-test, a programmatic move,
/// or a future caller this file can't see), not dead code once those
/// four keys became fold-aware.
fn to_display_offset_folded(content_len: usize, folded: &[Range<usize>], offset: usize) -> usize {
    let mut display = 0;
    for segment in fold_segments(content_len, folded) {
        match segment {
            FoldSegment::Verbatim(range) => {
                if offset <= range.end {
                    return display + offset.saturating_sub(range.start);
                }
                display += range.end - range.start;
            }
            FoldSegment::Folded(range) => {
                if offset < range.end {
                    return display + FOLD_MARKER.len_utf8();
                }
                display += FOLD_MARKER.len_utf8();
            }
        }
    }
    display
}

/// `to_display_offset_folded`'s own real inverse -- `hit_test_
/// position`'s own real need, translating a real click's resolved
/// *display*-space byte offset (against the real folded/elided
/// layout) back into `content`'s own real byte space. A click
/// resolving inside a real marker's own glyph lands at that fold's
/// own real start byte -- clicking a collapsed "⋯" is a real,
/// reasonable place to land a caret right before what it hides.
fn from_display_offset_folded(
    content_len: usize,
    folded: &[Range<usize>],
    display_offset: usize,
) -> usize {
    let mut display = 0;
    for segment in fold_segments(content_len, folded) {
        match segment {
            FoldSegment::Verbatim(range) => {
                let len = range.end - range.start;
                if display_offset <= display + len {
                    return range.start + (display_offset - display);
                }
                display += len;
            }
            FoldSegment::Folded(range) => {
                if display_offset < display + FOLD_MARKER.len_utf8() {
                    return range.start;
                }
                display += FOLD_MARKER.len_utf8();
            }
        }
    }
    content_len
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
                line_height: None,
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

    /// M64 (§8, §11.3): a real `NodeKind::Terminal` node -- `draw_
    /// terminal` itself never consults the tree, only `evict_stale_
    /// layouts` does (to know whether a cached run's own terminal
    /// still exists, and at what current `rows`/`cols`), so this is
    /// what the eviction tests below need a real `NodeId` for.
    fn terminal_node(tree: &mut Tree, state: TerminalState) -> engine_core::NodeId {
        tree.insert(
            NodeKind::Terminal(state),
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        )
    }

    /// A single real, non-blank cell at `(row, col)` -- `draw_terminal`
    /// skips a blank cell's own real transparent default foreground
    /// entirely (no glyphs to paint), so a real cache-behavior test
    /// needs at least one cell with a real, opaque `fg` to produce an
    /// actual shaped run at all.
    fn set_cell(state: &mut TerminalState, row: u16, col: u16, ch: char, fg: Color) {
        let index = usize::from(row) * usize::from(state.cols) + usize::from(col);
        state.cells[index] = TerminalCell {
            ch,
            fg,
            ..TerminalCell::blank()
        };
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

    /// M66 (§8): real, per-field regression coverage for `shaped_
    /// layout`'s own hand-written staleness comparison (M66 replaced
    /// the derived `PartialEq` whole-key comparison with one that
    /// checks the *borrowed* new inputs against the cached key's own
    /// fields, to avoid allocating an owned key on the cache-hit path)
    /// -- a real, non-hypothetical risk a hand-written comparison has
    /// that a derived one doesn't: a field silently missing from it
    /// would make that one input stop invalidating the cache at all.
    /// Changes exactly one real shaping input at a time, keeping every
    /// other one identical, and confirms the cached key's own field
    /// actually updated -- real, direct proof of a genuine reshape,
    /// not just "the cache didn't grow" (which a bug that silently
    /// *always* reshapes would also satisfy).
    #[test]
    fn shaped_layout_reshapes_on_a_change_to_any_single_real_field() {
        let mut renderer = TextRenderer::new();
        let id = Tree::new().insert(
            NodeKind::Text(TextState {
                content: "placeholder".to_string(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: TextAlign::Start,
                line_height: None,
            }),
            Style::default(),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 0.0, 1.0),
        );

        let base_spans: Vec<(Range<usize>, Color)> =
            vec![(0..2, Color::from_rgba8(255, 0, 0, 255))];
        renderer.shaped_layout(
            id,
            "hello",
            "Roboto",
            400.0,
            16.0,
            100.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            "Roboto",
            400.0,
            16.0,
            100.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.content,
            "goodbye"
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            400.0,
            16.0,
            100.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.font_family,
            MONOSPACE_FONT_FAMILY
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            16.0,
            100.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.font_weight,
            700.0
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            100.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(renderer.layout_cache.get(&id).unwrap().key.font_size, 24.0);

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            50.0,
            TextAlign::Start,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(renderer.layout_cache.get(&id).unwrap().key.max_width, 50.0);

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            50.0,
            TextAlign::Center,
            None,
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.align,
            TextAlign::Center
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            50.0,
            TextAlign::Center,
            Some(1.5),
            &base_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.line_height,
            Some(1.5)
        );

        let changed_spans: Vec<(Range<usize>, Color)> =
            vec![(0..2, Color::from_rgba8(0, 255, 0, 255))];
        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            50.0,
            TextAlign::Center,
            Some(1.5),
            &changed_spans,
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.spans,
            changed_spans
        );

        renderer.shaped_layout(
            id,
            "goodbye",
            MONOSPACE_FONT_FAMILY,
            700.0,
            24.0,
            50.0,
            TextAlign::Center,
            Some(1.5),
            &changed_spans,
            Color::from_rgba8(255, 255, 255, 255),
        );
        assert_eq!(
            renderer.layout_cache.get(&id).unwrap().key.default_color,
            Color::from_rgba8(255, 255, 255, 255)
        );
    }

    /// M31 Phase 1 (§5, §8): the real finding that closes this phase's
    /// own "real, open technical question" (`BUILD_TRACKER.md`'s own
    /// Phase 1 scoping note) without any new per-line-position API at
    /// all -- `draw` (plain `Text`) and `draw_field` (`TextField`) both
    /// build their `Layout` through this exact same `shaped_layout`
    /// method, confirmed by direct source read, not assumed. So a
    /// gutter composed as an ordinary sibling `Text` node (real digits
    /// joined by `\n`, same `font_family`/`font_weight`/`font_size` as
    /// the editor, wide enough not to wrap) lines up with the editor's
    /// own real per-line Y positions *by construction*, with zero new
    /// engine capability needed -- proven here directly against real
    /// `parley::Layout::lines()` geometry, not just plausible-sounding.
    #[test]
    fn a_plain_texts_own_multiline_content_lines_up_with_a_matching_multiline_textfields_own_lines()
    {
        let mut tree = Tree::new();
        let gutter_id = text_node(&mut tree, "1\n2\n3");
        let editor_id = text_node(&mut tree, "3\n2\n1"); // a second, distinct real NodeId

        let mut renderer = TextRenderer::new();
        let gutter_ys: Vec<f32> = renderer
            .shaped_layout(
                gutter_id,
                "1\n2\n3",
                "Roboto",
                400.0,
                16.0,
                100.0,
                TextAlign::Start,
                None,
                &[],
                Color::from_rgba8(0, 0, 0, 255),
            )
            .lines()
            .map(|line| line.metrics().block_min_coord)
            .collect();
        let field_ys: Vec<f32> = renderer
            .shaped_layout(
                editor_id,
                "1\n2\n3",
                "Roboto",
                400.0,
                16.0,
                f32::MAX,
                TextAlign::Start,
                None,
                &[],
                Color::from_rgba8(0, 0, 0, 255),
            )
            .lines()
            .map(|line| line.metrics().block_min_coord)
            .collect();

        assert_eq!(
            gutter_ys.len(),
            3,
            "three source lines must produce three real layout lines"
        );
        assert_eq!(
            gutter_ys, field_ys,
            "a plain Text's own per-line Y offsets must exactly match a matching multiline \
             TextField's, real proof that a gutter can be composed as an ordinary sibling node"
        );
    }

    /// M62 Phase 1 (§7.1, §16.3): real proof `line_height` isn't just
    /// inert stored data -- a `Some(ratio)` must genuinely change the
    /// real `parley::Layout`'s own per-line vertical advance, read back
    /// via `Layout::lines()`'s own real geometry (the identical real
    /// verification technique the gutter-alignment test above already
    /// established for this module), not merely "doesn't panic."
    #[test]
    fn a_larger_line_height_genuinely_widens_the_real_per_line_advance() {
        let mut tree = Tree::new();
        let default_id = text_node(&mut tree, "one\ntwo");
        let doubled_id = text_node(&mut tree, "one\ntwo"); // a second, distinct real NodeId

        let mut renderer = TextRenderer::new();
        let default_ys: Vec<f32> = renderer
            .shaped_layout(
                default_id,
                "one\ntwo",
                "Roboto",
                400.0,
                16.0,
                100.0,
                TextAlign::Start,
                None,
                &[],
                Color::from_rgba8(0, 0, 0, 255),
            )
            .lines()
            .map(|line| line.metrics().block_min_coord)
            .collect();
        let doubled_ys: Vec<f32> = renderer
            .shaped_layout(
                doubled_id,
                "one\ntwo",
                "Roboto",
                400.0,
                16.0,
                100.0,
                TextAlign::Start,
                Some(2.0),
                &[],
                Color::from_rgba8(0, 0, 0, 255),
            )
            .lines()
            .map(|line| line.metrics().block_min_coord)
            .collect();

        assert_eq!(default_ys.len(), 2);
        assert_eq!(doubled_ys.len(), 2);
        // A larger line_height distributes extra leading on both sides
        // of each line (real `parley` behavior, confirmed here rather
        // than assumed -- it also nudges the first line's own top up
        // slightly to fit half the extra leading above it), so the
        // real claim isn't "the first line never moves," it's "the gap
        // between consecutive lines genuinely widens."
        let default_advance = default_ys[1] - default_ys[0];
        let doubled_advance = doubled_ys[1] - doubled_ys[0];
        assert!(
            doubled_advance > default_advance * 1.5,
            "a 2.0x font-size-relative line_height must produce a real, substantially larger \
             per-line advance than the default (font-metrics-relative) one -- got default={default_advance}, doubled={doubled_advance}"
        );
    }

    /// M31 Phase 3 (§5, §8): white-box proof of the real offset-mapping
    /// machinery `hit_test_position`'s own integration test (`crates/
    /// engine-render/tests/text_field_paint.rs`) only exercises
    /// end-to-end -- every real char boundary in a mixed ASCII/space/
    /// tab/multi-byte string must round-trip through `to_display_
    /// offset`/`from_display_offset` exactly, not just at the two
    /// endpoints.
    #[test]
    fn display_offset_mapping_round_trips_every_real_char_boundary() {
        let content = "a b\tcafé d";
        for (byte_offset, _) in content.char_indices() {
            let display = to_display_offset(content, byte_offset);
            let back = from_display_offset(content, display);
            assert_eq!(
                back, byte_offset,
                "byte offset {byte_offset} in {content:?} must round-trip through the real \
                 display-offset mapping unchanged, got {back} (via display offset {display})"
            );
        }
        // And the real, whole-string end, the same real off-by-one-prone
        // edge `from_display_offset`'s own `content.len()` fallback
        // guards.
        let end_display = to_display_offset(content, content.len());
        assert_eq!(from_display_offset(content, end_display), content.len());
    }

    #[test]
    fn substitute_whitespace_replaces_only_space_and_tab_with_real_visible_glyphs() {
        assert_eq!(substitute_whitespace("a b\tc"), "a\u{B7}b\u{2192}c");
        assert_eq!(
            substitute_whitespace("café"),
            "café",
            "a real non-whitespace character must never be substituted"
        );
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn elide_folded_ranges_collapses_each_real_range_to_one_marker() {
        // "0123456789" with 3..6 ("345") folded.
        assert_eq!(
            elide_folded_ranges("0123456789", &[3..6]),
            "012\u{22EF}6789"
        );
        // Two real, non-adjacent folds.
        assert_eq!(
            elide_folded_ranges("0123456789", &[1..3, 7..9]),
            "0\u{22EF}3456\u{22EF}9"
        );
        // No real folds at all -- byte-for-byte unchanged.
        assert_eq!(elide_folded_ranges("hello", &[]), "hello");
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn elide_folded_ranges_skips_a_real_malformed_range_rather_than_corrupting_output() {
        // Out of order (starts before the previous fold's own end),
        // inverted (end <= start), and out of bounds -- each must be
        // skipped, not panic or corrupt the real unfolded remainder.
        assert_eq!(
            elide_folded_ranges("0123456789", &[3..6, 4..5]),
            "012\u{22EF}6789"
        );
        assert_eq!(elide_folded_ranges("0123456789", &[5..5]), "0123456789");
        assert_eq!(elide_folded_ranges("0123456789", &[8..100]), "0123456789");
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn folded_display_offset_mapping_round_trips_every_real_char_boundary_outside_a_fold() {
        let content = "0123456789";
        let folded = [3..6];
        for (byte_offset, _) in content.char_indices() {
            if (3..6).contains(&byte_offset) {
                continue; // real, deliberate v1 clamp -- tested separately below.
            }
            let display = to_display_offset_folded(content.len(), &folded, byte_offset);
            let back = from_display_offset_folded(content.len(), &folded, display);
            assert_eq!(
                back, byte_offset,
                "byte offset {byte_offset} outside any real fold must round-trip unchanged, \
                 got {back} (via display offset {display})"
            );
        }
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn folded_display_offset_clamps_a_real_offset_inside_a_fold_to_just_after_its_marker() {
        let content = "0123456789";
        let folded = [3..6];
        // Every real offset strictly inside the fold (4, 5) must clamp
        // to the identical real display position -- right after the
        // one real marker glyph -- the same real "can't usefully
        // distinguish a position inside genuinely hidden content"
        // reasoning `to_display_offset_folded`'s own doc comment
        // states.
        let at_3 = to_display_offset_folded(content.len(), &folded, 3);
        let at_4 = to_display_offset_folded(content.len(), &folded, 4);
        let at_5 = to_display_offset_folded(content.len(), &folded, 5);
        assert_eq!(at_4, at_3 + FOLD_MARKER.len_utf8());
        assert_eq!(at_5, at_3 + FOLD_MARKER.len_utf8());
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn from_folded_display_offset_inside_the_marker_lands_at_the_folds_own_real_start() {
        let content = "0123456789";
        let folded = [3..6];
        let marker_start = to_display_offset_folded(content.len(), &folded, 3);
        assert_eq!(
            from_display_offset_folded(content.len(), &folded, marker_start),
            3,
            "a real display offset landing on the marker's own glyph must resolve to the \
             fold's own real start byte"
        );
    }

    /// M32 Phase 1 (§5, §8, §10): the real, load-bearing claim this
    /// phase exists to prove -- the bundled `MONOSPACE_FONT_FAMILY` face
    /// genuinely has one uniform advance width across visually
    /// different-width glyphs ("M" wide, "i" narrow), unlike a real
    /// proportional face (`Roboto`, already bundled), which does not.
    /// If the family name failed to resolve to the real bundled font
    /// (a typo'd string, a registration bug), this would either fall
    /// back to a real proportional fallback face (this test would then
    /// fail the same way the `Roboto` half already does) or shape with
    /// zero real glyphs -- either way a real, meaningful failure, not a
    /// vacuous pass.
    #[test]
    fn the_bundled_monospace_face_has_uniform_advance_unlike_a_real_proportional_face() {
        let mut renderer = TextRenderer::new();
        let m_width = renderer
            .build_field_layout("M", MONOSPACE_FONT_FAMILY, 400.0, 16.0, f32::MAX)
            .width();
        let i_width = renderer
            .build_field_layout("i", MONOSPACE_FONT_FAMILY, 400.0, 16.0, f32::MAX)
            .width();
        assert!(
            (m_width - i_width).abs() < 0.01,
            "a genuinely monospace face must give \"M\" and \"i\" the identical real advance \
             width, got M={m_width} i={i_width}"
        );

        let roboto_m_width = renderer
            .build_field_layout("M", "Roboto", 400.0, 16.0, f32::MAX)
            .width();
        let roboto_i_width = renderer
            .build_field_layout("i", "Roboto", 400.0, 16.0, f32::MAX)
            .width();
        assert!(
            (roboto_m_width - roboto_i_width).abs() > 1.0,
            "the real contrast case: Roboto is genuinely proportional, so its own \"M\"/\"i\" \
             advances must differ by a real, visible amount, got M={roboto_m_width} \
             i={roboto_i_width} -- if this ever fails, the contrast this test relies on to \
             prove the monospace claim meaningful no longer holds"
        );
    }

    /// Real, direct coverage of `monospace_cell_size`'s own stated
    /// contract: a real per-font-size measurement (scales with
    /// `font_size`, not a constant), and a real cache that returns the
    /// identical value on a repeat call rather than silently drifting.
    #[test]
    fn monospace_cell_size_scales_with_font_size_and_is_cached() {
        let mut renderer = TextRenderer::new();
        let (w14, h14) = renderer.monospace_cell_size(MONOSPACE_FONT_FAMILY, 14.0);
        let (w28, h28) = renderer.monospace_cell_size(MONOSPACE_FONT_FAMILY, 28.0);
        assert!(
            w14 > 0.0 && h14 > 0.0,
            "a real font's own measured cell size must be strictly positive"
        );
        assert!(
            w28 > w14 && h28 > h14,
            "doubling font_size must genuinely grow both real measured dimensions, got \
             14pt=({w14}, {h14}) 28pt=({w28}, {h28})"
        );
        let (w14_again, h14_again) = renderer.monospace_cell_size(MONOSPACE_FONT_FAMILY, 14.0);
        assert_eq!(
            (w14, h14),
            (w14_again, h14_again),
            "a repeat call at the identical (font_family, font_size) must return the identical \
             cached value, not reshape and silently drift"
        );
    }

    /// M32 Phase 6 (§4, §5, §8): `terminal_hit_cell`'s own real
    /// contract -- a point inside a given real cell's own box resolves
    /// to that exact `(row, col)`, using the identical real metrics
    /// `draw_terminal` positions every cell on.
    #[test]
    fn terminal_hit_cell_resolves_a_point_to_its_own_real_cell() {
        let mut renderer = TextRenderer::new();
        let state = engine_core::TerminalState::new(10, 5, MONOSPACE_FONT_FAMILY, 16.0);
        let (cell_width, cell_height) = renderer.monospace_cell_size(MONOSPACE_FONT_FAMILY, 16.0);

        assert_eq!(
            renderer.terminal_hit_cell(&state, Point::new(0.0, 0.0)),
            (0, 0),
            "the real top-left origin must resolve to the real first cell"
        );
        // The real center of cell (row 2, col 3).
        let x = f64::from(cell_width) * 3.5;
        let y = f64::from(cell_height) * 2.5;
        assert_eq!(renderer.terminal_hit_cell(&state, Point::new(x, y)), (2, 3));
    }

    #[test]
    fn terminal_hit_cell_clamps_a_real_point_past_the_grids_own_edge() {
        let mut renderer = TextRenderer::new();
        let state = engine_core::TerminalState::new(10, 5, MONOSPACE_FONT_FAMILY, 16.0);
        assert_eq!(
            renderer.terminal_hit_cell(&state, Point::new(10_000.0, 10_000.0)),
            (4, 9),
            "a real point far past the grid's own edge (a plausible drag overshoot) must clamp \
             to the nearest real edge cell, not panic or return an out-of-bounds index"
        );
        assert_eq!(
            renderer.terminal_hit_cell(&state, Point::new(-5.0, -5.0)),
            (0, 0),
            "a real point before the grid's own origin must clamp to the first real cell"
        );
    }

    // --- M64 (§8, §11.3): draw_terminal's own real per-row/per-run
    // shaping cache. ------------------------------------------------

    #[test]
    fn terminal_run_cache_does_not_grow_on_an_unchanged_repaint() {
        pollster::block_on(async {
            let mut frame_renderer = frame_renderer_for_test().await;
            let mut renderer = TextRenderer::new();
            let mut scene = Scene::new(100, 100);
            let mut tree = Tree::new();

            let mut state = TerminalState::new(10, 3, MONOSPACE_FONT_FAMILY, 16.0);
            set_cell(&mut state, 0, 0, 'h', Color::from_rgba8(255, 255, 255, 255));
            set_cell(&mut state, 0, 1, 'i', Color::from_rgba8(255, 255, 255, 255));
            let id = terminal_node(&mut tree, state.clone());

            renderer.draw_terminal(
                &mut scene,
                frame_renderer.resources_mut(),
                &state,
                placement(),
                false,
                id,
            );
            assert_eq!(
                renderer.terminal_run_cache.len(),
                1,
                "one real contiguous same-style run (\"hi\") must produce exactly one cache entry"
            );

            // Repainting the identical, unchanged terminal must not
            // grow the cache -- the whole real point of this milestone.
            renderer.draw_terminal(
                &mut scene,
                frame_renderer.resources_mut(),
                &state,
                placement(),
                false,
                id,
            );
            assert_eq!(
                renderer.terminal_run_cache.len(),
                1,
                "repainting an unchanged terminal must not create a second entry"
            );
        });
    }

    #[test]
    fn terminal_run_cache_reshapes_only_the_row_whose_content_actually_changed() {
        pollster::block_on(async {
            let mut frame_renderer = frame_renderer_for_test().await;
            let mut renderer = TextRenderer::new();
            let mut scene = Scene::new(100, 100);
            let mut tree = Tree::new();

            let mut state = TerminalState::new(10, 2, MONOSPACE_FONT_FAMILY, 16.0);
            set_cell(&mut state, 0, 0, 'a', Color::from_rgba8(255, 255, 255, 255));
            set_cell(&mut state, 1, 0, 'b', Color::from_rgba8(255, 255, 255, 255));
            let id = terminal_node(&mut tree, state.clone());

            renderer.draw_terminal(
                &mut scene,
                frame_renderer.resources_mut(),
                &state,
                placement(),
                false,
                id,
            );
            assert_eq!(renderer.terminal_run_cache.len(), 2, "one run per row");
            let row0_key_before = renderer
                .terminal_run_cache
                .get(&(id, 0, 0))
                .expect("row 0's own run must be cached")
                .key
                .content
                .clone();
            assert_eq!(row0_key_before, "a");

            // Only row 1's own content changes -- row 0 stays identical.
            set_cell(&mut state, 1, 0, 'c', Color::from_rgba8(255, 255, 255, 255));
            renderer.draw_terminal(
                &mut scene,
                frame_renderer.resources_mut(),
                &state,
                placement(),
                false,
                id,
            );
            assert_eq!(
                renderer.terminal_run_cache.len(),
                2,
                "still exactly one real entry per row, no leak from the reshape"
            );
            assert_eq!(
                renderer
                    .terminal_run_cache
                    .get(&(id, 0, 0))
                    .expect("row 0's own entry must still be present")
                    .key
                    .content,
                "a",
                "row 0's own cached run must be untouched -- its real content never changed"
            );
            assert_eq!(
                renderer
                    .terminal_run_cache
                    .get(&(id, 1, 0))
                    .expect("row 1's own entry must still be present")
                    .key
                    .content,
                "c",
                "row 1's own cached run must reflect the real, changed content"
            );
        });
    }

    #[test]
    fn terminal_run_cache_is_evicted_on_removal_and_on_resize_shrink() {
        pollster::block_on(async {
            let mut frame_renderer = frame_renderer_for_test().await;
            let mut renderer = TextRenderer::new();
            let mut scene = Scene::new(100, 100);
            let mut tree = Tree::new();

            let mut state = TerminalState::new(10, 3, MONOSPACE_FONT_FAMILY, 16.0);
            for row in 0..3u16 {
                set_cell(
                    &mut state,
                    row,
                    0,
                    'x',
                    Color::from_rgba8(255, 255, 255, 255),
                );
            }
            let id = terminal_node(&mut tree, state.clone());
            renderer.draw_terminal(
                &mut scene,
                frame_renderer.resources_mut(),
                &state,
                placement(),
                false,
                id,
            );
            assert_eq!(
                renderer.terminal_run_cache.len(),
                3,
                "one run per row, 3 rows"
            );

            // A real resize down to 1 row -- `evict_stale_layouts` must
            // drop the now-out-of-bounds rows 1/2 even though the
            // terminal's own NodeId is still alive, the real case
            // `layout_cache`'s own eviction has no equivalent of.
            let NodeKind::Terminal(live_state) = &mut tree.get_mut(id).unwrap().kind else {
                panic!("expected Terminal");
            };
            live_state.rows = 1;
            renderer.evict_stale_layouts(&tree);
            assert_eq!(
                renderer.terminal_run_cache.len(),
                1,
                "rows beyond the terminal's own new, smaller real row count must be evicted"
            );
            assert!(renderer.terminal_run_cache.contains_key(&(id, 0, 0)));

            // Removing the node entirely must evict what's left too.
            tree.remove(id);
            renderer.evict_stale_layouts(&tree);
            assert_eq!(
                renderer.terminal_run_cache.len(),
                0,
                "a removed terminal's own cached runs must not be kept forever"
            );
        });
    }
}
