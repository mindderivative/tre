# Plan: M6 Phase 3 — `Position::Absolute` exposure from Python

## Context

Confirmed directly (`engine-py/src/window.rs`): every Python-facing
node-creation method (`add_rect`, `add_canvas`, `add_splitter`,
`add_virtual_list`) builds a plain `Style { size, ..Default::default()
}` — always implicit flex-row/block flow, never `Position::Absolute`.
The exact concrete blocker M5 Phase 4 hit while trying to write a
positioned node-graph example, and the reason M6 Phase 2's own
`pan_zoom.py` had to animate one node's transform directly instead of
a "camera" wrapping positioned children.

## Investigation before writing code

- **Scoped to `add_rect`/`add_canvas` specifically, not every node-
  creation method** — `add_splitter`/`add_virtual_list` are both
  semantically tied to their real position in the flex-row flow
  (a splitter sits *between* its two flanking siblings;
  `set_virtual_list_window`'s own block-stacking is how a "list" reads
  top-to-bottom at all) — absolute positioning wouldn't compose
  meaningfully with either, and no consumer needs it there. `add_rect`/
  `add_canvas` are exactly the two node-creation methods M5 Phase 4's
  own node-graph story actually needed freely-positioned instances of
  (circular nodes, custom-drawn edges).
- **The containing block for `Position::Absolute` insets is the
  window's own root, which already has real padding (`PADDING = 16.0`,
  `PyWindow::new`'s own constructor).** Confirmed by reading it
  directly, not assumed — an absolutely-positioned child's `(x, y)`
  lands relative to the root's own padding-box origin, not the raw
  window corner. Named explicitly in the new kwargs' own doc comment
  rather than silently surprising a caller who expects `(0, 0)` to mean
  the window's own top-left pixel.
- **Both `x`/`y` are independently optional, but trigger the same
  positioning mode together** — if either is given, the node uses
  `Position::Absolute` with both insets (the other defaulting to
  `0.0` if only one was given); if neither is given, behavior is
  byte-for-byte unchanged (the existing implicit flex-row flow) —
  fully backward compatible, no existing caller's output changes.

## Approach

1. **`engine-py/src/window.rs`**: new private `fn positioned_style(size:
   Size<Dimension>, x: Option<f32>, y: Option<f32>) -> Style` — the
   real `Position::Absolute` + `taffy::Rect` inset shape every Rust-
   level pixel test already uses internally (`absolute()` helpers in
   `overlay_menu.rs`/`transform_composition.rs`/etc.), factored out
   once here since two real Python call sites now need it. `add_rect`/
   `add_canvas` gain `x: Option<f32> = None, y: Option<f32> = None`
   kwargs, both routed through it.
2. **New pytest tests**: an absolutely-positioned rect/canvas actually
   lands at its own explicit position (verified the same way
   `absolute_position`-based tests elsewhere do — no pixel readback
   needed here either, matching M6 Phase 2's own corrected scope: this
   is a layout-shape claim, testable via the tree's own real bounds);
   omitting `x`/`y` entirely is unchanged (existing flex-row tests
   still pass unmodified).
3. **New example**: the real, positioned node-graph M5 Phase 4 couldn't
   build — independently-positioned `Rect` nodes and `Canvas` edges in
   one shared coordinate space, closing that phase's own stated gap for
   real this time.

## Files to touch

- `crates/engine-py/src/window.rs` — `positioned_style`, `add_rect`/
  `add_canvas` kwargs.
- New pytest tests + example.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
