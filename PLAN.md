# PLAN — M72: `Node.set_layout(flex_direction=...)`

*(Replaces the prior M71 plan in this file — M71 is complete,
committed. A small, necessary addendum discovered immediately while
starting Tesserae-side Part 2 of the approved plan.)*

## Goal
No imperative way existed to set a node's own flex main axis -- no
constructor kwarg, no `Node` method, no generic `add_container`. A
Python-composed multi-child widget needing a real vertical stack had
no way to ask for one.

## Status

**Complete, both phases.**

`Node.set_layout` widened with `flex_direction: Option<&str> = None`,
`"horizontal"`/`"vertical"` -- extending M70's own declarative-layer
vocabulary fix to the imperative surface. New `parse_flex_direction`
helper mirrors `parse_align_items`. One pre-existing internal caller
(`view.rs`) updated for the new argument count.

5 new pytest tests, matching `test_live_style.py`'s own established
honesty about `set_layout` having no pixel-box readback. Full chain
green: `cargo check`/`clippy -D warnings`/`fmt --check` clean; `cargo
test --workspace --release` unchanged; `maturin develop --release`;
`pytest tests/` 855 passed, 2 skipped, up from 850, +5; every example
ran clean; `demo/showcase.py` all 5 phases, exit 0. A real, direct
dispatched-click sanity check (two children vertically stacked, both
independently clickable) also run manually. `BUILD_TRACKER.md` updated
(Top Metrics, full Milestone 72 section, Up-next refreshed), tracker
regenerated (23 milestones/67 phases/179 items/3 known gaps/25 fixed
gaps), artifact republished. Committing locally on the `0.3.1` branch
now.

Next: reinstall into `tesserae/.venv`, then resume Tesserae-side Part 2
(the widget catalog).
