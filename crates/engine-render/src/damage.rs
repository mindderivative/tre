//! 0.4.0 M4: which parts of a window changed since the last frame.
//!
//! **Compare, don't instrument.** Pixels change through `Tree::get_mut`,
//! direct field writes inside `Tree`, animation ticks, layout, and
//! tree-level state such as focus. Hooking every one of those paths would
//! make any missed one a stale-pixel bug, and partial redraw is on by
//! default. Instead `DamageTracker` walks the tree the way the paint walk
//! does -- the same visibility, culling, composed transforms, and clips,
//! through the helpers both walks share -- and records, per node, its
//! *painted rect* in window pixels and a *fingerprint* of everything that
//! decides its pixels. A node that's new, gone, or changed since the last
//! frame contributes its old and new painted rects.
//!
//! The fingerprint code destructures every state struct without `..`: a
//! field added later won't compile until someone decides whether it
//! affects paint.
//!
//! The rects then merge into a small set (the user's decision D3): any
//! that overlap merge, then the closest pairs until at most `MAX_RECTS`
//! remain; past `FULL_FRACTION` of the window, or on a first frame, a size
//! change, or a `reset`, the answer is a full redraw.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use engine_core::{
    CanvasState, CellColor, DrawCommand, ImageState, ItemExtent, Node, NodeId, NodeKind,
    PaintProperties, PathState, ScrollViewState, TerminalState, TextFieldState, TextOptions,
    TextState, Tree, VirtualListState,
};
use peniko::Color;
use peniko::kurbo::{Affine, Rect, Shape};

use crate::{TextRenderer, clips_children, composed_transform, transformed_bounds};

/// At most this many rects before the closest pairs merge.
pub const MAX_RECTS: usize = 4;
/// Damage covering more than this fraction of the window redraws it all.
pub const FULL_FRACTION: f64 = 0.5;
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

/// One frame's walk: the records it builds and what the walk needs.
struct Walk<'a> {
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
        let mut walk = Walk {
            tree,
            text,
            records: HashMap::with_capacity(self.records.len()),
        };
        walk.visit(root, Affine::IDENTITY, window, 1.0, None, 0);
        let current = walk.records;

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
                    rects.push(before.painted);
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

impl Walk<'_> {
    /// Mirrors `paint_node`: skipped when hidden, culled, or fully
    /// transparent, exactly as painting skips them.
    fn visit(
        &mut self,
        id: NodeId,
        parent_transform: Affine,
        visible: Rect,
        parent_opacity: f64,
        parent: Option<NodeId>,
        order: usize,
    ) {
        let tree = self.tree;
        let Some(node) = tree.get(id) else { return };
        if !node.visible {
            return;
        }
        let layout = tree.layout(id);
        let (w, h) = (f64::from(layout.size.width), f64::from(layout.size.height));
        let position = (f64::from(layout.location.x), f64::from(layout.location.y));
        let composed = composed_transform(parent_transform, position, node, w, h);
        let bounds = transformed_bounds(composed, Rect::new(0.0, 0.0, w, h));
        if !bounds.overlaps(visible) {
            return;
        }
        let opacity = node.paint.opacity.current;
        if opacity <= 0.0 {
            return;
        }
        let effective = parent_opacity * opacity;

        let local = self.local_painted(id, node, w, h);
        let mut painted = transformed_bounds(composed, local);
        if let NodeKind::TextField(state) = &node.kind
            && !state.multiline
        {
            // A single-line input's text scrolls sideways past its box,
            // unclipped: take its whole row across the visible width.
            painted = painted.union(Rect::new(visible.x0, bounds.y0, visible.x1, bounds.y1));
        }
        let painted = round_out(painted.inflate(MARGIN, MARGIN).intersect(visible));

        let mut hasher = std::hash::DefaultHasher::new();
        parent.hash(&mut hasher);
        order.hash(&mut hasher);
        affine(&mut hasher, composed);
        num(&mut hasher, effective);
        rect(&mut hasher, visible);
        num(&mut hasher, w);
        num(&mut hasher, h);
        node_fingerprint(&mut hasher, tree, id, node);
        self.records.insert(
            id,
            Record {
                painted,
                fingerprint: hasher.finish(),
            },
        );

        let child_visible = if clips_children(node) {
            visible.intersect(bounds)
        } else {
            visible
        };
        for (index, &child) in tree.children_in_paint_order(id).iter().enumerate() {
            self.visit(child, composed, child_visible, effective, Some(id), index);
        }
    }

    /// What `node` paints, in its own coordinates: its box, plus whatever
    /// reaches past it.
    fn local_painted(&mut self, id: NodeId, node: &Node, w: f64, h: f64) -> Rect {
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
                let (tw, th) =
                    self.text
                        .text_extent(state, w as f32, node.paint.background.current, id);
                // Glyphs overhang their advance a little (italics,
                // ascenders); a quarter of the font size covers it.
                let overhang = f64::from(state.font_size) * 0.25;
                local = local.union(
                    Rect::new(0.0, 0.0, f64::from(tw), f64::from(th)).inflate(overhang, overhang),
                );
            }
            NodeKind::Path(state) => {
                let (fill, stroke) = state.geometry(w, h);
                let half = node.paint.border_width.current / 2.0;
                local = local
                    .union(fill.bounding_box())
                    .union(stroke.bounding_box().inflate(half, half));
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
            _ => {}
        }
        local
    }
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
    } = node;
    visible.hash(h);
    z_index.hash(h);
    paint_fingerprint(h, paint);
    std::mem::discriminant(kind).hash(h);
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
        resolved_offsets,
        thumb_drag_anchor,
    } = state;
    item_count.hash(h);
    match item_extent {
        ItemExtent::Fixed(extent) => num(h, *extent),
        ItemExtent::Variable => 1u8.hash(h),
    }
    num(h, scroll_offset.current);
    for (index, offset) in resolved_offsets {
        index.hash(h);
        num(h, *offset);
    }
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
