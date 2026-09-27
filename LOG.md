# LOG — Branch `0.3.5`: Milestone 102

- M101 complete, pushed at `2b98331`. The user: "push and start M102".

## Done

1. Phase 1 Step 0: `animate` and `get` still took the pre-0.3.5 property
   names M100 was meant to remove; they now fail, with the tests and two
   examples moved to the new names. cargo 326, pytest 428.

2. Phase 1 Step 1: ten guide pages, a switch walkthrough with
   `examples/switch.py`, the overview, Getting Started, and API reference
   rewritten; `_core.pyi` docstrings fixed; anchors validated.
3. Phase 1 Step 2: ARCHITECTURE.md, README, and docs/architecture.md
   rewritten; stale comments in 40 Rust files corrected.
4. Phase 2 Step 1: 21 self-checking examples, six new (layout, layers,
   slider, ripple, reorder, showcase), a docs Examples page.
5. Phase 2 Step 2: tests renamed and described in current terms, 4
   duplicates removed, the benchmark test migrated, alt/meta covered, CI
   runs every example. pytest 425.

## Status

**Complete (2026-09-27).** M103, the release, next.
