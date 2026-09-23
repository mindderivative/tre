# LOG — M64: Terminal Text-Shaping Cache

- Found by a `/review-project` multi-lens code review (Performance
  lens, adversarially verified before scoping) -- the one High-severity
  finding of the four scoped from that audit. `TextRenderer::draw_
  terminal` (`crates/engine-render/src/text.rs`) groups each terminal
  row into contiguous same-style runs and, before this milestone,
  called `build_field_layout` unconditionally for every run on every
  row, on every single paint -- a fresh `parley` shape from scratch
  every time, consulting neither `layout_cache` nor `monospace_cell_
  cache`, unlike every other text path in the same file. Trigger:
  `engine-py::terminal.rs`'s `drain_into` marks the tree dirty on every
  batch of new PTY bytes, so any fast/chatty terminal output (a verbose
  build, `cat` on a large file, `yes`) re-shaped every distinct-style
  run across every visible row, every single frame, even when most of
  the screen's own styling was unchanged from the previous one.
- Real design constraint investigated before writing any code: terminal
  rows/runs have no stable `NodeId` of their own (only the whole
  terminal widget does), so `layout_cache`'s own bare-`NodeId` key
  couldn't be reused directly -- needed a genuinely new compound key.

## What shipped (single milestone, both phases)

1. New `TerminalRunKey`/`CachedTerminalRun` types + a `terminal_run_
   cache: HashMap<(NodeId, u16, u16), CachedTerminalRun>` field on
   `TextRenderer`, keyed by `(terminal NodeId, row, the run's own
   starting column)` -- a stable real slot across frames (the same row
   tends to start the same run at the same column when nothing
   changed), not a synthetic per-row run index that would shift every
   later run on a row whenever an earlier one changed shape. The key
   holds only the real shaping inputs `build_field_layout` itself takes
   (`content`/`font_family`/`font_weight`/`font_size`) -- not `align`/
   `spans`/`default_color`, since a run's own foreground color/italic-
   shear/underline are applied at paint time, never baked into shaping.
2. New `shaped_terminal_run` method mirrors `shaped_layout`'s own real
   "equality on the key is the whole invalidation check" contract --
   but, deliberately, compares the cached key's own fields against the
   *borrowed* new inputs before ever allocating an owned key, rather
   than `shaped_layout`'s own allocate-then-compare pattern. This is
   new code with no reason to repeat a pattern already flagged as a
   real, separate finding (M66) elsewhere in the same file.
   `draw_terminal`'s own per-run shaping call site routed through it
   instead of calling `build_field_layout` unconditionally --
   `draw_terminal`'s own `_node_id` parameter un-underscored to
   `node_id`, now genuinely used for the first time.
3. `evict_stale_layouts` widened to also evict the new cache: both "the
   terminal node was removed entirely" (the identical real contract
   `layout_cache`'s own eviction already has) and a real second case
   `layout_cache` has no equivalent of at all -- any `(row, col)` that
   fell outside the terminal's own current `rows`/`cols` bounds after a
   resize shrunk it, checked directly against the live `NodeKind::
   Terminal`'s own real state (a plain `Text`/`TextField` node has no
   internal sub-grid of its own that can shrink out from under a cached
   sub-key the way a terminal's rows/cols can).
- Tests: 3 new Rust unit tests -- an unchanged repaint doesn't grow the
  cache; changing one row's own content reshapes only that row's entry
  while sibling unchanged rows' cached content stays untouched; a real
  resize-shrink evicts the now-out-of-bounds rows while the terminal's
  own `NodeId` stays alive, and a subsequent node removal evicts what's
  left. New `terminal_node`/`set_cell` test helpers, mirroring `text_
  node`'s own already-established construction pattern for this test
  module.
- `BUILD_TRACKER.md`: full Milestone 64 section, Top Metrics row at
  100%, Just-closed/Up-next refreshed to point at M65-M67 as the
  remaining real backlog from the same review. Tracker regenerated (18
  milestones/55 phases/137 items/3 known gaps/25 fixed gaps), artifact
  republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-render` 32, up
  from 29, +3; every other crate's own count unchanged); `maturin
  develop --release`; `pytest tests/` 831 passed, unchanged from the
  pre-milestone baseline -- this is pure internal Rust-side caching
  with no new Python-facing surface at all, matching `layout_cache`/
  `monospace_cell_cache`'s own identical "Rust-tested only" precedent,
  not a gap in this milestone's own coverage; every file in `examples/`
  ran clean including `terminal.py`; `demo/showcase.py` (all 5 phases,
  exit 0).

## Status

**M64 is complete, both phases.** The highest-severity finding from
the `/review-project` audit is closed with zero regression to any
existing test, example, or the showcase demo. Committing locally now;
push deferred pending explicit user confirmation. Next: M65
(incremental existence tracking for `Tree::compute_layout`'s four
redundant full-tree scans).
