# Plan: M3 Phase 7, Step 15 — Docking + Virtualization (§14 step 15)

Corresponds to `BUILD_TRACKER.md` M3 Phase 7, step 15 of 3 — **M3's
final step.** §14's own text: "the two largest net-new v1 subsystems
from §11, sequenced last since both build on the overlay mechanism
(step 13), splitters, and the accepted multi-window model (step 14)."
Splitters (§11.5) aren't a separately-numbered build-order step but are
a hard prerequisite docking's own text names directly ("docking is a
*consumer* of splitters, not a second resize mechanism") — nothing in
this codebase has `NodeKind::Splitter` yet, so it's built here, first.

Three stages, each separately committed and verified, matching step
12's own precedent for a step this large.

## Stage A — Splitters (§11.5)

`NodeKind::Splitter(SplitterState { position: Animated<f64> })`.
`Tree::set_splitter_position(splitter, position, now)`: finds the
splitter's two flanking siblings in its parent's own `children` order
(the splitter must sit directly between them), resizes both from
`position` (0.0..=1.0 along the split axis) against their combined
current extent, applying the result via step 13's own
`Tree::set_layout_style` (which already keeps `taffy`'s internal style
copy in sync — no new machinery needed there). A drag is a 1:1,
instant mouse-follow, not a smoothly-eased transition, so this sets
`position` via `animate_to` with `Duration::ZERO` and ticks it
immediately (the same "an instant application needs an explicit tick to
actually materialize" fix step 12 already found and applied to
bindings).

Proof: a headless pixel-readback test — two colored panes with a
splitter between them; calling `set_splitter_position` moves the real
pane boundary, verified on screen, not just in `layout_style` data.

## Stage B — Docking (§11.4)

`engine_core::{DockLayout, DockZone}` matching §11.4's own struct
sketch exactly (`zones: [Option<DockZone>; 5]`, `panels: SmallVec<
[NodeId; 4]>`, `active_tab: usize`, `size: Animated<f64>`) — POD, so an
app can serialize/restore it, per the architecture's own stated reason.

**Tabbed grouping via detach, not `Display::None`:** `Tree::
apply_active_tab(container, zone)` ensures exactly `zone.panels[zone.
active_tab]` is attached as a child of `container`, detaching (not
deleting — `taffy::TaffyTree::remove_child`, verified directly in its
own doc comment: "not removed from the tree entirely, simply no longer
attached") any other currently-attached panel. A detached panel isn't
in anyone's `children` list, so `build_tree_scene`'s existing recursive
walk already excludes it from paint automatically — the same "reuse
what's already there, prove it doesn't need touching" pattern step 13's
overlay proof established for append-order.

**Resizing between zones reuses Stage A's splitter mechanism exactly**
(§11.4's own text) — a docked layout's zone containers and the
splitters between them are ordinary flex siblings; `set_splitter_
position` needs no docking-specific code at all to resize a zone
boundary. `DockZone.size` is the app's own persisted-size mirror,
synced from the real post-resize layout by a small helper, not written
by the splitter mechanism itself — keeping "one generic resize
mechanism, N unrelated call sites" real, not just asserted.

Proof: a real (3-zone: Left/Center/Right, the same mechanism a 5-zone
layout would use) docked screen — each zone's active tab shows its own
color, switching a tab changes what's visible without disturbing other
zones, and dragging the Left/Center splitter moves the real zone
boundary on screen.

## Stage C — Virtualization (§11.7)

`NodeKind::VirtualList(VirtualListState { item_count, item_extent,
materialized: BTreeMap<usize, NodeId> })`. Only the visible window (+
small overscan) are ever real `Node`s; scrolling recycles slots via
§5's own generational `NodeId` reuse (already free — `Tree::remove`
already invalidates a slot's generation on drop, nothing new needed
there either).

**The Risk Register's own named acceptance gate for this step:** "Spike
this against a real large dataset... before any data-grid-shaped
component depends on it; if per-call GIL overhead dominates, batch the
callback." `engine-py` gains the "materialize item N" FFI callback
(§11.7's own "genuinely different callback pattern" from `on_click`/
`on_complete`) and a real benchmark against 100,000+ logical rows,
measuring actual per-call `pyo3` overhead crossing the GIL boundary --
not assumed fast enough, measured.

## Verification

Each stage: its own new tests pass, `cargo test --workspace`, `cargo
clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
all clean; Stage C additionally needs a real `maturin develop` +
benchmark run. Each stage gets its own local commit as it completes.
This closes M3 entirely once Stage C lands.
