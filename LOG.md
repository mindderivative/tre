# Log: M3 Phase 7, Step 13 — Overlay Mechanism (§14 step 13, §11.3)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 7, step 13 of 3 (steps 13-15).

## What happened

**Checked what "dismissed on outside-click or Escape" actually needs
before writing anything.** Real pointer/keyboard `InputEvent`/
`AppHandler` dispatch and hit-testing (§11.10) still don't exist
anywhere in this codebase — the same finding steps 7, 9, 11, and 12
already made. There's nothing to wire dismissal *to* yet, and no reason
to invent `NodeKind::MenuBar`/`MenuItem` component types with no
dispatch to drive them. This step builds and proves the real,
load-bearing mechanism §11.3 actually depends on everything else: tree
residency, `Position::Absolute` placement, and append-order-is-paint-
order — deferring dismissal behavior and real menu components to
whenever pointer/keyboard dispatch lands.

**`engine_core::OverlayMeta`** matches §11.3's own struct sketch
verbatim (`anchor`, `dismiss_on_outside_click`, `dismiss_on_escape`) —
real data, stored, not yet acted on.

**`Tree::absolute_position(id)`** is a genuinely new capability, not
previously exposed: `build_tree_scene`/`build_access_update` only ever
computed a node's root-relative position *inline*, during their own
full-tree walks, each keeping its own separate accumulator. Nothing
before this let a caller ask "where is this one node, absolutely"
without doing a full walk. Implemented as a bottom-up walk via each
node's own `parent` chain (`O(depth)`, not `O(tree size)`) — proven
correct through two levels of real nesting (padding + a sibling's
width contributing to the target's own offset), not just a trivial
single-level case.

**`Tree::set_layout_style`**: `TaffyTree::set_style` exists for exactly
"push a style update back in after creation" (verified directly in its
source before using it) — without it, mutating `Node::layout_style`
directly after `insert` would silently desync `engine-core`'s own copy
from `taffy`'s internal one, reintroducing the exact divergence
`insert`'s own doc comment says can't happen.

**`Tree::open_overlay`/`close_overlay`**: `open_overlay` computes the
anchor's absolute bounds, sets the content node's style to
`Position::Absolute` with `inset` derived from those bounds (placed
directly below-left of the anchor — a real dropdown's usual placement),
appends it to the given root's children, and records the metadata.
`close_overlay` reuses step 12's real recursive `Tree::remove` rather
than a second removal path, and drops the metadata entry.

**The actual proof, split at two levels.** `engine-core`'s own unit
tests prove the mechanics (`absolute_position` accumulates correctly,
`open_overlay` computes the right inset and appends in the right
place, metadata round-trips, `close_overlay` actually removes the node
and its metadata). The real, falsifiable claim — "paint order is
children-list order, so an appended overlay paints on top of whatever
it overlaps, with zero special-casing anywhere in `build_tree_scene`" —
needed a render-level pixel-readback test to actually prove, not just
assert from reading the architecture text:
`crates/engine-render/tests/overlay_menu.rs` builds a full-canvas green
background panel (inserted first), a blue "trigger" anchor, then opens
an orange dropdown menu against that anchor. One sample point sits
inside the menu's own anchor-relative bounds *and* is also covered by
the green background underneath — only correct positioning *and*
real append-order-controls-paint-order together explain seeing orange
there, and the test passed on its first real run, meaning
`build_tree_scene`'s existing children-list walk already handles an
absolutely-positioned, appended node with genuinely zero changes
needed to that function. A final step closes the overlay and confirms
the same point reverts to the background panel's own color, proving
real removal from what gets painted, not just from bookkeeping.

## Verification

```
$ cargo test -p engine-core           # 20 passed, incl. 3 new overlay tests
$ cargo test -p engine-render --test overlay_menu -- --nocapture
test dropdown_menu_positions_by_anchor_and_paints_above_the_background_via_append_order ... ok

$ cargo test --workspace              # all green
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                   # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 7 step 13 done (1 of 3 steps in this
phase). Next: step 14 — multi-window (§11.1), a second `PyWindow`
opened from a running app, proving `WindowId`-routed event dispatch
before docking's "detach into its own window" pattern needs it.
