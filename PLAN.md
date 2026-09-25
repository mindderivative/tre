# PLAN — Branch `0.3.5`: Milestone 97 Phase 2, Tesserae Migrates

*(Replaces the M96 plan — M96 is complete and released as `v0.3.4`. Every
step is in `BUILD_TRACKER.md`.)*

## Goal

Give Tesserae what it needs to stop using everything `0.3.5` removes, and a
way to prove it has: a migration guide in the form Tesserae asked for, and a
switch that makes every removed name raise.

## Design

- **Widget mapping, generated.** `tools/dump_widget.py` dumps any node's
  subtree through the new `get()`, as diffable JSON. Run on each of the 57
  legacy `add_*` factories at their defaults, it produces the reference;
  run on Tesserae's rebuilt widgets, it's the check.
- **Written from the source,** only what a static tree can't show:
  interaction timings, state-change animations, and how the Rust-painted
  kinds draw.
- **`engine-md3` handover as data:** colour science, type/shape/motion
  scales, and the icons as SVG path strings with their view box.
- **Audit** of `binding.rs`/`cascade.rs` against their doc comments.
- **Gate switch:** `TRE_FORBID_REMOVED=1`, read in `tre/__init__.py`,
  replaces every removed name with a stub raising `AttributeError` that
  names its replacement. The list lives in `tre/_removed.py`; a test keeps
  it in step with the spec's migration table.

## Status

Scoped (2026-09-25). Step 1 next.
