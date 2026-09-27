# PLAN — Branch `0.3.5.1`: Milestone 104, `dock_panel` Moves a Docked Panel

*(Replaces the M103 plan — M103 is complete. Every step is in `BUILD_TRACKER.md`.)*

Issue #14: `dock_panel` on a panel docked elsewhere left it in its old zone's
list, and `set_active_panel` then put it under two parents. Fix: `dock_panel`
moves it as a drop does, and `Tree::add_child` moves an attached node.

## Status

**Released (2026-09-27).** `v0.3.5.1` is out; Tesserae told.
