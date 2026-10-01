//! 0.4.0 M4: which parts of a window changed since the last frame.
//!
//! **Compare, don't instrument.** Pixels change through `Tree::get_mut`,
//! direct field writes inside `Tree`, animation ticks, layout, and
//! tree-level state such as focus. Hooking every one of those paths would
//! make any missed one a stale-pixel bug, and partial redraw is on by
//! default. Instead `DamageTracker` walks the tree with the paint walk's
//! own traversal (`walk`: the same visibility, culling, composed
//! transforms, and clips, written once) and records, per node, its
//! *painted rect* in window pixels and a *fingerprint* of everything that
//! decides its pixels. A node that's new, gone, or changed since the last
//! frame contributes its old and new painted rects.
//!
//! The fingerprint code destructures every state struct without `..`: a
//! field added later won't compile until someone decides whether it
//! affects paint. A large terminal or canvas is fingerprinted in full every
//! frame, with a fast hash (`foldhash`) rather than a content counter: a
//! counter bumped where content is written would be instrumentation again,
//! and these states' fields are written directly.
//!
//! The rects then merge into a small set (the user's decision D3): any
//! that overlap merge, then the closest pairs until at most `MAX_RECTS`
//! remain -- or, past 64 of them, their bounding box, which keeps the
//! merge's cost bounded when a whole grid animates; past `FULL_FRACTION` of
//! the window, or on a first frame, a size change, or a `reset`, the answer
//! is a full redraw.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};

use engine_core::{
    CanvasState, CellColor, DrawCommand, ImageState, ItemExtent, Node, NodeId, NodeKind,
    PaintProperties, PathState, ScrollViewState, TerminalState, TextFieldState, TextOptions,
    TextState, Tree, VirtualListState, fit_transform,
};
use peniko::Color;
use peniko::kurbo::{Affine, Rect, Shape};

use crate::{TextRenderer, transformed_bounds, walk};

/// At most this many rects before the closest pairs merge.
pub const MAX_RECTS: usize = 4;
/// Damage covering more than this fraction of the window redraws it all.
pub const FULL_FRACTION: f64 = 0.5;
/// More changed rects than this merge straight into their bounding box,
/// keeping the pair merging's cubic cost bounded.
const MAX_TRACKED: usize = 64;
/// The fingerprint hasher, the same every frame (and every run).
const FINGERPRINT: foldhash::fast::FixedState = foldhash::fast::FixedState::with_seed(0x74_72_65);
/// Pixels of antialiasing and glyph overhang added around every painted
/// rect, beyond its geometry.
const MARGIN: f64 = 2.0;

/// What changed since the last frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Damage {
    /// Nothing: the frame can be skipped.
    None,
    /// Redraw the whole window.
    Full,
    /// Redraw only these window-pixel rects -- at most `MAX_RECTS`,
    /// non-overlapping, together under `FULL_FRACTION` of the window.
    Rects(Vec<Rect>),
}

#[derive(Clone, Copy, PartialEq)]
struct Record {
    painted: Rect,
    fingerprint: u64,
}

/// Remembers each node's painted rect and fingerprint from the last frame.
#[derive(Default)]
pub struct DamageTracker {
    records: HashMap<NodeId, Record>,
    size: Option<(u16, u16)>,
}

/// One frame's walk (`walk::Visitor`): the records it builds and what it
/// needs to build them.
struct Recorder<'a> {
    tree: &'a Tree,
    text: &'a mut TextRenderer,
    records: HashMap<NodeId, Record>,
}

impl DamageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets the last frame, so the next `damage` is `Full` -- for when
    /// the target's contents were lost (a recreated `PersistentTarget`).
    pub fn reset(&mut self) {
        self.records.clear();
        self.size = None;
    }

    /// What changed in `root`'s tree, painted into a `width` x `height`
    /// window, since the last call.
    pub fn damage(
        &mut self,
        tree: &Tree,
        root: NodeId,
        width: u16,
        height: u16,
        text: &mut TextRenderer,
    ) -> Damage {
        let window = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let mut recorder = Recorder {
            tree,
            text,
            records: HashMap::with_capacity(self.records.len()),
        };
        walk::walk(tree, root, window, &mut recorder);
        let current = recorder.records;

        let first = self.size != Some((width, height));
        let previous = std::mem::replace(&mut self.records, current);
        self.size = Some((width, height));
        if first {
            return Damage::Full;
        }

        let mut rects = Vec::new();
        for (id, now) in &self.records {
            match previous.get(id) {
                Some(before) if before == now => {}
                Some(before) => {
                    // A change in place (a colour, a glyph) needs its rect
                    // once, not twice toward the `MAX_TRACKED` cap.
                    if before.painted != now.painted {
                        rects.push(before.painted);
                    }
                    rects.push(now.painted);
                }
                None => rects.push(now.painted),
            }
        }
        for (id, before) in &previous {
            if !self.records.contains_key(id) {
                rects.push(before.painted);
            }
        }
        merge(rects, window)
    }
}

impl<'t> walk::Visitor<'t> for Recorder<'_> {
    /// Records every node the paint walk reaches -- the same `walk`, so
    /// the two can't disagree about which nodes are drawn.
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        let painted = round_out(painted_rect(
            self.text, v.id, v.node, v.composed, v.w, v.h, v.bounds, v.visible,
        ));
        // Fast and fixed-seed: a frame's fingerprints compare with the
        // last frame's, and nothing adversarial picks what's hashed.
        let mut hasher = FINGERPRINT.build_hasher();
        v.parent.hash(&mut hasher);
        v.order.hash(&mut hasher);
        affine(&mut hasher, v.composed);
        num(&mut hasher, v.opacity);
        rect(&mut hasher, v.visible);
        num(&mut hasher, v.w);
        num(&mut hasher, v.h);
        node_fingerprint(&mut hasher, self.tree, v.id, v.node);
        self.records.insert(
            v.id,
            Record {
                painted,
                fingerprint: hasher.finish(),
            },
        );
        true
    }
}

/// What `node` paints, in window pixels: its own extent (`local_painted`)
/// under `composed`, a single-line text input's whole row, and an
/// antialiasing margin, within `visible`. 0.4.0: shared by the damage walk
/// and partial redraw's culling, so a node is repainted wherever it's
/// recorded as painting.
#[allow(clippy::too_many_arguments)]
pub(crate) fn painted_rect(
    text: &mut TextRenderer,
    id: NodeId,
    node: &Node,
    composed: Affine,
    w: f64,
    h: f64,
    bounds: Rect,
    visible: Rect,
) -> Rect {
    let local = local_painted(text, id, node, w, h);
    let mut painted = transformed_bounds(composed, local);
    if let NodeKind::TextField(state) = &node.kind
        && !state.multiline
    {
        // A single-line input's text scrolls sideways past its box,
        // unclipped: take its whole row across the visible width.
        painted = painted.union(Rect::new(visible.x0, bounds.y0, visible.x1, bounds.y1));
    }
    painted.inflate(MARGIN, MARGIN).intersect(visible)
}

/// What `node` paints, in its own coordinates: its box, plus whatever
/// reaches past it.
fn local_painted(text: &mut TextRenderer, id: NodeId, node: &Node, w: f64, h: f64) -> Rect {
    let mut local = Rect::new(0.0, 0.0, w, h);
    for shadow in &node.paint.shadows.current.0 {
        if shadow.color.components[3] <= 0.0 {
            continue;
        }
        // A Gaussian blur with standard deviation `blur / 2` fades
        // out by 3 deviations; `blur * 2` is past that.
        let reach = shadow.spread + shadow.blur * 2.0;
        local = local.union(Rect::new(
            shadow.offset_x - reach,
            shadow.offset_y - reach,
            w + shadow.offset_x + reach,
            h + shadow.offset_y + reach,
        ));
    }
    match &node.kind {
        NodeKind::Text(state) => {
            let (tw, th) = text.text_extent(state, w as f32, node.paint.background.current, id);
            // Glyphs overhang their advance a little (italics,
            // ascenders); a quarter of the font size covers it.
            let overhang = f64::from(state.font_size) * 0.25;
            local = local.union(
                Rect::new(0.0, 0.0, f64::from(tw), f64::from(th)).inflate(overhang, overhang),
            );
        }
        NodeKind::Path(state) => {
            // Bounded without building the geometry: the fit is a uniform
            // scale and a translation, so it maps the data's box exactly,
            // and a trimmed stroke lies within the whole path.
            let bounds = fit_transform(state.view_box, w, h)
                .transform_rect_bbox(state.data.current.0.bounding_box());
            let half = node.paint.border_width.current / 2.0;
            local = local.union(bounds.inflate(half, half));
        }
        NodeKind::Canvas(state) => {
            for command in &state.commands {
                local = local.union(match command {
                    DrawCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        ..
                    } => Rect::new(*x, *y, x + width, y + height),
                    DrawCommand::FillCircle { cx, cy, radius, .. } => {
                        Rect::new(cx - radius, cy - radius, cx + radius, cy + radius)
                    }
                    DrawCommand::StrokePath { path, width, .. } => {
                        path.bounding_box().inflate(width / 2.0, width / 2.0)
                    }
                });
            }
        }
        NodeKind::Terminal(state) => {
            // The whole cols x rows grid, unclipped, whatever the box.
            let (cw, ch) = text.monospace_cell_size(&state.font_family, state.font_size);
            local = local.union(Rect::new(
                0.0,
                0.0,
                f64::from(state.cols) * f64::from(cw),
                f64::from(state.rows) * f64::from(ch),
            ));
        }
        _ => {}
    }
    local
}

/// 0.5.1 (#66, #68): a shader's part in its node's pixels: which shader, in
/// which mode, whether its uniforms or module changed -- and what its inputs
/// hold now: an image node's current frame, or another shader's own
/// fingerprint, so a change anywhere upstream repaints the node.
fn shader_fingerprint(
    h: &mut impl Hasher,
    tree: &Tree,
    shader: &std::sync::Arc<engine_core::Shader>,
    depth: usize,
) {
    1u8.hash(h);
    (std::sync::Arc::as_ptr(shader) as usize).hash(h);
    shader.mode().hash(h);
    shader.animated().hash(h);
    shader.versions().hash(h);
    for (_, input) in shader.inputs() {
        match tree.get(*input) {
            None => 0u8.hash(h),
            Some(node) => match (&node.kind, &node.shader) {
                (NodeKind::Image(state), _) => {
                    1u8.hash(h);
                    state.image.data.id().hash(h);
                }
                (_, Some(inner)) if depth < 16 => {
                    2u8.hash(h);
                    let size = tree.layout(*input).size;
                    size.width.to_bits().hash(h);
                    size.height.to_bits().hash(h);
                    shader_fingerprint(h, tree, inner, depth + 1);
                }
                _ => 3u8.hash(h),
            },
        }
    }
}

/// 0.5.1 (#69): what `id`'s descendants paint, for an effect node: each one's
/// place in the tree, its box, and everything `node_fingerprint` hashes.
fn subtree_fingerprint(h: &mut impl Hasher, tree: &Tree, id: NodeId) {
    for (index, &child) in tree.children_in_paint_order(id).iter().enumerate() {
        let Some(node) = tree.get(child) else {
            continue;
        };
        index.hash(h);
        let layout = tree.layout(child);
        for v in [
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        ] {
            v.to_bits().hash(h);
        }
        node_fingerprint(h, tree, child, node);
        subtree_fingerprint(h, tree, child);
    }
}

/// 0.5.1 (#69): the fingerprint of an effect node's subtree alone -- what its
/// offscreen content render depends on -- as a pass's staleness key.
pub(crate) fn effect_content_fingerprint(tree: &Tree, id: NodeId) -> u64 {
    let mut hasher = FINGERPRINT.build_hasher();
    subtree_fingerprint(&mut hasher, tree, id);
    // The node's own paint is part of its content too.
    if let Some(node) = tree.get(id) {
        let mut own = FINGERPRINT.build_hasher();
        paint_fingerprint(&mut own, &node.paint);
        std::mem::discriminant(&node.kind).hash(&mut hasher);
        own.finish().hash(&mut hasher);
    }
    hasher.finish()
}

/// Everything about `node` itself that decides its pixels.
fn node_fingerprint(h: &mut impl Hasher, tree: &Tree, id: NodeId, node: &Node) {
    let Node {
        id: _,
        parent: _,
        // Children fingerprint themselves, with their paint order.
        children: _,
        visible,
        z_index,
        kind,
        // Layout reaches paint as the box size and composed transform.
        layout_style: _,
        paint,
        // Accessibility, hit testing, and the pointer shape paint nothing.
        access: _,
        hit_testable: _,
        cursor: _,
        // 0.5.0 M3: which presses move the window; paints nothing.
        window_region: _,
        shader,
    } = node;
    visible.hash(h);
    z_index.hash(h);
    // 0.5.1 (#66): which shader, in which mode, and whether its uniforms or
    // module changed since it was last painted.
    match shader {
        None => 0u8.hash(h),
        Some(shader) => {
            shader_fingerprint(h, tree, shader, 0);
            // An effect's result depends on its whole subtree, so a change
            // anywhere in it repaints the node's box.
            if shader.mode() == engine_core::ShaderMode::Effect {
                subtree_fingerprint(h, tree, id);
            }
        }
    }
    paint_fingerprint(h, paint);
    std::mem::discriminant(kind).hash(h);
    // 0.5.1 (#44, #53): a node that draws its own content draws it inside its
    // padding, so padding is part of its pixels even when its box doesn't
    // change size. (A box's padding only moves its children, which
    // fingerprint themselves.)
    if matches!(
        kind,
        NodeKind::Text(_)
            | NodeKind::TextField(_)
            | NodeKind::Terminal(_)
            | NodeKind::Image(_)
            | NodeKind::Path(_)
            | NodeKind::Canvas(_)
    ) {
        let pad = tree.layout(id).padding;
        for edge in [pad.left, pad.top, pad.right, pad.bottom] {
            edge.to_bits().hash(h);
        }
    }
    let focused = tree.focused() == Some(id);
    match kind {
        NodeKind::Rect | NodeKind::Container => {}
        NodeKind::Text(state) => text_fingerprint(h, state),
        NodeKind::TextField(state) => {
            focused.hash(h); // the caret
            field_fingerprint(h, state);
        }
        NodeKind::Image(state) => image_fingerprint(h, state),
        NodeKind::Path(state) => path_fingerprint(h, state),
        NodeKind::Canvas(state) => canvas_fingerprint(h, state),
        NodeKind::ScrollView(state) => {
            scroll_fingerprint(h, state);
            // The scrollbar thumb's size follows the content's.
            for &child in &node.children {
                let size = tree.layout(child).size;
                size.width.to_bits().hash(h);
                size.height.to_bits().hash(h);
            }
        }
        NodeKind::VirtualList(state) => list_fingerprint(h, state),
        NodeKind::Terminal(state) => {
            focused.hash(h); // the cursor
            terminal_fingerprint(h, state);
        }
    }
}

fn paint_fingerprint(h: &mut impl Hasher, paint: &PaintProperties) {
    let PaintProperties {
        background,
        corner_radius,
        opacity,
        transform,
        border_color,
        border_width,
        corner_radii_override,
        shadows,
        node_transform,
        clip_children,
    } = paint;
    color(h, background.current);
    num(h, corner_radius.current);
    num(h, opacity.current);
    affine(h, transform.current);
    color(h, border_color.current);
    num(h, border_width.current);
    if let Some(radii) = corner_radii_override {
        radii.current.0.iter().for_each(|r| num(h, *r));
    }
    for shadow in &shadows.current.0 {
        color(h, shadow.color);
        for value in [shadow.offset_x, shadow.offset_y, shadow.blur, shadow.spread] {
            num(h, value);
        }
    }
    let engine_core::NodeTransform {
        translate_x,
        translate_y,
        scale,
        rotation_deg,
    } = node_transform;
    for part in [translate_x, translate_y, scale, rotation_deg] {
        num(h, part.current);
    }
    clip_children.hash(h);
}

fn options_fingerprint(h: &mut impl Hasher, options: &TextOptions) {
    let TextOptions {
        italic,
        letter_spacing,
        wrap,
        max_lines,
        ellipsis,
    } = options;
    italic.hash(h);
    letter_spacing.to_bits().hash(h);
    wrap.hash(h);
    max_lines.hash(h);
    ellipsis.hash(h);
}

fn text_fingerprint(h: &mut impl Hasher, state: &TextState) {
    let TextState {
        content,
        font_family,
        font_weight,
        font_size,
        align,
        line_height,
        options,
    } = state;
    content.hash(h);
    font_family.hash(h);
    font_weight.to_bits().hash(h);
    font_size.to_bits().hash(h);
    std::mem::discriminant(align).hash(h);
    line_height.map(f32::to_bits).hash(h);
    options_fingerprint(h, options);
}

fn field_fingerprint(h: &mut impl Hasher, state: &TextFieldState) {
    let TextFieldState {
        content,
        font_family,
        font_weight,
        font_size,
        cursor,
        selection_anchor,
        preedit,
        text_tint,
        multiline,
        show_whitespace,
        syntax_spans,
        folded_ranges,
        // Where Up/Down aim; the caret's drawn position is `cursor`.
        goal_column: _,
        scroll_offset,
        horizontal_scroll_offset,
        placeholder,
        placeholder_fill,
        caret_color,
        selection_fill,
        obscured,
    } = state;
    content.hash(h);
    font_family.hash(h);
    font_weight.to_bits().hash(h);
    font_size.to_bits().hash(h);
    cursor.hash(h);
    selection_anchor.hash(h);
    preedit.hash(h);
    color(h, text_tint.current);
    multiline.hash(h);
    show_whitespace.hash(h);
    for (range, span_color) in syntax_spans {
        range.hash(h);
        color(h, *span_color);
    }
    folded_ranges.hash(h);
    num(h, scroll_offset.current);
    num(h, horizontal_scroll_offset.current);
    placeholder.hash(h);
    for fill in [placeholder_fill, caret_color, selection_fill] {
        fill.is_some().hash(h);
        if let Some(c) = fill {
            color(h, *c);
        }
    }
    obscured.hash(h);
}

fn image_fingerprint(h: &mut impl Hasher, state: &ImageState) {
    let ImageState { image, content_fit } = state;
    // A new frame is a new blob; hashing its bytes every frame would cost
    // megabytes.
    image.data.id().hash(h);
    image.width.hash(h);
    image.height.hash(h);
    std::mem::discriminant(&image.format).hash(h);
    std::mem::discriminant(&image.alpha_type).hash(h);
    std::mem::discriminant(content_fit).hash(h);
}

fn path_fingerprint(h: &mut impl Hasher, state: &PathState) {
    let PathState {
        data,
        view_box,
        trim_start,
        trim_end,
    } = state;
    bez_path(h, &data.current.0);
    view_box.is_some().hash(h);
    if let Some(r) = view_box {
        rect(h, *r);
    }
    num(h, trim_start.current);
    num(h, trim_end.current);
}

fn bez_path(h: &mut impl Hasher, path: &peniko::kurbo::BezPath) {
    use peniko::kurbo::PathEl;
    for element in path.elements() {
        std::mem::discriminant(element).hash(h);
        let points: &[peniko::kurbo::Point] = match element {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => std::slice::from_ref(p),
            PathEl::QuadTo(a, b) => &[*a, *b],
            PathEl::CurveTo(a, b, c) => &[*a, *b, *c],
            PathEl::ClosePath => &[],
        };
        for point in points {
            num(h, point.x);
            num(h, point.y);
        }
    }
}

fn canvas_fingerprint(h: &mut impl Hasher, state: &CanvasState) {
    let CanvasState {
        commands,
        // Where a pointer hits, not what's drawn.
        hit_test: _,
    } = state;
    for command in commands {
        std::mem::discriminant(command).hash(h);
        match command {
            DrawCommand::FillRect {
                x,
                y,
                width,
                height,
                color: c,
            } => {
                [*x, *y, *width, *height].iter().for_each(|v| num(h, *v));
                color(h, *c);
            }
            DrawCommand::FillCircle {
                cx,
                cy,
                radius,
                color: c,
            } => {
                [*cx, *cy, *radius].iter().for_each(|v| num(h, *v));
                color(h, *c);
            }
            DrawCommand::StrokePath {
                path,
                color: c,
                width,
            } => {
                bez_path(h, path);
                color(h, *c);
                num(h, *width);
            }
        }
    }
}

fn scroll_fingerprint(h: &mut impl Hasher, state: &ScrollViewState) {
    let ScrollViewState {
        scroll,
        horizontal,
        thumb_drag_anchor,
        scrollbar_fill,
        scrollbar_width,
        // Bookkeeping for the `scroll` event; paints nothing.
        reported: _,
    } = state;
    num(h, scroll.current);
    horizontal.hash(h);
    thumb_drag_anchor
        .map(|(a, b)| (a.to_bits(), b.to_bits()))
        .hash(h);
    scrollbar_fill.is_some().hash(h);
    if let Some(c) = scrollbar_fill {
        color(h, *c);
    }
    num(h, *scrollbar_width);
}

fn list_fingerprint(h: &mut impl Hasher, state: &VirtualListState) {
    let VirtualListState {
        item_count,
        item_extent,
        // Its rows are nodes, which fingerprint themselves.
        materialized: _,
        scroll_offset,
        // Moving rows are nodes that fingerprint themselves; the list's
        // own paint (its thumb) reads only the total extent, hashed below
        // -- not every offset, which a long list would pay for per frame.
        resolved_offsets: _,
        thumb_drag_anchor,
    } = state;
    item_count.hash(h);
    match item_extent {
        ItemExtent::Fixed(extent) => num(h, *extent),
        ItemExtent::Variable => 1u8.hash(h),
    }
    num(h, scroll_offset.current);
    num(h, state.total_extent());
    thumb_drag_anchor
        .map(|(a, b)| (a.to_bits(), b.to_bits()))
        .hash(h);
}

fn terminal_fingerprint(h: &mut impl Hasher, state: &TerminalState) {
    let TerminalState {
        cols,
        rows,
        cells,
        cursor_col,
        cursor_row,
        cursor_visible,
        font_family,
        font_size,
        selection_start,
        selection_end,
        palette,
    } = state;
    cols.hash(h);
    rows.hash(h);
    for cell in cells {
        let engine_core::TerminalCell {
            ch,
            fg,
            bg,
            bold,
            dim,
            italic,
            underline,
            inverse,
        } = cell;
        ch.hash(h);
        for cell_color in [fg, bg] {
            match cell_color {
                CellColor::Default => 0u8.hash(h),
                CellColor::Indexed(i) => (1u8, *i).hash(h),
                CellColor::Rgb(c) => {
                    2u8.hash(h);
                    color(h, *c);
                }
            }
        }
        (bold, dim, italic, underline, inverse).hash(h);
    }
    (cursor_col, cursor_row, cursor_visible).hash(h);
    font_family.hash(h);
    font_size.to_bits().hash(h);
    (selection_start, selection_end).hash(h);
    let engine_core::TerminalPalette {
        ansi,
        foreground,
        background,
        cursor,
        selection,
    } = palette;
    ansi.iter().for_each(|c| color(h, *c));
    for c in [foreground, background, cursor, selection] {
        color(h, *c);
    }
}

fn num(h: &mut impl Hasher, value: f64) {
    value.to_bits().hash(h);
}

fn color(h: &mut impl Hasher, value: Color) {
    value.components.map(f32::to_bits).hash(h);
}

fn rect(h: &mut impl Hasher, value: Rect) {
    [value.x0, value.y0, value.x1, value.y1]
        .iter()
        .for_each(|v| num(h, *v));
}

fn affine(h: &mut impl Hasher, value: Affine) {
    value.as_coeffs().iter().for_each(|v| num(h, *v));
}

/// `rect` grown to whole pixels.
fn round_out(rect: Rect) -> Rect {
    if rect.is_zero_area() {
        return Rect::ZERO;
    }
    Rect::new(
        rect.x0.floor(),
        rect.y0.floor(),
        rect.x1.ceil(),
        rect.y1.ceil(),
    )
}

/// Merges `rects` (D3): drops empty ones, merges overlapping ones, then
/// merges the pair whose union grows least until at most `MAX_RECTS`
/// remain -- re-merging any overlap a union creates; `Full` past
/// `FULL_FRACTION` of `window`.
fn merge(rects: Vec<Rect>, window: Rect) -> Damage {
    let mut rects: Vec<Rect> = rects
        .into_iter()
        .map(|r| r.intersect(window))
        .filter(|r| !r.is_zero_area())
        .collect();
    if rects.is_empty() {
        return Damage::None;
    }
    // Pair merging is cubic in the count; past `MAX_TRACKED` changes (a
    // whole grid animating), their bounding box stands in for them -- what
    // four rects covering that many changes would come close to anyway.
    if rects.len() > MAX_TRACKED {
        let bounds = rects.iter().fold(rects[0], |acc, r| acc.union(*r));
        rects = vec![bounds];
    }
    loop {
        merge_overlapping(&mut rects);
        if rects.len() <= MAX_RECTS {
            break;
        }
        let (mut best, mut cost) = ((0, 1), f64::INFINITY);
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let grown = rects[i].union(rects[j]).area() - rects[i].area() - rects[j].area();
                if grown < cost {
                    (best, cost) = ((i, j), grown);
                }
            }
        }
        rects[best.0] = rects[best.0].union(rects[best.1]);
        rects.swap_remove(best.1);
    }
    let covered: f64 = rects.iter().map(Rect::area).sum();
    if covered > window.area() * FULL_FRACTION {
        Damage::Full
    } else {
        Damage::Rects(rects)
    }
}

/// Unions overlapping rects until none overlap.
fn merge_overlapping(rects: &mut Vec<Rect>) {
    let mut i = 0;
    while i < rects.len() {
        let overlapping = (i + 1..rects.len()).find(|&j| rects[i].overlaps(rects[j]));
        match overlapping {
            Some(j) => {
                rects[i] = rects[i].union(rects[j]);
                rects.swap_remove(j);
                i = 0;
            }
            None => i += 1,
        }
    }
}
