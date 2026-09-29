# PLAN — Branch `0.4.1`: Milestone 9, Housekeeping and Release `0.4.1`

*(Replaces the M8 plan — M8 is complete. Every step is in
`BUILD_TRACKER.md`.)*

## Steps

1. Housekeeping: retire the local `0.3.5.3` branch and its `tre-0.3.5.2`
   worktree (its sdist fix shipped in `0.4.0`); prune merged local release
   branches; `rect_window.rs` onto the shared test helpers where it can.
2. The user's manual check of partial redraw on real hardware -- X11,
   Wayland, macOS, Windows: `partial_redraw_active`, repaint on uncover,
   resize, and minimize; `show_damage` makes it visible.
3. PR to `main`, CI green, merge, tag `v0.4.1`, release, PyPI (the user
   approves the `pypi` deployment).
4. Tesserae moves to `0.4.1`.
