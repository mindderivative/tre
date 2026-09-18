# Plan: M27 Phase 2 — MD3 Component & Theming Gallery Screen

Corresponds to `BUILD_TRACKER.md` M27 Phase 2. Written retroactively
alongside implementation (moved directly from scoping to building this
phase) — see `LOG.md` and `BUILD_TRACKER.md`'s own Phase 2 entry for
the complete real investigation, findings, and verification record.

## What changed

- New `Window.add_text(...)` (`crates/engine-py/src/window.rs`) — a
  real, genuine gap found while building this screen: no imperative
  way to create a plain `NodeKind::Text` label existed before this.
- `demo/showcase.py`'s "components" placeholder replaced with
  `build_gallery_screen`: `Checkbox`/`Slider`/`TextField`/`Image`/
  `Icon` all live, plus a real seed-color/dark-mode theme picker
  calling `Window.set_theme`.
- Two real, connected bugs found and fixed by actually running it:
  `Node.animate(..., duration_ms=0)` needs a tick to land (no render
  loop running yet at verification time) — worked around with a real
  Tab-focus + `ArrowRight` nudge instead, mirroring `examples/
  slider.py`; and the gallery's own Tab order starts after the two
  nav buttons, not at the gallery's own first control.
- `docs/api/python/window.md` updated with the new `add_text` method.

See `LOG.md` for the full narrative and verification results.
