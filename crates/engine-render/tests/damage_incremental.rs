//! 0.5.4 (#125): the incremental damage walk, checked against the full one.
//!
//! The walk that visits only changed nodes is only safe if it never misses a
//! change the full walk would find. A `DamageTracker::with_verification` runs
//! both on every call and panics on a miss, so these tests just drive trees
//! through many random edits: colours, moves, opacity, visibility, z order,
//! sizes (layout), reparenting, removal, new nodes, clipping, text, scrolling,
//! sticky nodes, borders, shadows, a backdrop blur (which hands over to the
//! full walk). Failures print the seed, frame and edits, so one can be
//! replayed by running that seed alone.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use engine_core::{
    ItemExtent, NodeId, NodeKind, PaintProperties, ScrollViewState, TextFieldState, TextState,
    Tree, VirtualListState,
};
use engine_render::{Damage, DamageTracker, TextRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

const W: u16 = 400;
const H: u16 = 300;

/// xorshift64*: enough for picking edits, and the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 1000) as f32 / 1000.0
    }
}

fn color(rng: &mut Rng) -> Color {
    Color::from_rgba8(
        rng.below(256) as u8,
        rng.below(256) as u8,
        rng.below(256) as u8,
        255,
    )
}

fn style(rng: &mut Rng, absolute: bool) -> Style {
    let (w, h) = (10.0 + 80.0 * rng.unit(), 10.0 + 60.0 * rng.unit());
    Style {
        position: if absolute {
            Position::Absolute
        } else {
            Position::Relative
        },
        inset: TaffyRect {
            left: length(380.0 * rng.unit()),
            top: length(280.0 * rng.unit()),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

struct World {
    tree: Tree,
    root: NodeId,
    all: Vec<NodeId>,
    /// The list and its rows, which the list owns: not removed or moved here.
    owned: Vec<NodeId>,
    text: TextRenderer,
    tracker: DamageTracker,
    rng: Rng,
}

impl World {
    fn new(seed: u64) -> Self {
        let mut tree = Tree::new();
        let root = tree.insert(
            NodeKind::Container,
            Style {
                size: Size {
                    width: length(f32::from(W)),
                    height: length(f32::from(H)),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(30, 30, 30, 255), 0.0, 1.0),
        );
        let mut world = Self {
            tree,
            root,
            all: vec![root],
            owned: Vec::new(),
            text: TextRenderer::new(),
            tracker: DamageTracker::with_verification(),
            rng: Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
        };
        for _ in 0..(12 + world.rng.below(30)) {
            world.add_node();
        }
        world.add_virtual_list();
        world
    }

    /// A virtual list with its rows built, some of them sticky: the scroller
    /// whose offset moves its children without any layout changing.
    fn add_virtual_list(&mut self) {
        let list = self.tree.insert(
            NodeKind::VirtualList(VirtualListState::new(40, ItemExtent::Fixed(30.0))),
            Style {
                position: Position::Absolute,
                inset: TaffyRect {
                    left: length(60.0),
                    top: length(40.0),
                    right: auto(),
                    bottom: auto(),
                },
                size: Size {
                    width: length(200.0),
                    height: length(150.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(60, 60, 90, 255), 0.0, 1.0),
        );
        self.tree.add_child(self.root, list);
        self.tree.set_virtual_list_window(list, 0..8, |i| {
            (
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(200.0),
                        height: length(30.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(
                    Color::from_rgba8((i * 30) as u8, 120, 200 - (i * 20) as u8, 255),
                    0.0,
                    1.0,
                ),
            )
        });
        self.all.push(list);
        self.owned.push(list);
        let rows: Vec<NodeId> = self
            .tree
            .get(list)
            .map_or(Vec::new(), |n| n.children.clone());
        for (i, row) in rows.iter().enumerate() {
            if i % 3 == 0 {
                self.tree.set_sticky(*row, Some(0.0));
            }
            self.all.push(*row);
            self.owned.push(*row);
        }
    }

    /// A parent that can hold children: the root, or any container.
    fn container(&mut self) -> NodeId {
        for _ in 0..8 {
            let id = self.all[self.rng.below(self.all.len())];
            if let Some(node) = self.tree.get(id)
                && matches!(
                    node.kind,
                    NodeKind::Container | NodeKind::Rect | NodeKind::ScrollView(_)
                )
            {
                return id;
            }
        }
        self.root
    }

    fn add_node(&mut self) -> String {
        let parent = self.container();
        let absolute = self.rng.below(3) != 0;
        let (kind, label) = match self.rng.below(10) {
            0 => (NodeKind::Container, "container"),
            1 => (
                NodeKind::Text(TextState {
                    content: "hello world".into(),
                    font_family: "Roboto".into(),
                    font_weight: 400.0,
                    font_size: 14.0,
                    align: Default::default(),
                    line_height: None,
                    options: Default::default(),
                }),
                "text",
            ),
            2 => (NodeKind::ScrollView(ScrollViewState::new(false)), "scroll"),
            3 => (
                NodeKind::TextField(Box::new(TextFieldState::new("Hi", "Roboto", 400.0, 16.0))),
                "text input",
            ),
            _ => (NodeKind::Rect, "rect"),
        };
        let scroll = matches!(kind, NodeKind::ScrollView(_));
        let st = style(&mut self.rng, absolute && !scroll);
        let fill = color(&mut self.rng);
        let id = self
            .tree
            .insert(kind, st, PaintProperties::new(fill, 0.0, 1.0));
        self.tree.add_child(parent, id);
        self.all.push(id);
        format!("add {label} under {parent:?}")
    }

    fn alive(&mut self) -> Option<NodeId> {
        self.all.retain(|id| self.tree.get(*id).is_some());
        if self.all.len() < 2 {
            return None;
        }
        // Never the root: it is the tree.
        Some(self.all[1 + self.rng.below(self.all.len() - 1)])
    }

    /// One random edit; says what it did.
    fn edit(&mut self) -> String {
        let Some(id) = self.alive() else {
            return self.add_node();
        };
        let r = &mut self.rng;
        match r.below(28) {
            0 | 1 => {
                let c = color(r);
                self.tree.get_mut(id).unwrap().paint.background.current = c;
                format!("colour {id:?}")
            }
            2 => {
                let (dx, dy) = (r.unit() * 40.0 - 20.0, r.unit() * 40.0 - 20.0);
                let t = &mut self.tree.get_mut(id).unwrap().paint.node_transform;
                t.translate_x.current = f64::from(dx);
                t.translate_y.current = f64::from(dy);
                format!("translate {id:?}")
            }
            3 => {
                let o = [0.0, 0.4, 1.0][r.below(3)];
                self.tree.get_mut(id).unwrap().paint.opacity.current = o;
                format!("opacity {id:?} {o}")
            }
            4 => {
                let node = self.tree.get_mut(id).unwrap();
                node.visible = !node.visible;
                format!("visible {id:?}")
            }
            5 => {
                let z = [0, 1, -1, 2][r.below(4)];
                self.tree.get_mut(id).unwrap().z_index = z;
                format!("z {id:?} {z}")
            }
            6 => {
                let (w, h) = (10.0 + 90.0 * r.unit(), 10.0 + 70.0 * r.unit());
                let node = self.tree.get_mut(id).unwrap();
                node.layout_style.size.width = length(w);
                node.layout_style.size.height = length(h);
                format!("resize {id:?}")
            }
            7 => {
                // Reparent.
                let to = self.container();
                if !self.owned.contains(&id)
                    && !self.owned.contains(&to)
                    && to != id
                    && !self.tree.ancestors(to).any(|a| a == id)
                {
                    if let Some(from) = self.tree.get(id).and_then(|n| n.parent) {
                        self.tree.detach(from, id);
                    }
                    self.tree.add_child(to, id);
                    format!("reparent {id:?} -> {to:?}")
                } else {
                    "skipped reparent".into()
                }
            }
            8 if !self.owned.contains(&id) => {
                self.tree.remove(id);
                format!("remove {id:?}")
            }
            9 | 10 => self.add_node(),
            11 => {
                let w = r.unit() * 5.0;
                let rad = r.unit() * 12.0;
                let node = self.tree.get_mut(id).unwrap();
                node.paint.border_width.current = f64::from(w);
                node.paint.corner_radius.current = f64::from(rad);
                format!("border {id:?}")
            }
            12 => {
                let node = self.tree.get_mut(id).unwrap();
                node.paint.clip_children = !node.paint.clip_children;
                format!("clip {id:?}")
            }
            13 => {
                let words = ["a", "longer label here", "x y z", ""][r.below(4)];
                if let NodeKind::Text(state) = &mut self.tree.get_mut(id).unwrap().kind {
                    state.content = words.into();
                }
                format!("text {id:?}")
            }
            14 => {
                if let NodeKind::ScrollView(state) = &mut self.tree.get_mut(id).unwrap().kind {
                    state.scroll.current = f64::from(r.unit() * 60.0);
                }
                format!("scroll {id:?}")
            }
            15 => {
                let inset = r.below(2).eq(&0).then_some(f64::from(r.unit() * 10.0));
                self.tree.set_sticky(id, inset);
                format!("sticky {id:?}")
            }
            16 => {
                let scale = 0.5 + f64::from(r.unit());
                self.tree
                    .get_mut(id)
                    .unwrap()
                    .paint
                    .node_transform
                    .scale
                    .current = scale;
                format!("scale {id:?}")
            }
            17 if !self.owned.contains(&id) => {
                let parent = self.tree.get(id).and_then(|n| n.parent);
                if let Some(p) = parent {
                    let len = self.tree.get(p).map_or(0, |n| n.children.len());
                    let to = r.below(len.max(1));
                    self.tree.detach(p, id);
                    self.tree.insert_child(p, to.min(len.saturating_sub(1)), id);
                    format!("move {id:?} to index {to}")
                } else {
                    "skipped move".into()
                }
            }
            18 => {
                let on = r.below(8) == 0;
                self.tree.get_mut(id).unwrap().paint.backdrop_blur.current =
                    if on { 3.0 } else { 0.0 };
                format!("backdrop blur {id:?} {on}")
            }
            19 => {
                let _ = self.tree.set_focus_to(id);
                format!("focus {id:?}")
            }
            20 => {
                let node = self.tree.get_mut(id).unwrap();
                node.paint.blur.current = if node.paint.blur.current > 0.0 {
                    0.0
                } else {
                    2.0
                };
                format!("blur {id:?}")
            }
            21..=24 => {
                // Every list's offset, not just the one picked.
                let lists: Vec<NodeId> = self
                    .all
                    .iter()
                    .copied()
                    .filter(|l| {
                        self.tree
                            .get(*l)
                            .is_some_and(|n| matches!(n.kind, NodeKind::VirtualList(_)))
                    })
                    .collect();
                let off = f64::from(r.unit() * 120.0);
                for l in lists {
                    if let NodeKind::VirtualList(state) = &mut self.tree.get_mut(l).unwrap().kind {
                        state.scroll_offset.current = off;
                    }
                }
                format!("list offset {off}")
            }
            _ => {
                let _ = self.tree.get_mut(id);
                format!("touch {id:?} with no change")
            }
        }
    }

    fn frame(&mut self) -> Damage {
        self.tree.compute_layout(
            self.root,
            Size {
                width: AvailableSpace::Definite(f32::from(W)),
                height: AvailableSpace::Definite(f32::from(H)),
            },
        );
        self.tracker
            .damage(&self.tree, self.root, W, H, &mut self.text)
    }
}

fn run(seed: u64, frames: usize, edits: usize) {
    let mut world = World::new(seed);
    world.frame();
    let mut log: Vec<String> = Vec::new();
    for frame in 0..frames {
        log.clear();
        let count = if edits == 0 {
            0
        } else {
            world.rng.below(edits + 1)
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..count {
                let what = world.edit();
                log.push(what);
            }
            world.frame()
        }));
        if let Err(panic) = outcome {
            eprintln!("seed {seed}, frame {frame}, edits: {log:#?}");
            resume_unwind(panic);
        }
    }
}

#[test]
fn many_small_edits_never_miss_a_change() {
    for seed in 0..120 {
        run(seed, 60, 3);
    }
}

#[test]
fn bursts_of_edits_never_miss_a_change() {
    for seed in 1000..1060 {
        run(seed, 40, 12);
    }
}

#[test]
fn quiet_frames_stay_quiet() {
    for seed in 2000..2020 {
        let mut world = World::new(seed);
        world.frame();
        for _ in 0..5 {
            assert_eq!(world.frame(), Damage::None);
        }
    }
}

/// A big tree with one node changing walks a handful of nodes, not the tree.
#[test]
fn a_changed_leaf_walks_only_its_path() {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            flex_direction: taffy::FlexDirection::Column,
            flex_shrink: 0.0,
            size: Size {
                width: length(f32::from(W)),
                height: length(f32::from(H)),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
    );
    let mut leaves = Vec::new();
    for _ in 0..14 {
        let group = tree.insert(
            NodeKind::Container,
            Style {
                flex_shrink: 0.0,
                size: Size {
                    width: length(300.0),
                    height: length(20.0),
                },
                ..Default::default()
            },
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
        );
        tree.add_child(root, group);
        for _ in 0..20 {
            let leaf = tree.insert(
                NodeKind::Rect,
                Style {
                    size: Size {
                        width: length(10.0),
                        height: length(10.0),
                    },
                    ..Default::default()
                },
                PaintProperties::new(Color::from_rgba8(255, 0, 0, 255), 0.0, 1.0),
            );
            tree.add_child(group, leaf);
            leaves.push(leaf);
        }
    }
    let mut text = TextRenderer::new();
    let mut tracker = DamageTracker::with_verification();
    let layout = |tree: &mut Tree| {
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(W)),
                height: AvailableSpace::Definite(f32::from(H)),
            },
        );
    };
    layout(&mut tree);
    assert_eq!(tracker.damage(&tree, root, W, H, &mut text), Damage::Full);
    assert!(tracker.visited() > 250, "the first frame walks everything");
    tree.get_mut(leaves[137]).unwrap().paint.background.current = Color::from_rgba8(0, 255, 0, 255);
    layout(&mut tree);
    let damage = tracker.damage(&tree, root, W, H, &mut text);
    assert!(matches!(damage, Damage::Rects(_)), "{damage:?}");
    assert_eq!(
        tracker.visited(),
        1,
        "only the changed leaf is walked again"
    );
}

/// A scrolled list's rows are placed by an offset no row can see: after one
/// scroll, a change to a row must still erase where the row really was.
#[test]
fn a_row_changing_after_its_list_scrolled_erases_its_real_old_place() {
    for seed in 0..40 {
        let mut world = World::new(seed);
        world.frame();
        let list = world.owned[0];
        let row = world.owned[2];
        for step in 0..6 {
            if let NodeKind::VirtualList(state) = &mut world.tree.get_mut(list).unwrap().kind {
                state.scroll_offset.current = 17.0 * f64::from(step % 3 + 1);
            }
            world.frame();
            world
                .tree
                .get_mut(row)
                .unwrap()
                .paint
                .node_transform
                .translate_x
                .current = f64::from(step) * 7.0;
            world.frame();
        }
    }
}

/// New fonts change every text's measure, so what was recorded is not trusted:
/// the next call walks everything.
#[test]
fn a_change_of_fonts_walks_the_whole_tree() {
    let mut world = World::new(7);
    world.frame();
    world.frame();
    assert!(world.tracker.visited() <= 1, "a quiet frame walks nothing");
    let total = {
        world.tree.get_mut(world.root).unwrap().visible = true; // touches the root only
        world.frame();
        world.tracker.visited()
    };
    engine_render::register_font(
        include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf").to_vec(),
    )
    .unwrap();
    world.text.sync_registered_fonts();
    world.frame();
    assert!(
        world.tracker.visited() > total,
        "{} walked after a font change, {total} for a root-only change",
        world.tracker.visited()
    );
}
