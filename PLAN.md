# Plan: M27 Phase 5 — Accessibility Pass & Polish, closing M27

Corresponds to `BUILD_TRACKER.md` M27 Phase 5. Written retroactively
alongside implementation — see `LOG.md` and `BUILD_TRACKER.md`'s own
Phase 5 entry for the complete real investigation, findings, and
verification record.

## What changed

- `demo/showcase.py`: each screen's `verify_*_screen` function gained
  a real, comprehensive Tab-order sweep across all of its own real
  interactive controls (8/5/3), plus a real keyboard `Enter`-press
  activation on one representative control per screen.
- Real finding, confirmed empirically first: a screen swap resets
  focus to none, so each screen's own sweep re-consumes the 3
  nav-button Tab stops before its own content.
- Deduplicated the `label()` helper (three near-identical local
  closures) into one shared `make_label_fn(window, screen)` factory.
- Corrected M27's own Phase 5 scoping text ("four content screens" →
  the real three).
- Made the finished demo discoverable: pointers added to `README.md`
  and `docs/index.md`, plus a stale milestone-count correction there.

See `LOG.md` for the full narrative and verification results.
