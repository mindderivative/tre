# LOG — Branch `0.3.5.2`: Milestone 105

- Tesserae filed issue #16 (its M52 hot reload can't drop a panel). The user:
  "Take a look at main branch issue #16 about undocking panels", then "yes"
  to M105 on a new `0.3.5.2` branch off `main`.
- Worked in a separate worktree (`../tre-0.3.5.2`, its own `.venv`), so the
  `0.4.0` checkout and its build were left alone.
- `take_out_of_zone` shared by `move_panel` and the new `undock_panel`; it
  lowers `active_tab` when an earlier panel leaves (a shipped M104 bug).
- A destroyed docked panel panicked `set_active_panel` (`add_child: child
  NodeId not found`); `forget_freed` runs first in every docking entry point.

## Status

**Complete (2026-09-27).** Push and release wait on the user.
