# Plan: M3 Phase 7, Step 13 — Overlay Mechanism (§14 step 13, §11.3)

Corresponds to `BUILD_TRACKER.md` M3 Phase 7, step 13 of 3 (steps 13-15).

## Goal

Per §14 step 13: "Overlay mechanism (§11.3) — one dropdown menu,
proving the tree-resident/`Position::Absolute`/append-to-`children`
approach before menu bars, dialogs, or docking's drop-zone indicators
depend on it."

§11.3's core claim: an overlay is **tree-resident, not a parallel
structure** — an ordinary child of the tree's root, using `taffy`'s
existing `Position::Absolute` (positioned relative to its anchor's
computed bounds), *appended* to the root's `children` (so paint order
= children-list order = on-top-with-zero-new-z-order-concept). Because
it's a real tree node, paint, hit-testing, the focus model, and
`build_access_update()` should all already walk it with **zero special-
casing** — that claim is exactly what this step's own test needs to
prove empirically (pixel-readback), not assume from reading the
architecture text.

## Scope narrowing (checked directly before writing anything)

§11.3's full picture includes real dismissal behavior ("dismissed on
outside-click or Escape") and real MD3 components (`NodeKind::MenuBar`/
`MenuItem`/`Menu`, keyboard mnemonics). Checked directly: real pointer/
keyboard `InputEvent`/`AppHandler` dispatch and hit-testing (§11.10)
still don't exist anywhere in this codebase — the same finding steps
7/9/11/12 already made. There is nothing to wire dismiss-on-click/
Escape *to* yet, and no NodeKind payload exists for MenuBar/MenuItem
(inventing one now, with no dispatch to drive it, would be exactly the
premature-abstraction trap this project keeps avoiding).

This step builds and proves the real, load-bearing mechanism only:

In scope:
- `engine_core::OverlayMeta` matching §11.3's own struct exactly
  (`anchor`, `dismiss_on_outside_click`, `dismiss_on_escape` — stored
  as real data, not yet acted on by any dispatch).
- `Tree` gains `overlays: HashMap<NodeId, OverlayMeta>`,
  `Tree::absolute_position(id) -> (f64, f64)` (a new, reusable, real
  capability: a node's position accumulated all the way to the root,
  not previously exposed as a standalone query — `build_tree_scene`/
  `build_access_update` only ever computed this *inline* during a
  full-tree walk), `Tree::set_layout_style` (keeps the `Node`'s own
  `Style` copy and `taffy`'s internal copy in sync — `TaffyTree::
  set_style` exists for exactly this, verified directly), `Tree::
  open_overlay(root, anchor, content, meta)` (positions `content`
  absolutely relative to `anchor`'s current bounds, appends it to
  `root`'s children, records the metadata), `Tree::close_overlay(id)`
  (reuses step 12's `Tree::remove` for real subtree removal, drops the
  metadata entry).
- The actual proof: a real "one dropdown menu" scenario (a Rect
  standing in for the menu surface — no `MenuItem` component exists to
  put inside it yet, and none is needed to prove positioning/z-order),
  headless pixel-readback confirming: the overlay renders at its
  anchor-relative position (not some default/origin position), it
  paints *on top* of whatever it overlaps (proving append-order-is-
  paint-order, not a coincidence of non-overlapping geometry), and
  `close_overlay` actually removes it from a re-render.

Out of scope, stated explicitly: dismiss-on-outside-click/Escape firing
(needs pointer/keyboard dispatch that doesn't exist), `NodeKind::
MenuBar`/`MenuItem`/`Menu` component types, keyboard mnemonics.

## Verification

`cargo test -p engine-core` (new unit tests for `absolute_position`/
`open_overlay`/`close_overlay`) and a new headless pixel-readback test
in `engine-render/tests/` proving real on-screen position and paint
order. `cargo test --workspace`, `cargo clippy --workspace --all-
targets -- -D warnings`, `cargo fmt --check` all clean.
