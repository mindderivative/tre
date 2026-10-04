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
    PaintProperties, PathState, ScrollViewState, TerminalState, TextFieldState, TextOptions,
    TextState, Tree, VirtualListState, fit_transform,
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

/// What the walk knows about one node as of the last frame.
#[derive(Clone)]
struct Record {
    painted: Rect,
    fingerprint: u64,
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
        let result = self.damage_unverified(tree, root, width, height, text);
        if let Some(mut shadow) = self.verify.take() {
            shadow.time = self.time;
            shadow.scale = self.scale;
            let full = shadow.damage(tree, root, width, height, text);
            assert!(
                covers(&result, &full, f64::from(width), f64::from(height)),
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
        let damage = match result {
            Ok(damage) => damage,
            Err(Bail) => self.full_walk(tree, root, window, width, height, text),
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
    ) -> Damage {
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
            return Damage::Full;
        }

        let mut rects = Vec::new();
        for (id, now) in &self.records {
            match previous.get(id) {
                Some(before) if before.same_pixels(now) => {}
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
        for (id, before) in previous {
            if !self.records.contains_key(id) {
                rects.push(before.painted);
            }
        }
        merge(rects, window)
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
    ) -> Result<Damage, Bail> {
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
            rects: Vec::new(),
            visited: 0,
        };
        pass.register();
        let base = Affine::scale(self.scale);
        pass.node(root, base, window, 1.0, None, 0, false)?;
        let Pass {
            updates,
            removed,
            rects,
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
        Ok(merge(rects, window))
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
    rects: Vec<Rect>,
    visited: usize,
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

    /// Whether `id` was reached: it has a record now.
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
    ) -> Result<bool, Bail> {
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
                self.pass_through(id)?;
            }
            return Ok(true);
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
        ) else {
            if old.is_some() {
                self.remove_subtree(id)?;
            }
            return Ok(false);
        };
        if is_special(v.node) {
            return Err(Bail);
        }
        let mut record = Record {
            painted: round_out(painted_rect(
                self.text, v.id, v.node, v.composed, v.w, v.h, v.bounds, v.visible,
            )),
            fingerprint: {
                let mut hasher = FINGERPRINT.build_hasher();
                fingerprint_placement(&mut hasher, &v);
                node_fingerprint(&mut hasher, self.tree, self.time, v.id, v.node);
                hasher.finish()
            },
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
            Some(before) => {
                if before.painted != record.painted {
                    self.rects.push(before.painted);
                }
                self.rects.push(record.painted);
            }
            None => self.rects.push(record.painted),
        }
        // A scroller's children are placed by its offset, which a child
        // cannot see: they are all walked again. (Today a scroll also moves
        // layout, so the layout epoch has already sent such a frame to the
        // full walk; this holds if that ever stops being so.)
        let force_children = force
            || (self.changed.contains(&id)
                && matches!(
                    v.node.kind,
                    NodeKind::ScrollView(_) | NodeKind::VirtualList(_)
                ));
        let children = self.tree.children_in_paint_order(id);
        record.sorted = has_z_order(self.tree, v.node);
        for (index, &child) in children.iter().enumerate() {
            if self.node(
                child,
                record.composed,
                record.child_visible,
                record.opacity,
                Some(id),
                index,
                force_children,
            )? {
                record.children.push(child);
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
        self.updates.push((id, record));
        Ok(true)
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
    fn pass_through(&mut self, id: NodeId) -> Result<(), Bail> {
        let Some(own) = self.old.get(id) else {
            return Err(Bail);
        };
        let Some(dirty) = self.dirty.get(&id) else {
            return Ok(());
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
            if now != had {
                let list = reached.get_or_insert_with(|| own.children.clone());
                if now {
                    list.push(child);
                } else {
                    list.retain(|c| *c != child);
                }
            }
        }
        if let Some(children) = reached {
            let mut record = own.clone();
            record.children = children;
            self.updates.push((id, record));
        }
        Ok(())
    }

    /// `id` is no longer painted: its rect, and every node below it that was.
    fn remove_subtree(&mut self, id: NodeId) -> Result<(), Bail> {
        let Some(record) = self.old.get(id) else {
            return Ok(());
        };
        self.rects.push(record.painted);
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

/// What places a node: its parent, index, transform, opacity, visible rect and
/// size. The same for the full walk and the incremental one.
fn fingerprint_placement(hasher: &mut impl Hasher, v: &walk::Visit<'_>) {
    v.parent.hash(hasher);
    v.order.hash(hasher);
    affine(hasher, v.composed);
    num(hasher, v.opacity);
    rect(hasher, v.visible);
    num(hasher, v.w);
    num(hasher, v.h);
}

impl<'t> walk::Visitor<'t> for Recorder<'_> {
    /// Records every node the paint walk reaches -- the same `walk`, so
    /// the two can't disagree about which nodes are drawn.
    fn enter(&mut self, v: &walk::Visit<'t>) -> bool {
        self.visited += 1;
        let painted = round_out(painted_rect(
            self.text, v.id, v.node, v.composed, v.w, v.h, v.bounds, v.visible,
        ));
        // Fast and fixed-seed: a frame's fingerprints compare with the
        // last frame's, and nothing adversarial picks what's hashed.
        let mut hasher = FINGERPRINT.build_hasher();
        fingerprint_placement(&mut hasher, v);
        node_fingerprint(&mut hasher, self.tree, self.time, v.id, v.node);
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
        gradient,
        blur,
        blend,
        backdrop_blur,
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
