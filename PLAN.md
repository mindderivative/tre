# Plan: M4 Phase 3 — Real Pointer-Drag Dispatch (§11.5, §11.4)

Phases 1-2 (`c761d1c`/`1d6086b`/`b3b179a`) built real pointer/keyboard
dispatch and real assistive-technology action dispatch end to end. One
real, explicitly-named gap remains from every earlier splitter/docking
step: `Tree::set_splitter_position` has existed since M3 step 15 Stage A
as a direct, programmatic API, with its own real doc comment and every
later mention of it stating the same thing — "real splitter-drag input
... needs §11.10 pointer dispatch" (step 15's own `LOG.md`), later
"still have no `winit`-driven UX wired... nothing calls `set_splitter_
position`... from a real drag gesture yet" (M4 Phase 1's own `BUILD_
TRACKER.md` entry). §11.5's own text describes exactly this: "On drag,
`position`'s tick handler mutates its two adjacent siblings' `layout_
style`... **used both standalone... and by docking (§11.4) for zone
resizing — one mechanism, two call sites.**" This phase makes that real
for the first time: a user can drag a splitter with the mouse, and
because docking's own zone-resize already reuses `set_splitter_position`
verbatim (M3 step 15 Stage B), dragging a dock-zone boundary becomes
real for free, with zero docking-specific code.

## Scope

**In scope:** real mouse-driven splitter *resizing* — press on a
`NodeKind::Splitter`, drag, release. This is what §11.5's own text
describes and what every earlier step's "known gap" note actually
named.

**Explicitly out of scope, stated here:** "drag-to-rearrange docking"
in the sense of picking up a whole dock *panel* and moving it to a
different zone (a genuinely different feature — reordering/redocking,
not resizing) is not touched. Nothing in this codebase models that yet,
and it's a substantially larger feature (drop-target detection, panel
reparenting mid-drag) than this phase's own real, narrow claim.

## Design

**`Tree` gains a `dragging: Option<NodeId>` field** (the splitter
currently being dragged, if any) alongside the existing `pressed`/
`hovered` fields, same lifecycle shape.

**A new private `splitter_geometry(&self, id) -> (NodeId, NodeId, bool,
f64)` helper** factors the "find a splitter's own flanking siblings,
its parent's flex axis, and their current combined extent" logic
already inlined in `set_splitter_position` out into something a new
method can also call — not duplicated a second time.

**`Tree::update_drag(&mut self, point, now)`** (new): if `self.dragging`
names a real splitter, converts `point`'s coordinate along the parent's
own flex axis into a 0.0..=1.0 fraction relative to the left sibling's
own absolute start and the flanking siblings' combined extent (which
stays constant during a drag — the two siblings only trade extent
between each other, never grow/shrink together), then calls the
*existing* `set_splitter_position` with that fraction — the real "one
mechanism, reused, not reimplemented" claim §11.5's own text makes.

**`Tree::dispatch` changes**: `PointerPressed` on a `NodeKind::Splitter`
with the primary button sets `self.dragging = Some(node)` (alongside
its existing `self.pressed` press-tracking and ripple-spawn, reusing
the same hit-test call, not a second one). `PointerMoved` calls
`update_drag` whenever a drag is active, in addition to its existing
`update_hover` call. `PointerReleased` with the primary button clears
`self.dragging` (a real mouse-up always ends a drag, wherever it
happens — not conditioned on hitting the splitter again, matching real
OS drag semantics: the pointer can leave the splitter's own thin hit
region mid-drag and the drag must still track it).

## Verification

New `engine-core` unit tests, each isolating one claim: pressing a
splitter and moving the pointer resizes the two flanking siblings
continuously (not just once, like `set_splitter_position`'s own
existing test proves — this proves the *live-follows-the-cursor*
claim, sampling mid-drag, not just before/after); releasing ends the
drag (a further pointer move afterward must not keep resizing);
pressing on a non-splitter node never starts a drag (dragging a button
must not accidentally move some unrelated splitter); a drag started
with a non-primary button never happens. `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check` all clean.

A real pixel-readback test in `engine-render`, matching `splitter_drag.
rs`'s own existing shape but driven through `Tree::dispatch`'s real
`PointerPressed`/`PointerMoved`/`PointerReleased` sequence instead of a
direct `set_splitter_position` call -- the concrete, on-screen proof
that real dispatch (not just the underlying mechanism) moves a real
rendered pane boundary.

## Not this phase (stated, not silent)

Two-phase press/hold/release ripple timing (`interaction.rs`'s own
long-stated deferred scope) and `HoverEnter`/`HoverExit` firing through
`engine-spec`'s handler path (§7.3's own text) are both real, separate
pieces of work M4's overall scope still leaves open -- neither is
touched here; this phase is scoped to the single most concretely-named,
repeatedly-flagged gap.
