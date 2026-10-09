//! 0.4.0 M4: which parts of a window changed since the last frame.
//!
//! **Compare what changed; instrument nothing that can be missed.** Pixels
//! change through `Tree::get_mut`, direct field writes inside `Tree`,
//! animation ticks, layout, and tree-level state such as focus. The tracker
//! records, per node, its *painted rect* in window pixels and a *fingerprint*
//! of everything that decides its pixels, and a node that is new, gone, or
//! changed since the last frame contributes its old and new painted rects.
//!
//! Two ways of finding those nodes:
//!
//! - **The full walk** visits every node with the paint walk's own traversal
//!   (`walk`: the same visibility, culling, composed transforms, and clips,
//!   written once) and compares each against its last record. It trusts
//!   nothing but the tree as it is now.
//! - **The incremental walk** (0.5.4, #125) visits only the nodes written to
//!   since the last call, and the nodes between them and the root, and keeps
//!   every other record. What makes that safe is that a write cannot go
//!   unnoticed: the tree's node map is private to `engine-core`'s `Tree` and
//!   notes every mutable access (`tree/nodes.rs`), so no write path can skip
//!   the note and still compile. Everything else a node's pixels follow from
//!   is rechecked: where it is placed (layout counts its own computations,
//!   and a new count means a full walk), what it is placed under (every node
//!   reached has its parent's transform, visible rect, opacity and index
//!   hashed and compared), focus, fonts, scale, size and the root. A shader or
//!   a backdrop blur paints from other nodes and the clock, and anything
//!   else that cannot be judged from the changed nodes alone, hands the call
//!   over to the full walk. The cost follows the changes, not the tree.
//!
//! **A scroll is a shift, not a change** (0.5.4, #126). Scrolling a view moves
//! every node under it by the same whole number of pixels, which as changes
//! would redraw the whole view. `plan_shift` recognises the case (one scroller
//! whose own change is only its offset, everything inside moved alike with its
//! content unchanged, nothing outside changed or painting over the view, an
//! opaque plain background, enough nodes moving to be worth it) and reports a
//! `Shift`: copy the view's kept pixels by that distance, then redraw only
//! what the copy does not explain (the uncovered strip, the scrollbar where it
//! was and is, and the old pixels of anything inside that did not move with
//! the rest). Any doubt returns `None` and the changes are ordinary damage.
//!
//! **Subtree extents** (0.5.4, #149). Each record also keeps the extent of its
//! whole subtree: a rect nothing it or its descendants paint falls outside.
//! The paint walk asks for them (`DamageTracker::extents`) and passes over any
//! child, before even placing it, whose extent misses every damage rect, so
//! building a partial redraw's scene costs the damage, not the tree (9,216
//! nodes, one changed: 515 us down to 27 us). An extent may be wider than the
//! exact union (it only grows while a node is passed through), never
//! narrower; verification checks that against the full walk's.
//!
//! `TRE_DAMAGE_VERIFY=1` (or `DamageTracker::with_verification`) runs both on
//! every call and panics if the incremental result misses anything the full
//! walk finds, or leaves a record that differs from it; CI runs the suites
//! that way. `TRE_DAMAGE_FULL=1` turns the incremental walk off.
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

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash, Hasher};

use engine_core::{
    CanvasState, CellColor, DrawCommand, ImageState, ItemExtent, Node, NodeId, NodeKind,
    PaintProperties, PathState, SCROLLBAR_MARGIN, SCROLLBAR_THICKNESS, ScrollViewState,
    TerminalState, TextFieldState, TextOptions, TextState, Tree, VirtualListState, fit_transform,
};
use peniko::Color;
use peniko::kurbo::{Affine, Rect, Shape};
use slotmap::SecondaryMap;

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
/// How many nodes must move with a scroll before it is carried by a copy
/// rather than redrawn. Measured (0.5.4, #126): a block copy of a view costs
/// about as much as redrawing a few dozen plain nodes, and wins only past a
/// couple of hundred; the paint walk's cost per node outside the damage (#149)
/// caps what it can win until that goes.
pub const SCROLL_BLIT_MIN_NODES: usize = 200;

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

/// What marks a node as a scroller: which way it scrolls.
#[derive(Clone, Copy, PartialEq)]
struct ScrollInfo {
    horizontal: bool,
}

/// 0.5.4 (#126): a scroller's content moved as a block. Copy `region` (window
/// pixels) of the kept frame by (`dx`, `dy`) device pixels before redrawing the
/// damage, which then holds only what the move does not explain: the strip it
/// uncovered, the scrollbar, and anything that changed besides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shift {
    pub region: Rect,
    pub dx: f64,
    pub dy: f64,
}

/// A node's record, as a change reports it.
#[derive(Clone, Copy)]
struct Snap {
    painted: Rect,
    /// What the node and everything under it paint (see `Record::extent`).
    extent: Rect,
    /// The node's `z_index`: a change moves its whole subtree among its
    /// siblings, not just the node.
    z_index: i32,
    /// How far a scroller was scrolled (0 for anything else).
    offset: f64,
    composed: Affine,
    content: u64,
    parent: Option<NodeId>,
    scroll: Option<ScrollInfo>,
}

/// One node's pixels changed (or it appeared, or it went).
struct Change {
    id: NodeId,
    before: Option<Snap>,
    after: Option<Snap>,
}

/// What the walk knows about one node as of the last frame.
#[derive(Clone)]
struct Record {
    painted: Rect,
    fingerprint: u64,
    /// 0.5.4 (#149): what the node and everything under it paint, together: a
    /// rect no pixel of the subtree falls outside, so a damage rect that
    /// misses it misses the whole subtree. After a change deep in the tree it
    /// may stay wider than the exact union until the node is walked again.
    extent: Rect,
    z_index: i32,
    /// How far a scroller was scrolled (0 for anything else).
    offset: f64,
    /// The node's box in window pixels, before margins and clipping.
    bounds: Rect,
    /// 0.5.4 (#126): what the node is, apart from where it is and how far its
    /// own contents are scrolled. Equal `content` and a moved `composed` means
    /// the node's pixels only moved.
    content: u64,
    parent: Option<NodeId>,
    scroll: Option<ScrollInfo>,
    // 0.5.4 (#125): the rest is what reaching this node's children takes,
    // without walking down to them from the root.
    composed: Affine,
    /// What the node leaves visible to its children.
    child_visible: Rect,
    /// The node's opacity times its ancestors'.
    opacity: f64,
    /// Hash of what the node was placed under: its parent's transform,
    /// visible rect and opacity, the parent and its index. A different
    /// value means the node must be walked again.
    context: u64,
    /// The children the walk reached, in paint order.
    children: Vec<NodeId>,
    /// The node's index among its parent's children when last walked: where
    /// to look for it first.
    order: usize,
    /// Whether the node's children were painted in an order other than their
    /// own (a nonzero `z_index` among them).
    sorted: bool,
}

impl Record {
    fn snap(&self) -> Snap {
        Snap {
            painted: self.painted,
            extent: self.extent,
            z_index: self.z_index,
            offset: self.offset,
            composed: self.composed,
            content: self.content,
            parent: self.parent,
            scroll: self.scroll,
        }
    }

    /// Whether the node paints the same as `other` did.
    fn same_pixels(&self, other: &Record) -> bool {
        self.painted == other.painted && self.fingerprint == other.fingerprint
    }
}

/// What a node is placed under, for `Record::context`.
fn context_hash(
    transform: Affine,
    visible: Rect,
    opacity: f64,
    parent: Option<NodeId>,
    order: usize,
) -> u64 {
    let mut hasher = FINGERPRINT.build_hasher();
    affine(&mut hasher, transform);
    rect(&mut hasher, visible);
    num(&mut hasher, opacity);
    parent.hash(&mut hasher);
    order.hash(&mut hasher);
    hasher.finish()
}

/// Whether painting `node` depends on other nodes or on the clock, so it
/// cannot be judged from its own state: a shader, or a backdrop blur.
fn is_special(node: &Node) -> bool {
    node.shader.is_some() || node.paint.backdrop_blur.current > 0.0
}

/// Remembers each node's painted rect and fingerprint from the last frame.
pub struct DamageTracker {
    records: SecondaryMap<NodeId, Record>,
    /// 0.5.4 (#104): the frame being built, kept between calls so its
    /// storage is reused instead of allocated each frame.
    spare: SecondaryMap<NodeId, Record>,
    size: Option<(u16, u16)>,
    /// 0.5.1 (#70): the window's clock, in seconds, which an animated
    /// shader's pixels depend on.
    time: f32,
    /// 0.5.4 (#102): the display scale; layout is logical, the painted rects
    /// and the window physical.
    scale: f64,
    /// 0.5.4 (#125): the state of the tree the records were made against, so
    /// the next call can tell what else besides touched nodes moved.
    layout_epoch: Option<u64>,
    focused: Option<NodeId>,
    fonts: u64,
    root: Option<NodeId>,
    /// The last full walk met a shader or a backdrop blur: such a tree is
    /// walked in full, as those paint from other nodes and the clock.
    special: bool,
    /// Walk only the nodes that changed (default). Off, every call walks the
    /// whole tree.
    incremental: bool,
    /// The nodes the last call walked.
    visited: usize,
    /// Debug aid: a second, full-walk tracker whose answer the incremental
    /// one must cover. See `with_verification`.
    verify: Option<Box<DamageTracker>>,
    /// 0.5.4 (#126): tell a scroll from a change (default; see `Shift`).
    scroll_blit: bool,
    /// A scroll is carried by a copy only when at least this many nodes move
    /// with it: below that, redrawing them costs less than the copies.
    scroll_blit_min_nodes: usize,
    shift: Option<Shift>,
}

/// One frame's walk (`walk::Visitor`): the records it builds and what it
/// needs to build them.
struct Recorder<'a> {
    tree: &'a Tree,
    time: f32,
    text: &'a mut TextRenderer,
    records: &'a mut SecondaryMap<NodeId, Record>,
    /// `Record::context` of the root, which has no parent record to read.
    root_context: u64,
    special: bool,
    visited: usize,
}

impl Default for DamageTracker {
    fn default() -> Self {
        let mut tracker = Self::default_plain();
        tracker.incremental = std::env::var_os("TRE_DAMAGE_FULL").is_none();
        tracker.scroll_blit = std::env::var_os("TRE_SCROLL_BLIT_OFF").is_none();
        if let Some(n) = std::env::var("TRE_SCROLL_BLIT_MIN_NODES")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            tracker.scroll_blit_min_nodes = n;
        }
        if std::env::var_os("TRE_DAMAGE_VERIFY").is_some() {
            tracker.verify = Some(Box::new(Self::full_only()));
        }
        tracker
    }
}

/// A call to the incremental walk that must hand over to the full one: it
/// met something it cannot judge from the changed nodes alone.
struct Bail;

impl DamageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// A tracker that always walks the whole tree.
    fn full_only() -> Self {
        Self {
            incremental: false,
            verify: None,
            scroll_blit: false,
            ..Self::default_plain()
        }
    }

    fn default_plain() -> Self {
        Self {
            records: SecondaryMap::new(),
            spare: SecondaryMap::new(),
            size: None,
            time: 0.0,
            scale: 1.0,
            layout_epoch: None,
            focused: None,
            fonts: 0,
            root: None,
            special: false,
            incremental: true,
            visited: 0,
            verify: None,
            scroll_blit: true,
            scroll_blit_min_nodes: SCROLL_BLIT_MIN_NODES,
            shift: None,
        }
    }

    /// A tracker that also walks the whole tree on every call, and panics
    /// when what the incremental walk reports misses a change the full walk
    /// finds. Set `TRE_DAMAGE_VERIFY` to get one from `new`, to run the test
    /// suites under it.
    pub fn with_verification() -> Self {
        Self {
            verify: Some(Box::new(Self::full_only())),
            ..Self::default_plain()
        }
    }

    /// A tracker that walks the whole tree on every call (no shortcut), for
    /// comparing against.
    pub fn without_shortcuts() -> Self {
        Self::full_only()
    }

    /// A tracker that walks the whole tree and never plans a scroll shift, to
    /// compare against.
    pub fn without_scroll_blit() -> Self {
        Self {
            scroll_blit: false,
            ..Self::default_plain()
        }
    }

    /// Sets how many nodes must move with a scroll for it to be carried by a
    /// copy (default `SCROLL_BLIT_MIN_NODES`); `0` always carries it.
    pub fn set_scroll_blit_min_nodes(&mut self, nodes: usize) {
        self.scroll_blit_min_nodes = nodes;
    }

    /// 0.5.4 (#149): what each node and its subtree paint, as of the last
    /// `damage` call, for a paint walk to skip a subtree that misses the damage.
    /// `None` when that cannot be trusted: nothing walked yet, or the tree has
    /// a shader or backdrop blur, which paint from other nodes.
    pub fn extents(&self) -> Option<Extents<'_>> {
        (!self.special && !self.records.is_empty()).then_some(Extents(&self.records))
    }

    /// The scroll shift the last `damage` call found, if any: apply it to the
    /// kept frame before redrawing that call's damage (which is `Rects`
    /// whenever this is `Some`).
    pub fn take_shift(&mut self) -> Option<Shift> {
        self.shift.take()
    }

    /// How many nodes the last `damage` call walked.
    pub fn visited(&self) -> usize {
        self.visited
    }

    /// Sets the time, in seconds, the next `damage` call sees: an animated
    /// shader's node is damaged whenever it changes.
    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
    }

    /// Sets the display scale (default 1). A change forgets the last frame,
    /// so the next `damage` is `Full`.
    pub fn set_scale(&mut self, scale: f64) {
        if self.scale != scale {
            self.scale = scale;
            self.reset();
        }
    }

    /// Forgets the last frame, so the next `damage` is `Full` -- for when
    /// the target's contents were lost (a recreated `PersistentTarget`).
    pub fn reset(&mut self) {
        self.records.clear();
        self.size = None;
        if let Some(shadow) = &mut self.verify {
            shadow.reset();
        }
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
        self.shift = None;
        let result = self.damage_unverified(tree, root, width, height, text);
        if let Some(mut shadow) = self.verify.take() {
            shadow.time = self.time;
            shadow.scale = self.scale;
            let full = shadow.damage(tree, root, width, height, text);
            // A scroll shift stands in for the moved nodes' rects: the pixel
            // tests are what check it. The records must agree regardless.
            assert!(
                self.shift.is_some() || covers(&result, &full, f64::from(width), f64::from(height)),
                "the incremental damage walk missed a change.\n  incremental: {result:?}\n  full walk:   {full:?}"
            );
            // The records themselves must agree too: a record the incremental
            // walk left stale would only show up as a miss later.
            if let Some(why) = self.records_differ(&shadow) {
                panic!("the incremental damage walk's records differ from the full walk's: {why}");
            }
            self.verify = Some(shadow);
        }
        result
    }

    /// Why this tracker's records differ from `full`'s, if they do.
    fn records_differ(&self, full: &DamageTracker) -> Option<String> {
        for (id, theirs) in &full.records {
            match self.records.get(id) {
                None => {
                    return Some(format!(
                        "{id:?} has no record; full walk: {:?}",
                        theirs.painted
                    ));
                }
                Some(mine) if !mine.extent.contains_rect(theirs.extent) => {
                    return Some(format!(
                        "{id:?} has the subtree extent {:?} here, which misses the full walk's {:?}",
                        mine.extent, theirs.extent
                    ));
                }
                Some(mine) if !mine.same_pixels(theirs) => {
                    return Some(format!(
                        "{id:?} paints {:?} here and {:?} in the full walk",
                        mine.painted, theirs.painted
                    ));
                }
                Some(_) => {}
            }
        }
        self.records
            .iter()
            .find(|(id, _)| !full.records.contains_key(*id))
            .map(|(id, mine)| {
                format!(
                    "{id:?} has a record ({:?}) the full walk does not",
                    mine.painted
                )
            })
    }

    fn damage_unverified(
        &mut self,
        tree: &Tree,
        root: NodeId,
        width: u16,
        height: u16,
        text: &mut TextRenderer,
    ) -> Damage {
        let window = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let epoch = tree.layout_epoch();
        let fonts = text.font_generation();
        let focused = tree.focused();
        // Always drained, so what a full walk covers is not counted twice.
        let touched = tree.take_touched();
        let first = self.size != Some((width, height));
        let usable = self.incremental
            && !first
            && !self.special
            && !touched.all
            && self.root == Some(root)
            && self.layout_epoch == Some(epoch)
            && self.fonts == fonts
            && !self.records.is_empty();
        let result = if usable {
            self.incremental_walk(tree, root, window, &touched, focused, text)
        } else {
            Err(Bail)
        };
        let changes = match result {
            Ok(changes) => Some(changes),
            Err(Bail) => self.full_walk(tree, root, window, width, height, text),
        };
        let damage = match changes {
            None => Damage::Full,
            Some(changes) => self.resolve(tree, changes, window),
        };
        self.layout_epoch = Some(epoch);
        self.focused = focused;
        self.fonts = fonts;
        self.root = Some(root);
        damage
    }

    /// The whole tree, compared with the last frame's records.
    fn full_walk(
        &mut self,
        tree: &Tree,
        root: NodeId,
        window: Rect,
        width: u16,
        height: u16,
        text: &mut TextRenderer,
    ) -> Option<Vec<Change>> {
        self.spare.clear();
        let mut recorder = Recorder {
            tree,
            time: self.time,
            text,
            records: &mut self.spare,
            root_context: context_hash(Affine::scale(self.scale), window, 1.0, None, 0),
            special: false,
            visited: 0,
        };
        walk::walk(tree, root, Affine::scale(self.scale), window, &mut recorder);
        self.special = recorder.special;
        self.visited = recorder.visited;

        let first = self.size != Some((width, height));
        // `spare` is now this frame; `records` (the last) becomes the next
        // frame's scratch.
        std::mem::swap(&mut self.records, &mut self.spare);
        let previous = &self.spare;
        self.size = Some((width, height));
        if first {
            return None;
        }

        let mut changes = Vec::new();
        for (id, now) in &self.records {
            match previous.get(id) {
                Some(before) if before.same_pixels(now) => {}
                before => changes.push(Change {
                    id,
                    before: before.map(Record::snap),
                    after: Some(now.snap()),
                }),
            }
        }
        for (id, before) in previous {
            if !self.records.contains_key(id) {
                changes.push(Change {
                    id,
                    before: Some(before.snap()),
                    after: None,
                });
            }
        }
        Some(changes)
    }

    /// The damage for `changes`: a scroll shift and the little it leaves, if
    /// the changes are one, else every changed node's old and new rect.
    fn resolve(&mut self, tree: &Tree, changes: Vec<Change>, window: Rect) -> Damage {
        if self.scroll_blit
            && !self.special
            && let Some((shift, rects)) = self.plan_shift(tree, &changes)
        {
            let damage = merge(rects, window);
            // Nothing to save when most of the window is redrawn anyway.
            if matches!(damage, Damage::Rects(_)) {
                self.shift = Some(shift);
                return damage;
            }
        }
        merge(plain_rects(&changes), window)
    }

    /// 0.5.4 (#126): whether `changes` are one scroller's content moving
    /// as a block, so that its pixels can be copied instead of redrawn.
    ///
    /// Everything about it is conservative: any doubt returns `None` and the
    /// changes become ordinary damage. A scroller qualifies when its own change
    /// is only its offset; every node under it that moved did so by the same
    /// whole number of device pixels with its own content unchanged (what is
    /// new, gone or different is redrawn where it now is, and where its old
    /// pixels land); and nothing outside it changed or paints over it. Its
    /// background must be opaque and plain, so that what lies behind the
    /// content does not matter, its box square-cornered and whole-pixel, and
    /// nothing from it up the tree blends, blurs, fades or rounds its clip.
    fn plan_shift(&self, tree: &Tree, changes: &[Change]) -> Option<(Shift, Vec<Rect>)> {
        let mut scrolled = changes.iter().filter(|c| match (&c.before, &c.after) {
            (Some(b), Some(a)) => {
                a.scroll.is_some()
                    && b.scroll == a.scroll
                    && b.content == a.content
                    && b.composed == a.composed
                    && b.parent == a.parent
            }
            _ => false,
        });
        let scrolled_change = scrolled.next()?;
        let scroller = scrolled_change.id;
        if scrolled.next().is_some() {
            return None;
        }
        let record = self.records.get(scroller)?;
        record.scroll?;
        let view = record.child_visible;
        let [a, b, c, d, _, _] = record.composed.as_coeffs();
        let whole = |v: f64| (v - v.round()).abs() < 1e-6;
        if b != 0.0
            || c != 0.0
            || a <= 0.0
            || d <= 0.0
            || !(whole(view.x0) && whole(view.y0) && whole(view.x1) && whole(view.y1))
            || view.area() <= 0.0
            // Clipped by an ancestor: the scrollbar and corners are not where
            // the box says.
            || view != record.bounds
            || (record.opacity - 1.0).abs() > 1e-9
            || !blittable(tree, scroller)
        {
            return None;
        }

        let parents: HashMap<NodeId, Option<NodeId>> = changes
            .iter()
            .filter_map(|c| c.after.or(c.before).map(|s| (c.id, s.parent)))
            .collect();
        let parent_of = |id: NodeId| -> Option<NodeId> {
            match parents.get(&id) {
                Some(parent) => *parent,
                None => self.records.get(id).and_then(|r| r.parent),
            }
        };
        let inside = |id: NodeId| -> bool {
            let mut current = parent_of(id);
            for _ in 0..4096 {
                match current {
                    Some(p) if p == scroller => return true,
                    Some(p) => current = parent_of(p),
                    None => return false,
                }
            }
            false
        };
        let mut above = HashSet::new();
        let mut current = parent_of(scroller);
        while let Some(p) = current {
            above.insert(p);
            current = parent_of(p);
        }

        // How far what is inside moved: all of it alike, in whole pixels.
        let mut delta: Option<(f64, f64)> = None;
        let mut moved: HashSet<NodeId> = HashSet::new();
        for change in changes {
            let (Some(before), Some(after)) = (&change.before, &change.after) else {
                continue;
            };
            if change.id == scroller
                || before.content != after.content
                || before.parent != after.parent
                || !inside(change.id)
            {
                continue;
            }
            let (was, now) = (before.composed.as_coeffs(), after.composed.as_coeffs());
            if was[..4] != now[..4] {
                continue;
            }
            let step = (now[4] - was[4], now[5] - was[5]);
            match delta {
                None => delta = Some(step),
                Some(first)
                    if (first.0 - step.0).abs() < 1e-6 && (first.1 - step.1).abs() < 1e-6 => {}
                // Not a block: some moved differently.
                Some(_) => return None,
            }
            moved.insert(change.id);
        }
        let (dx, dy) = delta?;
        if !(whole(dx) && whole(dy)) || (dx == 0.0 && dy == 0.0) {
            return None;
        }
        // Too little moves for the copy to pay.
        if moved.len() < self.scroll_blit_min_nodes {
            return None;
        }
        let (dx, dy) = (dx.round(), dy.round());
        let kept = view.intersect(view + peniko::kurbo::Vec2::new(dx, dy));
        // Worth it only if most of the view is carried over.
        if kept.width() <= 0.0 || kept.height() <= 0.0 || kept.area() < view.area() * 0.25 {
            return None;
        }

        // Nothing outside the scroller may have changed within its view, and
        // nothing but it, its ancestors (behind it) and its contents may paint
        // there: a block copy would carry such pixels along.
        let touches = |s: &Snap| s.painted.overlaps(view);
        for change in changes {
            if change.id == scroller || inside(change.id) {
                continue;
            }
            if change.before.iter().chain(change.after.iter()).any(touches) {
                return None;
            }
        }
        let changed_ids: HashSet<NodeId> = changes.iter().map(|c| c.id).collect();
        let mut rects: Vec<Rect> = Vec::new();
        for (id, other) in &self.records {
            if !other.painted.overlaps(view) || id == scroller || above.contains(&id) {
                continue;
            }
            if !inside(id) {
                return None;
            }
            // Content that stayed put (a sticky header) is not carried by the
            // move: it is drawn again where it is.
            // (A changed node is covered below, with where its old pixels went.)
            if !moved.contains(&id) && !changed_ids.contains(&id) {
                rects.push(other.painted);
                // ...and the copy carried its pixels along with the rest.
                let ghost = (other.painted + peniko::kurbo::Vec2::new(dx, dy)).intersect(view);
                if ghost.width() > 0.0 && ghost.height() > 0.0 {
                    rects.push(ghost);
                }
            }
        }

        // What each change leaves to redraw.
        for change in changes {
            if change.id == scroller {
                continue;
            }
            if !inside(change.id) {
                rects.extend(plain_rects(std::slice::from_ref(change)));
                continue;
            }
            if moved.contains(&change.id) {
                continue;
            }
            // (A node that changed its z_index changed its whole subtree.)
            let reordered = matches!(
                (&change.before, &change.after),
                (Some(b), Some(a)) if b.z_index != a.z_index
            );
            if let Some(after) = &change.after {
                rects.push(if reordered {
                    after.extent
                } else {
                    after.painted
                });
            }
            // The copy puts the old pixels of a node that did not move with
            // the rest where the rest went.
            if let Some(before) = &change.before {
                let was = if reordered {
                    before.extent
                } else {
                    before.painted
                };
                let ghost = (was + peniko::kurbo::Vec2::new(dx, dy)).intersect(view);
                if ghost.width() > 0.0 && ghost.height() > 0.0 {
                    rects.push(ghost);
                }
            }
        }
        // The strip the move uncovered.
        if dy > 0.0 {
            rects.push(Rect::new(view.x0, view.y0, view.x1, kept.y0));
        } else if dy < 0.0 {
            rects.push(Rect::new(view.x0, kept.y1, view.x1, view.y1));
        }
        if dx > 0.0 {
            rects.push(Rect::new(view.x0, view.y0, kept.x0, view.y1));
        } else if dx < 0.0 {
            rects.push(Rect::new(kept.x1, view.y0, view.x1, view.y1));
        }
        // The scrollbar's thumb moved, and the copy carried its old pixels:
        // where it was and where it is.
        let (before, after) = match (&scrolled_change.before, &scrolled_change.after) {
            (Some(b), Some(a)) => (b.offset, a.offset),
            _ => return None,
        };
        let moved_by = peniko::kurbo::Vec2::new(dx, dy);
        // (The old thumb's pixels are in the copy, `moved_by` along.)
        if let Some(thumb) = thumb_rect(tree, scroller, before, record.composed) {
            rects.push((thumb + moved_by).inflate(2.0, 2.0).intersect(view));
        }
        if let Some(thumb) = thumb_rect(tree, scroller, after, record.composed) {
            rects.push(thumb.inflate(2.0, 2.0).intersect(view));
        }
        Some((
            Shift {
                region: view,
                dx,
                dy,
            },
            rects,
        ))
    }

    /// 0.5.4 (#125): only the nodes that were written to since the last
    /// call, and what stands between them and the root.
    ///
    /// Nothing else can have changed pixels, because a node's pixels follow
    /// from its own state (reached only through the tree's node map, which
    /// notes every mutable access), its place (layout, which counts its own
    /// computations), what it is placed under (rechecked for every node
    /// reached), and a few tree-wide things (focus, fonts, scale, size, the
    /// root), all rechecked here. A shader or backdrop blur paints from other
    /// nodes and the clock: meeting one hands over to the full walk. So does
    /// anything else this cannot judge locally (`Bail`).
    fn incremental_walk(
        &mut self,
        tree: &Tree,
        root: NodeId,
        window: Rect,
        touched: &engine_core::Touched,
        focused: Option<NodeId>,
        text: &mut TextRenderer,
    ) -> Result<Vec<Change>, Bail> {
        let mut changed: HashSet<NodeId> = touched
            .ids
            .iter()
            .copied()
            .filter(|&id| tree.get(id).is_some())
            .collect();
        // The caret and cursor are drawn for the focused node only.
        if self.focused != focused {
            changed.extend(self.focused.into_iter().chain(focused));
        }
        let mut pass = Pass {
            tree,
            text,
            time: self.time,
            old: &self.records,
            dirty: HashMap::new(),
            registered: HashSet::new(),
            changed,
            updates: Vec::new(),
            removed: Vec::new(),
            changes: Vec::new(),
            visited: 0,
            scale: self.scale,
        };
        pass.register();
        let base = Affine::scale(self.scale);
        pass.node(root, base, window, 1.0, None, 0, false)?;
        let Pass {
            updates,
            removed,
            changes,
            visited,
            ..
        } = pass;
        for id in removed {
            self.records.remove(id);
        }
        for (id, record) in updates {
            self.records.insert(id, record);
        }
        self.visited = visited;
        Ok(changes)
    }
}

/// Each node's subtree extent (see `DamageTracker::extents`).
#[derive(Clone, Copy)]
pub struct Extents<'a>(&'a SecondaryMap<NodeId, Record>);

impl Extents<'_> {
    /// A rect that everything `id` and its descendants paint lies within.
    pub(crate) fn of(&self, id: NodeId) -> Option<Rect> {
        self.0.get(id).map(|r| r.extent)
    }
}

/// One call's incremental walk: reads the last frame's records, collects the
/// changes, and leaves applying them to the caller (so a `Bail` loses nothing).
struct Pass<'a> {
    tree: &'a Tree,
    text: &'a mut TextRenderer,
    time: f32,
    old: &'a SecondaryMap<NodeId, Record>,
    /// For each node on the way down to a changed one, the children that
    /// lead there.
    dirty: HashMap<NodeId, Vec<NodeId>>,
    /// Every node in `dirty`'s values, and the changed nodes.
    registered: HashSet<NodeId>,
    /// The nodes to walk again.
    changed: HashSet<NodeId>,
    updates: Vec<(NodeId, Record)>,
    removed: Vec<NodeId>,
    changes: Vec<Change>,
    visited: usize,
    /// The window's scale: what layout offsets snap to.
    scale: f64,
}

impl Pass<'_> {
    /// Marks every changed node and its ancestors, so the walk can find the
    /// changed ones from the root.
    fn register(&mut self) {
        let changed: Vec<NodeId> = self.changed.iter().copied().collect();
        for id in changed {
            let mut current = id;
            if !self.registered.insert(current) {
                continue;
            }
            while let Some(parent) = self.tree.get(current).and_then(|n| n.parent) {
                self.dirty.entry(parent).or_default().push(current);
                if !self.registered.insert(parent) {
                    break;
                }
                current = parent;
            }
        }
    }

    /// What `id` and everything under it paint, if it was reached: it has a
    /// record now.
    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        id: NodeId,
        parent_transform: Affine,
        visible: Rect,
        parent_opacity: f64,
        parent: Option<NodeId>,
        order: usize,
        force: bool,
    ) -> Result<Option<Rect>, Bail> {
        let context = context_hash(parent_transform, visible, parent_opacity, parent, order);
        let old = self.old.get(id);
        let again = force
            || self.changed.contains(&id)
            || old.is_none_or(|o| o.context != context)
            // A child with a z_index can reorder its siblings, and the order
            // is part of what each one is: all of them are looked at.
            || (self.registered.contains(&id) && !self.plain(id, old));
        if !again {
            if self.registered.contains(&id) {
                return Ok(Some(self.pass_through(id)?));
            }
            return Ok(old.map(|o| o.extent));
        }
        self.visited += 1;
        let Some(v) = walk::resolve(
            self.tree,
            id,
            parent_transform,
            visible,
            parent_opacity,
            parent,
            order,
            self.scale,
        ) else {
            if old.is_some() {
                self.remove_subtree(id)?;
            }
            return Ok(None);
        };
        if is_special(v.node) {
            return Err(Bail);
        }
        let (hasher, content, scroll, offset) = record_hasher(self.tree, self.time, &v);
        let painted = round_out(painted_rect(
            self.text, v.id, v.node, v.composed, v.w, v.h, v.bounds, v.visible,
        ));
        let mut record = Record {
            painted,
            fingerprint: hasher.finish(),
            extent: painted,
            z_index: v.node.z_index,
            offset,
            bounds: v.bounds,
            content,
            parent,
            scroll,
            composed: v.composed,
            child_visible: walk::child_visible(&v),
            opacity: v.opacity,
            context,
            children: Vec::new(),
            order,
            sorted: false,
        };
        match old {
            Some(before) if before.same_pixels(&record) => {}
            _ => self.changes.push(Change {
                id,
                before: old.map(Record::snap),
                after: Some(record.snap()),
            }),
        }
        // A scroller's children are placed by its offset, which a child
        // cannot see (a scroll is a paint-time shift, not a layout change, since
        // #105): they are all walked again.
        let force_children = force
            || (self.changed.contains(&id)
                && matches!(
                    v.node.kind,
                    NodeKind::ScrollView(_) | NodeKind::VirtualList(_)
                ));
        let children = self.tree.children_in_paint_order(id);
        record.sorted = has_z_order(self.tree, v.node);
        for (index, &child) in children.iter().enumerate() {
            if let Some(extent) = self.node(
                child,
                record.composed,
                record.child_visible,
                record.opacity,
                Some(id),
                index,
                force_children,
            )? {
                record.children.push(child);
                record.extent = record.extent.union(extent);
            }
        }
        if let Some(before) = old
            && before.children != record.children
        {
            let now: HashSet<NodeId> = record.children.iter().copied().collect();
            for &gone in &before.children {
                if now.contains(&gone) {
                    continue;
                }
                match self.tree.get(gone) {
                    // Still here, under this node: its own visit dropped it.
                    Some(node) if node.parent == Some(id) => {}
                    // Moved elsewhere in the tree: not judged here.
                    Some(_) => return Err(Bail),
                    None => self.remove_subtree(gone)?,
                }
            }
        }
        let extent = record.extent;
        self.updates.push((id, record));
        Ok(Some(extent))
    }

    /// Whether the children of `id` paint in their own order, and so none of
    /// the ones to look at has a z_index to reorder the others: only dirty
    /// children can have changed theirs, so the check is theirs alone, and
    /// the list is neither sorted nor scanned.
    fn plain(&self, id: NodeId, old: Option<&Record>) -> bool {
        let Some(own) = old else { return false };
        !own.sorted
            && self.dirty.get(&id).is_none_or(|dirty| {
                dirty
                    .iter()
                    .all(|c| self.tree.get(*c).is_some_and(|n| n.z_index == 0))
            })
    }

    /// A node nothing changed in itself, with changed nodes below it: walks
    /// down to just those, placing them under its own record.
    fn pass_through(&mut self, id: NodeId) -> Result<Rect, Bail> {
        let Some(own) = self.old.get(id) else {
            return Err(Bail);
        };
        let Some(dirty) = self.dirty.get(&id) else {
            return Ok(own.extent);
        };
        // `plain` held when this node was judged to need no more than its
        // dirty children, so paint order is the child order.
        let children: std::borrow::Cow<'_, [NodeId]> = match self.tree.get(id) {
            Some(node) => std::borrow::Cow::Borrowed(&node.children),
            None => return Err(Bail),
        };
        let place = |child: NodeId, old: &SecondaryMap<NodeId, Record>| -> Option<usize> {
            // Where it was last time, nearly always still right.
            let hint = old.get(child).map(|r| r.order);
            match hint {
                Some(i) if children.get(i) == Some(&child) => Some(i),
                _ => children.iter().position(|k| *k == child),
            }
        };
        let mut targets: Vec<(usize, NodeId)> = if dirty.len() <= 16 {
            dirty
                .iter()
                .filter_map(|c| place(*c, self.old).map(|i| (i, *c)))
                .collect()
        } else {
            let wanted: HashSet<NodeId> = dirty.iter().copied().collect();
            children
                .iter()
                .enumerate()
                .filter(|(_, k)| wanted.contains(k))
                .map(|(i, k)| (i, *k))
                .collect()
        };
        targets.sort_by_key(|(i, _)| *i);
        let (composed, child_visible, opacity) = (own.composed, own.child_visible, own.opacity);
        // Which of the dirty children were reached before is whether they
        // have a record; the list is only copied if that changes.
        let mut reached: Option<Vec<NodeId>> = None;
        // Only ever grows here (the unchanged children are not looked at, so
        // the exact union is not known): still a rect nothing falls outside.
        let mut extent = own.extent;
        for (index, child) in targets {
            let had = self.old.contains_key(child);
            let now = self.node(
                child,
                composed,
                child_visible,
                opacity,
                Some(id),
                index,
                false,
            )?;
            if let Some(child_extent) = now {
                extent = extent.union(child_extent);
            }
            if now.is_some() != had {
                let list = reached.get_or_insert_with(|| own.children.clone());
                if now.is_some() {
                    list.push(child);
                } else {
                    list.retain(|c| *c != child);
                }
            }
        }
        if reached.is_some() || extent != own.extent {
            let mut record = own.clone();
            if let Some(children) = reached {
                record.children = children;
            }
            record.extent = extent;
            self.updates.push((id, record));
        }
        Ok(extent)
    }

    /// `id` is no longer painted: its rect, and every node below it that was.
    fn remove_subtree(&mut self, id: NodeId) -> Result<(), Bail> {
        let Some(record) = self.old.get(id) else {
            return Ok(());
        };
        self.changes.push(Change {
            id,
            before: Some(record.snap()),
            after: None,
        });
        self.removed.push(id);
        for &child in &record.children {
            // A node of this subtree that now hangs somewhere else was
            // reparented, and would be walked there: not judged here.
            if let Some(node) = self.tree.get(child)
                && node.parent != Some(id)
            {
                return Err(Bail);
            }
            self.remove_subtree(child)?;
        }
        Ok(())
    }
}

/// Every change's old and new painted rect (once, when it did not move).
/// A node whose `z_index` changed paints its whole subtree in a new place among
/// its siblings, so its rects are the subtrees' extents: its children can
/// overflow it, and nothing about them changed to say so.
fn plain_rects(changes: &[Change]) -> Vec<Rect> {
    let mut rects = Vec::new();
    for change in changes {
        change_rects(change, &mut rects);
    }
    rects
}

fn change_rects(change: &Change, rects: &mut Vec<Rect>) {
    match (&change.before, &change.after) {
        (Some(before), Some(after)) if before.z_index != after.z_index => {
            rects.push(before.extent);
            rects.push(after.extent);
        }
        (Some(before), Some(after)) => {
            // A change in place (a colour, a glyph) needs its rect once, not
            // twice toward the `MAX_TRACKED` cap.
            if before.painted != after.painted {
                rects.push(before.painted);
            }
            rects.push(after.painted);
        }
        (Some(only), None) | (None, Some(only)) => rects.push(only.painted),
        (None, None) => {}
    }
}

/// Where a scroller's scrollbar thumb is, in window pixels, with its offset
/// at `offset`; `None` when it has nothing to scroll and shows none. The
/// geometry is the paint's own (`thumb_geometry_at`).
fn thumb_rect(tree: &Tree, scroller: NodeId, offset: f64, composed: Affine) -> Option<Rect> {
    let node = tree.get(scroller)?;
    let size = tree.layout(scroller).size;
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    let (horizontal, thickness, (track, thumb, along), content, viewport) = match &node.kind {
        NodeKind::ScrollView(state) => {
            let child = *node.children.first()?;
            let child_size = tree.layout(child).size;
            let (viewport, content) = if state.horizontal {
                (width, f64::from(child_size.width))
            } else {
                (height, f64::from(child_size.height))
            };
            (
                state.horizontal,
                state.scrollbar_width,
                state.thumb_geometry_at(viewport, content, offset),
                content,
                viewport,
            )
        }
        NodeKind::VirtualList(state) => (
            false,
            SCROLLBAR_THICKNESS,
            state.thumb_geometry_at(height, offset),
            state.total_extent(),
            height,
        ),
        _ => return None,
    };
    if content <= viewport || track <= 0.0 {
        return None;
    }
    let local = if horizontal {
        Rect::new(
            along,
            height - thickness - SCROLLBAR_MARGIN,
            along + thumb,
            height - SCROLLBAR_MARGIN,
        )
    } else {
        Rect::new(
            width - thickness - SCROLLBAR_MARGIN,
            along,
            width - SCROLLBAR_MARGIN,
            along + thumb,
        )
    };
    Some(composed.transform_rect_bbox(local))
}

/// Whether a scroller's pixels can be carried by a block copy: nothing behind
/// it shows through (an opaque plain background), its box has no rounded
/// corners or border to move, and nothing from it up the tree blends,
/// blurs, rounds a clip, or runs a shader.
fn blittable(tree: &Tree, scroller: NodeId) -> bool {
    let Some(node) = tree.get(scroller) else {
        return false;
    };
    let paint = &node.paint;
    let rounded =
        |n: &Node| n.paint.corner_radius.current > 0.0 || n.paint.corner_radii_override.is_some();
    if !node.visible
        || node.shader.is_some()
        || paint.gradient.is_some()
        || paint.border_gradient.is_some()
        || paint.background.current.components[3] < 1.0 - 1e-6
        || paint.border_width.current > 0.0
        || rounded(node)
        || paint.blur.current > 0.0
        || paint.backdrop_blur.current > 0.0
        || paint.blend != engine_core::Blend::Normal
        || paint.mask.is_some()
    {
        return false;
    }
    tree.ancestors(scroller).skip(1).all(|id| {
        tree.get(id).is_some_and(|n| {
            n.shader.is_none()
                && n.paint.blur.current <= 0.0
                && n.paint.backdrop_blur.current <= 0.0
                && n.paint.blend == engine_core::Blend::Normal
                && n.paint.mask.is_none()
                && !(crate::clips_children(n) && rounded(n))
        })
    })
}

/// Whether any of `node`'s children has a nonzero `z_index`, so that they
/// paint in an order other than their own.
fn has_z_order(tree: &Tree, node: &Node) -> bool {
    node.children
        .iter()
        .any(|c| tree.get(*c).is_some_and(|n| n.z_index != 0))
}

/// Whether `result` covers everything `full` reports (a change reported
/// twice, or reported more broadly, is fine; a change missed is not).
fn covers(result: &Damage, full: &Damage, width: f64, height: f64) -> bool {
    match (result, full) {
        (_, Damage::None) => true,
        (Damage::Full, _) => true,
        (Damage::None, _) => false,
        (Damage::Rects(_), Damage::Full) => {
            let Damage::Rects(rects) = result else {
                return false;
            };
            rects.iter().map(Rect::area).sum::<f64>() >= width * height - 1e-6
        }
        (Damage::Rects(mine), Damage::Rects(theirs)) => theirs.iter().all(|want| {
            // The rects do not overlap, so their overlaps with `want` add up.
            let got: f64 = mine
                .iter()
                .map(|r| r.intersect(*want))
                .filter(|r| r.width() > 0.0 && r.height() > 0.0)
                .map(|r| r.area())
                .sum();
            got >= want.area() - 1e-6
        }),
    }
}

/// The hashes a node's record is made of: its `fingerprint` so far (what a
/// record's whole identity is, still open for more), its `content` (what the
/// node is, apart from where it is and how far its own contents are
/// scrolled), and what tells a scroller. The same for the full walk and the
/// incremental one.
fn record_hasher(
    tree: &Tree,
    time: f32,
    v: &walk::Visit<'_>,
) -> (impl Hasher, u64, Option<ScrollInfo>, f64) {
    // Fast and fixed-seed: a frame's fingerprints compare with the last
    // frame's, and nothing adversarial picks what's hashed.
    let mut content = FINGERPRINT.build_hasher();
    num(&mut content, v.opacity);
    rect(&mut content, v.visible);
    num(&mut content, v.w);
    num(&mut content, v.h);
    node_fingerprint_with(&mut content, tree, time, v.id, v.node, false);
    let content = content.finish();
    let (offset, scroll) = match &v.node.kind {
        NodeKind::ScrollView(state) => (
            state.scroll.current,
            Some(ScrollInfo {
                horizontal: state.horizontal,
            }),
        ),
        NodeKind::VirtualList(state) => (
            state.scroll_offset.current,
            Some(ScrollInfo { horizontal: false }),
        ),
        _ => (0.0, None),
    };
    let mut hasher = FINGERPRINT.build_hasher();
    v.parent.hash(&mut hasher);
    v.order.hash(&mut hasher);
    affine(&mut hasher, v.composed);
    content.hash(&mut hasher);
    num(&mut hasher, offset);
    (hasher, content, scroll, offset)
}

impl<'t> walk::Visitor<'t> for Recorder<'_> {
    /// The subtree is recorded: its extent is the node's own painted rect and
    /// its children's extents together.
    fn leave(&mut self, v: &walk::Visit<'t>) {
        let Some(own) = self.records.get(v.id) else {
            return;
        };
        let (mut extent, count) = (own.painted, own.children.len());
        for i in 0..count {
            let child = self.records[v.id].children[i];
            if let Some(record) = self.records.get(child) {
                extent = extent.union(record.extent);
            }
        }
        self.records[v.id].extent = extent;
    }

    /// Records every node the paint walk reaches -- the same `walk`, so
    /// the two can't disagree about which nodes are drawn.
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        self.visited += 1;
        let painted = round_out(painted_rect(
            self.text, v.id, v.node, v.composed, v.w, v.h, v.bounds, v.visible,
        ));
        // Fast and fixed-seed: a frame's fingerprints compare with the
        // last frame's, and nothing adversarial picks what's hashed.
        let (mut hasher, content, scroll, offset) = record_hasher(self.tree, self.time, v);
        // 0.5.4 (#110): a backdrop blur shows what is behind it, so anything
        // painted before it that reaches its box or the blur's reach is part
        // of what it paints. The records so far are exactly those nodes.
        let sigma = v.node.paint.backdrop_blur.current;
        if sigma > 0.0 {
            let reach = v.bounds.inflate(sigma * 3.0, sigma * 3.0);
            for (id, record) in self.records.iter() {
                if record.painted.overlaps(reach) {
                    id.hash(&mut hasher);
                    record.fingerprint.hash(&mut hasher);
                    rect(&mut hasher, record.painted);
                }
            }
        }
        self.special |= is_special(v.node);
        let context = match v.parent.and_then(|p| self.records.get(p)) {
            Some(parent) => context_hash(
                parent.composed,
                parent.child_visible,
                parent.opacity,
                v.parent,
                v.order,
            ),
            None => self.root_context,
        };
        if let Some(parent) = v.parent.and_then(|p| self.records.get_mut(p)) {
            parent.children.push(v.id);
        }
        self.records.insert(
            v.id,
            Record {
                painted,
                fingerprint: hasher.finish(),
                extent: painted,
                z_index: v.node.z_index,
                offset,
                bounds: v.bounds,
                content,
                parent: v.parent,
                scroll,
                composed: v.composed,
                child_visible: walk::child_visible(v),
                opacity: v.opacity,
                context,
                children: Vec::new(),
                order: v.order,
                sorted: has_z_order(self.tree, v.node),
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
    // Most nodes paint nothing outside their box, and the walk has already
    // bounded that box under `composed` (0.5.4, #104).
    let mut painted = if local == Rect::new(0.0, 0.0, w, h) {
        bounds
    } else {
        transformed_bounds(composed, local)
    };
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
    // 0.5.4 (#110): a blur spreads the node's pixels about three standard
    // deviations past its box.
    let reach = node.paint.blur.current * 3.0;
    if reach > 0.0 {
        local = local.inflate(reach, reach);
    }
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
            let (tw, th) = text.text_extent(state, w as f32, id);
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
    time: f32,
    shader: &std::sync::Arc<engine_core::Shader>,
    depth: usize,
) {
    1u8.hash(h);
    (std::sync::Arc::as_ptr(shader) as usize).hash(h);
    shader.mode().hash(h);
    shader.animated().hash(h);
    shader.versions().hash(h);
    // An animated shader paints differently every frame: the time is part
    // of what it painted.
    if shader.animated() {
        time.to_bits().hash(h);
    }
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
                    shader_fingerprint(h, tree, time, inner, depth + 1);
                }
                _ => 3u8.hash(h),
            },
        }
    }
}

/// 0.5.1 (#69): what `id`'s descendants paint, for an effect node: each one's
/// place in the tree, its box, and everything `node_fingerprint` hashes.
fn subtree_fingerprint(h: &mut impl Hasher, tree: &Tree, time: f32, id: NodeId) {
    for (index, &child) in tree.children_in_paint_order(id).iter().enumerate() {
        let Some(node) = tree.get(child) else {
            continue;
        };
        index.hash(h);
        let layout = tree.layout(child);
        let (sx, sy) = tree.scroll_shift(child);
        for v in [
            layout.location.x + sx as f32,
            layout.location.y + sy as f32,
            layout.size.width,
            layout.size.height,
        ] {
            v.to_bits().hash(h);
        }
        node_fingerprint(h, tree, time, child, node);
        subtree_fingerprint(h, tree, time, child);
    }
}

/// 0.5.1 (#69): the fingerprint of an effect node's subtree alone -- what its
/// offscreen content render depends on -- as a pass's staleness key.
pub(crate) fn effect_content_fingerprint(tree: &Tree, time: f32, id: NodeId) -> u64 {
    let mut hasher = FINGERPRINT.build_hasher();
    subtree_fingerprint(&mut hasher, tree, time, id);
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
fn node_fingerprint(h: &mut impl Hasher, tree: &Tree, time: f32, id: NodeId, node: &Node) {
    node_fingerprint_with(h, tree, time, id, node, true);
}

/// `node_fingerprint`, optionally without how far a scroller is scrolled
/// (hashed apart, so a node whose only change is a scroll can be told).
fn node_fingerprint_with(
    h: &mut impl Hasher,
    tree: &Tree,
    time: f32,
    id: NodeId,
    node: &Node,
    with_offset: bool,
) {
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
        // 0.5.4 (#139): where a sticky node is reaches paint through its composed
        // transform, which is hashed above.
        sticky: _,
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
            shader_fingerprint(h, tree, time, shader, 0);
            // An effect's result depends on its whole subtree, so a change
            // anywhere in it repaints the node's box.
            if shader.mode() == engine_core::ShaderMode::Effect {
                subtree_fingerprint(h, tree, time, id);
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
            | NodeKind::Svg(_)
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
        // A document is immutable; a new one is a new revision.
        NodeKind::Svg(state) => state.document.revision.hash(h),
        NodeKind::Canvas(state) => canvas_fingerprint(h, state),
        NodeKind::ScrollView(state) => {
            scroll_fingerprint(h, state, with_offset);
            // The scrollbar thumb's size follows the content's.
            for &child in &node.children {
                let size = tree.layout(child).size;
                size.width.to_bits().hash(h);
                size.height.to_bits().hash(h);
            }
        }
        NodeKind::VirtualList(state) => list_fingerprint(h, state, with_offset),
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
        gradient,
        border_gradient,
        blur,
        blend,
        backdrop_blur,
        mask,
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
    // 0.5.4 (#110): blur, blend mode and backdrop blur.
    num(h, blur.current);
    (*blend as u8).hash(h);
    num(h, backdrop_blur.current);
    // 0.5.4 (#109): a gradient is part of the fill.
    match gradient {
        None => 0u8.hash(h),
        Some(gradient) => {
            1u8.hash(h);
            gradient_fingerprint(h, &gradient.current);
        }
    }
    // 0.5.4 (#129): and one is part of the border.
    optional_gradient_fingerprint(h, border_gradient.as_deref());
    // 0.5.6 (#164): and a mask is the shape of everything the node paints.
    match mask.as_deref() {
        None => 0u8.hash(h),
        Some(engine_core::Mask::Circle) => 1u8.hash(h),
        Some(engine_core::Mask::Rounded(radii)) => {
            2u8.hash(h);
            radii.0.iter().for_each(|r| num(h, *r));
        }
        Some(engine_core::Mask::Path { data, view_box }) => {
            3u8.hash(h);
            data.to_svg().hash(h);
            for value in [view_box.x0, view_box.y0, view_box.x1, view_box.y1] {
                num(h, value);
            }
        }
    }
}

fn optional_gradient_fingerprint(h: &mut impl Hasher, gradient: Option<&engine_core::Gradient>) {
    match gradient {
        None => 0u8.hash(h),
        Some(gradient) => {
            1u8.hash(h);
            gradient_fingerprint(h, gradient);
        }
    }
}

fn gradient_fingerprint(h: &mut impl Hasher, gradient: &engine_core::Gradient) {
    use engine_core::GradientShape;
    match gradient.shape() {
        GradientShape::Linear { angle_deg } => {
            0u8.hash(h);
            num(h, *angle_deg);
        }
        GradientShape::Radial { center, radius } => {
            1u8.hash(h);
            num(h, center.0);
            num(h, center.1);
            num(h, *radius);
        }
        GradientShape::Sweep { center, start_deg } => {
            2u8.hash(h);
            num(h, center.0);
            num(h, center.1);
            num(h, *start_deg);
        }
    }
    for stop in gradient.stops() {
        stop.offset.to_bits().hash(h);
        color(h, stop.color);
    }
}

fn options_fingerprint(h: &mut impl Hasher, options: &TextOptions) {
    let TextOptions {
        italic,
        letter_spacing,
        wrap,
        max_lines,
        ellipsis,
        spans,
        selectable,
        selection,
    } = options;
    italic.hash(h);
    letter_spacing.to_bits().hash(h);
    wrap.hash(h);
    max_lines.hash(h);
    ellipsis.hash(h);
    // 0.5.4 (#112): the selection is paint (the highlight); selectable is not.
    let _ = selectable;
    selection.hash(h);
    // 0.5.4 (#112): spans.
    for span in spans {
        let engine_core::TextSpan {
            start,
            end,
            color: span_color,
            weight,
            italic,
            underline,
            strikethrough,
            font_size,
            font_family,
            link,
        } = span;
        start.hash(h);
        end.hash(h);
        match span_color {
            Some(c) => {
                1u8.hash(h);
                color(h, *c);
            }
            None => 0u8.hash(h),
        }
        weight.map(f32::to_bits).hash(h);
        italic.hash(h);
        underline.hash(h);
        strikethrough.hash(h);
        font_size.map(f32::to_bits).hash(h);
        font_family.hash(h);
        // A link changes how the text answers a press, not how it looks.
        let _ = link;
    }
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
                gradient,
            } => {
                [*x, *y, *width, *height].iter().for_each(|v| num(h, *v));
                color(h, *c);
                optional_gradient_fingerprint(h, gradient.as_ref());
            }
            DrawCommand::FillCircle {
                cx,
                cy,
                radius,
                color: c,
                gradient,
            } => {
                [*cx, *cy, *radius].iter().for_each(|v| num(h, *v));
                color(h, *c);
                optional_gradient_fingerprint(h, gradient.as_ref());
            }
            DrawCommand::StrokePath {
                path,
                color: c,
                width,
                gradient,
            } => {
                bez_path(h, path);
                color(h, *c);
                num(h, *width);
                optional_gradient_fingerprint(h, gradient.as_ref());
            }
        }
    }
}

fn scroll_fingerprint(h: &mut impl Hasher, state: &ScrollViewState, with_offset: bool) {
    let ScrollViewState {
        scroll,
        horizontal,
        thumb_drag_anchor,
        scrollbar_fill,
        scrollbar_width,
        // Bookkeeping for the `scroll` event; paints nothing.
        reported: _,
    } = state;
    if with_offset {
        num(h, scroll.current);
    }
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

fn list_fingerprint(h: &mut impl Hasher, state: &VirtualListState, with_offset: bool) {
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
    if with_offset {
        num(h, scroll_offset.current);
    }
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
