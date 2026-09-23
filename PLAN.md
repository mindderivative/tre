# PLAN — M64: Terminal Text-Shaping Cache

*(Replaces the prior M63 plan in this file — M63 is complete, committed.
First of four milestones scoped directly from a `/review-project`
multi-lens audit; see this file's own Milestone 64 section in
`BUILD_TRACKER.md` for the full real investigation.)*

## Goal
`TextRenderer::draw_terminal` re-shaped every styled run on every row
from scratch, on every single paint, with zero caching -- unlike every
other text path in the same file. Found and adversarially verified by
the review's Performance lens; the one High-severity finding of the
four.

## Real investigation
`draw_terminal` groups each row into contiguous same-style runs and
called `build_field_layout` unconditionally per run per row -- a fresh
`parley` shape every time, consulting neither `layout_cache` nor
`monospace_cell_cache`. Trigger: `engine-py::terminal.rs`'s `drain_into`
marks the tree dirty on every batch of PTY bytes, so fast/chatty
terminal output re-shapes everything every frame. Real design
constraint: terminal rows/runs have no stable `NodeId` of their own
(only the whole terminal widget does), so the existing `NodeId`-keyed
`layout_cache` couldn't be reused directly -- needed a compound key.

## Design (1 milestone, 2 phases)
1. Per-row/per-run shaping cache + real invalidation.
2. Tests, docs, verification.

## Status

**Complete, both phases.**

New `TerminalRunKey`/`CachedTerminalRun` types + a `terminal_run_cache:
HashMap<(NodeId, u16, u16), CachedTerminalRun>` field, keyed by
(terminal NodeId, row, the run's own starting column) -- a stable real
slot across frames, not a synthetic run index that would shift every
later run on a row whenever an earlier one changed shape. New
`shaped_terminal_run` method mirrors `shaped_layout`'s own real
staleness-check contract, but -- a deliberate improvement, since this
is new code -- compares the cached key's fields against the *borrowed*
new inputs before ever allocating an owned key, rather than
`shaped_layout`'s own allocate-then-compare pattern (M66's own real,
separately-tracked target). `draw_terminal`'s own per-run call site
routed through it (`_node_id` un-underscored to `node_id`, now
genuinely used). `evict_stale_layouts` widened to also evict the new
cache: both "the terminal node was removed" (identical to
`layout_cache`'s own contract) and a real second case `layout_cache`
has no equivalent of -- any `(row, col)` that fell outside the
terminal's own current `rows`/`cols` bounds after a resize shrunk it,
checked directly against the live `NodeKind::Terminal`'s own state.

Tests: 3 new Rust unit tests (an unchanged repaint doesn't grow the
cache; changing one row's own content reshapes only that row, sibling
rows' cached content stays untouched; a resize-shrink evicts
out-of-bounds rows, a subsequent removal evicts what's left) -- new
`terminal_node`/`set_cell` test helpers mirroring `text_node`'s own
established construction pattern.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-render` 32, up from
29, +3; every other crate unchanged); `maturin develop --release`;
`pytest tests/` 831 passed, unchanged -- pure internal caching, no new
Python-facing surface, matching `layout_cache`/`monospace_cell_cache`'s
own identical "Rust-tested only" precedent; every example ran clean
including `terminal.py`; `demo/showcase.py` all 5 phases, exit 0.
`BUILD_TRACKER.md` updated (Top Metrics, full Milestone 64 section,
Just-closed/Up-next refreshed), tracker regenerated (18 milestones/55
phases/137 items/3 known gaps/25 fixed gaps), artifact republished.
Committing locally now.

Next: M65 (incremental existence tracking for `compute_layout`'s four
redundant full-tree scans).
