# PLAN — Branch `0.4.1`: Milestone 8, Show What's Redrawn

*(Replaces the M7 plan — M7 is complete: measured, nothing worth cutting.
Every step is in `BUILD_TRACKER.md`.)*

## Goal

`window.set(show_damage=True)` makes each presented frame show what it
redrew: the damage rects tinted, a whole-window redraw outlined. For seeing
partial redraw work and finding wasted redraws.

## Design

- The tint is drawn into the surface image after the kept frame is copied
  there -- never into the kept frame, so the next partial frame starts clean
  and the pixels partial redraw keeps are unaffected.
- `Rects`: each rect filled translucent magenta, outlined. `Full`: the
  window's edge outlined. `None`: nothing presented (unchanged).
- A per-window `Rc<Cell<bool>>` in `WindowHandles`, read each frame, like
  `partial_redraw`; `get("show_damage")`.
- Tests: a GPU test that the surface shows the tint inside the rect and the
  next frame (overlay off) matches a full render; pytest for set/get.
