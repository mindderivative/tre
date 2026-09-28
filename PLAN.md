# PLAN — Branch `0.3.5.2`: Milestone 105, `undock_panel`

*(Replaces the M104 plan — M104 is complete and released as `v0.3.5.1`. Every step is in `BUILD_TRACKER.md`.)*

Issue #16: nothing takes a panel out of docking. `node.remove()` takes it off
the tree but leaves it in its zone's list, so `set_active_panel` shows it
again. Fix: `Window.undock_panel(panel)`, sharing `move_panel`'s take-out
path; that path now keeps the shown panel when an earlier one leaves, and a
freed panel leaves its zone instead of panicking `add_child`.

## Status

**Complete (2026-09-27).** Awaiting the user's word to push and release `0.3.5.2`.
