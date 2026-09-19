# PLAN — M30 Phase 9 Step 2: Node Graph

## Goal
Add `Window.add_node_graph`/`Window.add_graph_node` — closing the real
gap M5 Phase 4's own `PLAN.md` named and `examples/positioned_graph.py`
(M6 Phase 3) only ever demonstrated as raw composition (interchangeable
circles, no real node anatomy, no reparent/position automation).

## Steps
1. Read pyCopper's own real `NodeGraph` widget
   (`src/pycopper/widgets/nodegraph.py`) directly for its real design:
   title-bar-styled draggable nodes, named ports, declared edges,
   panning via scroll-offset (zoom explicitly out of its own v1 scope).
2. Read TRE's own real M5 Phase 4/M6 Phase 3 history directly in
   `BUILD_TRACKER.md` (the durable record) to understand exactly what
   groundwork already exists (`x`/`y` positioning, `transform`
   animation) and what gap remains (a real, styled, reusable node
   component; reparent-into-a-pannable-viewport automation).
3. Investigate whether `Node.add_child` (M6 Phase 1) genuinely
   re-resolves a node's own `Position::Absolute` inset relative to its
   new parent after reparenting -- confirmed empirically via the same
   real overlap-and-click hit-test technique `test_position.py`
   established, not assumed.
4. Investigate whether an ancestor's own `transform` already composes
   correctly into a descendant's `absolute_position` -- confirmed via
   direct read of an already-existing, already-passing engine-core
   test (`absolute_position_follows_an_ancestor_transform`, M6 Phase
   4), not a new empirical check.
5. Confirmed via direct source read: no general clipping mechanism
   exists for any container besides `VirtualList` -- a real, stated
   scope boundary for this step (nodes/edges panned outside the
   viewport overflow visually, not clipped).
6. Confirmed via direct source read: no Python-facing
   `PointerMoved`-while-pressed hook exists -- a real, stated scope
   boundary (no drag-to-move mouse gesture; `node.animate("transform",
   ...)` is the real, available repositioning mechanism).
7. Implement `Window.add_node_graph(width, height, x, y)` in
   `window_factory.rs` -- a themed `surface_container_low` viewport,
   mechanically identical to `add_rect`.
8. Implement `Window.add_graph_node(graph, label, x, y, width,
   height)` -- a real composed node (title bar + body), attached
   directly under `graph` (not `self.root`), reusing `add_status_bar`'s
   own theme-resolution pattern and `open_menu`'s own `Rc::ptr_eq`
   cross-window safety check.
9. **Found a real, confirmed bug live while verifying end to end**:
   the title bar's own `Rect` intercepted clicks meant for the node.
   Fixed with `tree.set_hit_testable(title_bar, false)`, the same
   established fix `Navigation Rail`/`Tabs` already used.
10. Add `.pyi` stubs for both.
11. Write `tests/test_node_graph.py` and a new example.
12. **Found and corrected a real self-inflicted mistake**: a first
    draft of the example silently overwrote the pre-existing
    `examples/node_graph.py` (M5 Phase 4's own Canvas/`CustomHitTest`
    example) -- caught via `git status` before committing, restored
    byte-for-byte, and the new demonstration renamed to
    `examples/graph_editor.py`.
13. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
14. Update `BUILD_TRACKER.md` (Top Metrics row, Step 2 line,
    "Just closed"/"Up next" trailer), regenerate + republish the
    Build Tracker artifact.
15. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (465 pytest
passed/1 skipped up from 457, all 64 examples, showcase demo, 44 Rust
test binaries — unchanged, this step needed no new Rust unit test
since it's a pure engine-py composition, verified through direct
source reads and empirical hit-testing instead).
