# Log: M6 Phase 3 — `Position::Absolute` exposure from Python

Corresponds to `BUILD_TRACKER.md` M6 Phase 3. Confirmed directly
(`engine-py/src/window.rs`): every Python-facing node-creation method
built a plain `Style { size, ..Default::default() }` — implicit
flex-row/block flow only, never `Position::Absolute`. The exact
concrete blocker M5 Phase 4 hit, and the reason M6 Phase 2's own
`pan_zoom.py` had to animate one node's transform directly instead of
a "camera" wrapping positioned children.

## Investigation before writing code

- **Scoped to `add_rect`/`add_canvas` specifically, not every node-
  creation method.** `add_splitter`/`add_virtual_list` are both
  semantically tied to their real position in the flex-row flow (a
  splitter sits *between* its two flanking siblings; a virtual list's
  own block-stacking is how "a list" reads at all) — absolute
  positioning wouldn't compose meaningfully with either, and no
  consumer needs it there. `add_rect`/`add_canvas` are exactly the two
  methods M5 Phase 4's own node-graph story needed freely-positioned
  instances of.
- **The containing block for `Position::Absolute` insets is the
  window's own root, which already has real padding** (`PADDING =
  16.0`, `PyWindow::new`'s own constructor) — confirmed by reading it
  directly. An absolutely-positioned child's `(x, y)` lands relative to
  the root's own padding-box origin, not the raw window corner. Named
  explicitly in the new kwargs' doc comment rather than silently
  surprising a caller.

## A real testing constraint found while planning verification, not discovered by trial and error

No Python-level pixel readback exists anywhere (M6 Phase 2's own
corrected finding), and there is no raw-coordinate hit-test entry point
either — `Window.click(node)` always resolves to `node`'s *own* current
center point before dispatching, never an arbitrary `(x, y)`. The real,
available proof: position a second node to deliberately *overlap* a
first node's own default (unpositioned) flex-row position, then click
the first node — `Window.click`'s own real implementation resolves to
the clicked node's center and then runs genuine topmost-wins
hit-testing there, so if the explicit position genuinely took effect,
the *second* node's handler fires instead. If positioning had silently
fallen back to the old flex-row-only behavior, the second node would
sit elsewhere in-flow and the first node's own handler would still
fire. This tests the real, observable behavior through the same
dispatch mechanism every other FFI test in this project already uses,
with no new introspection API needed.

## What happened

`engine-py/src/window.rs`: new private `fn positioned_style(size, x:
Option<f32>, y: Option<f32>) -> Style` — the real `Position::Absolute`
+ `taffy::Rect` inset shape every Rust-level pixel test already uses
internally, factored out once here. `add_rect`/`add_canvas` gain
`x`/`y` optional kwargs (both default `None`, fully backward
compatible — omitting both is byte-for-byte the prior behavior),
routed through it.

Three new pytest tests: the overlap-proves-positioning trick for both
`add_rect` and `add_canvas`, plus an explicit backward-compatibility
regression test for omitting `x`/`y` entirely — all passed on the first
run. New `examples/positioned_graph.py`: the real, idiomatic node-graph
shape M5 Phase 4 originally wanted — independently-positioned, real,
clickable `Rect` nodes plus one positioned `Canvas` for edges, closing
that phase's own stated gap for real (`examples/node_graph.py`'s own
single-`Canvas` workaround is left as-is, still a legitimate, different
pattern for batched drawing, not replaced).

Full `cargo test --workspace --release` clean (unchanged Rust test
count — this phase touched no `engine-core`/`engine-render` code),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check` all clean. `maturin develop --release` + full `pytest tests/`
(77 passed, up from 74, 1 skipped) and all ten examples (nine existing
+ new `positioned_graph.py`) confirmed clean.
