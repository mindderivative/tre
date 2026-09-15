# Plan: M3 Phase 2, Step 3 — Taffy Layout + Frame-Time CI Benchmark (§14 step 3)

Corresponds to `BUILD_TRACKER.md` M3 Phase 2, step 3 of 4.

## Goal

Per §14 step 3: "Wire `taffy` for layout of multiple static nodes. Add
the frame-time CI benchmark here (§6 Locked Decisions) — this is the
earliest point a real render+layout+tick pipeline exists to measure
against the stated 16.6ms/8.3ms target." Implement §5's `NodeId`/`Node`/
`NodeKind`/`PaintProperties`/`Tree` in `engine-core`, wire `taffy` for
real layout computation, prove `engine-render` paints nodes at their
taffy-computed positions (not just that colors are right), and add the
CI-enforced frame-time benchmark §6 has been promising since M2.

## Scope

In scope:
- `engine-core::node`: `NodeId` (via `slotmap::new_key_type!` — costs no
  new dependency, `taffy` already pulls in `slotmap` transitively at the
  same version, and its key type gives exactly the generational-index
  semantics §5 specifies), `NodeKind` (`Rect`/`Container` only —
  `Text`/`Image`/`Slider`/`Checkbox`/`Canvas` land at their own steps),
  `PaintProperties` (`background`/`corner_radius`/`elevation`/`opacity`
  only — `transform`/`shape` need `Interpolate` impls no step before
  their own real use needs), `Node` (omitting `access`/`interaction`,
  same reasoning).
- `engine-core::tree::Tree`: owns a `slotmap::SlotMap<NodeId, Node>` plus
  a `taffy::TaffyTree<()>`, linked via a `SecondaryMap<NodeId,
  taffy::NodeId>` — two parallel structures kept in sync only at
  `insert`/`add_child`, not taffy's custom-tree traits (simpler, provably
  correct by construction, revisit only if profiling shows the
  bookkeeping itself is the cost). `insert`/`add_child`/`get`/`get_mut`/
  `compute_layout`/`layout`/`tick_all`.
- `engine-render::build_tree_scene`: walks a `Tree` from a root, paints
  every `NodeKind::Rect` at its absolute (ancestor-offset-accumulated)
  taffy position.
- Unit tests in `engine-core` (insert, add_child linking both
  structures, a deterministic 3-child row layout with hand-computable
  positions, `tick_all` advancing every node).
- A headless integration test in `engine-render`
  (`tests/layout_tree.rs`) proving layout position actually drives paint
  position — two same-size, differently-colored children side by side,
  sampling a pixel inside each one's own laid-out box.
- The frame-time CI benchmark (`tests/frame_budget.rs`): 300 animated
  nodes, timed tick+layout+build_tree_scene+encode+submit, asserting the
  median stays under 16.6ms. `#[ignore]`d by default (meaningless in an
  unoptimized debug build — measured ~36ms there vs. ~0.6ms in release
  for the identical pipeline) and run explicitly with `--release`.
- Extending the real windowed demo (`tests/rect_window.rs`) to a real
  `Tree` of 4 rects laid out in a row, each independently animating its
  own color inside its own `PaintProperties`.

Out of scope: the "walks only the active animation set" scoped version
of the central tick (§5's own wording) — `Tree::tick_all` is a naive
whole-tree walk; real dirty-tracking is §6's own design surface, not
manufactured ahead of a step that profiles it as necessary. `parley`/
text (step 4). `access`/`interaction` node fields (steps 7/9).

## Verification

`cargo test --workspace` (all green, `frame_budget` reports `ignored`);
`cargo test -p engine-render --test frame_budget --release -- --ignored
--nocapture` (the actual budget assertion, release mode); `cargo clippy
--workspace --all-targets` and `cargo fmt --check` clean; the real
windowed demo presenting four laid-out, independently-animating rects.
