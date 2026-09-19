# PLAN — M35 Phase 1: Toolbars

## Goal
Scope and start the next milestone, per the user's "Start on the next
milestone." Investigated for a genuinely fresh milestone (the vello_
hybrid fork stays pure future work, per the prior turn's own explicit
scope choice) the same way M30/M32 were originally scoped: cross-
referenced TRE's existing 52-method `add_*` catalog against MD3's own
current official catalog (local scraped mirror, `m3.material.io`
itself being JS-rendered) and pyCopper's own real widget set. Five
real, ranked gaps found; user chose Button Groups + Split Button +
Toolbars -- the coherent, composition-only trio with real pyCopper
precedent and no new `NodeKind`.

## Steps (Phase 1 — Toolbars)
1. Real anatomy verified directly from the local MD3 spec mirror
   (`COMPONENT_TOOLBARS.md`) before designing anything: 64dp height
   both variants; docked = full window width, square corners,
   `surface_container`/`primary_container` fill; floating = hugs
   content, fully rounded, real elevation, horizontal or vertical.
2. Read the existing `add_top_app_bar` (M30 Phase 5 Step 3) as the
   direct structural precedent to follow -- same `resolve_*_colors`
   pattern (`resolve_fab_colors`), same `role(name, fallback)` theme
   closure, same `positioned_style` construction.
3. Implemented `Window.add_toolbar(variant, orientation, color, width,
   height, x, y) -> Node` in `window_factory.rs`, right after `add_
   top_app_bar`. Real validation: unknown variant/orientation/color
   each raise a clear `ValueError`; a vertical *docked* toolbar
   (a real MD3 anatomy that doesn't exist) also raises rather than
   silently ignoring the param.
4. A real "container with configurable slots" per MD3's own anatomy,
   verbatim -- no specialized children-list parameter; the caller
   composes already-built nodes in via the existing, generic `Node.
   add_child` (M6 Phase 1), the same split `clip_children` (M32 Phase
   3) already established.
5. New constants: `TOOLBAR_HEIGHT` (64.0), `TOOLBAR_PADDING` (16.0,
   the spec's own "minimum outside padding"), `TOOLBAR_ITEM_GAP`
   (32.0, the spec's own "equal padding between items" default).
   Elevation reuses `FAB_REST_ELEVATION_LEVEL` (3.0) -- a real, honest
   gap stated directly: no discrete numeric elevation token exists in
   the scraped spec for "floating toolbars have elevation by default."
6. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, a real empirical script before any pytest, `tests/
   test_toolbar.py` (10 tests), `examples/toolbar.py`, full pytest
   suite, all examples, showcase demo, mypy --strict.
7. `BUILD_TRACKER.md` (M35 scoped, Phase 1 closed), artifact
   republish, memory update, commit (holding push -- M35 has two more
   phases before the milestone closes).

## Status
Phase 1 complete. Full verification chain green (`pytest tests/` 537
passed/1 skipped, 10 new, zero regressions; all 72 examples + showcase
demo clean; mypy --strict clean). **M35 is not yet closed -- Phase 2
(Split Button) and Phase 3 (Button Groups) remain.**
