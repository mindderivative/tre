# PLAN — Branch `0.3.5`: Milestone 99, Remove MD3 Components, Kinds, and Theming

*(Replaces the M98 plan — M98 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Take Material Design 3 out of `tre`: the composed widget factories, the
MD3-specific node kinds and their engine behavior, theming, and the
`engine-md3` crate. `tre` keeps the building blocks Tesserae rebuilt them on.
Tesserae confirmed (M97, CI run 36293860118) it uses none of it.

## Decisions (the user)

- M98's: `Window.set_theme` stayed until now; it goes here, with
  `theme_spec.rs`, `default_theme.yaml`, `serde_yaml_ng`, and `pythonize`.
- M99's (2026-09-26, "Delete them now"): the 61 example files that use what
  M99 removes, `demo/showcase.py` included, are deleted; M102 writes a new,
  smaller set and a new showcase on the final building blocks.

## Steps

**Phase 1 — Factories and theming**
1. The MD3 composition factories, the legacy overlay `open_*`/`close_*`
   methods and `build_menu`, `build_shell`, the container transform, the
   `Theme` class, `set_theme`, `window.theme`, the retheme hooks, and the
   theme parsing that came from `engine-spec`.
2. Docking reduced to D10's bare bones: `set_active_panel`,
   `start_panel_drag(panel)`, and new `dock_target`/`dock_drop` window
   events replace the tab strip, drag handle, highlight, and the scripted
   drag methods.
3. The `engine-md3` crate (and `add_icon`, whose icons live there); the MD3
   handover's generators retire with it, and its pages stay as the record.

**Phase 2 — Engine-side MD3 behavior**
1. The MD3 kinds -- `Checkbox`, `RadioButton`, `Switch`, `Slider`, the
   three progress indicators, `LoadingIndicator`, `TimePickerDial`,
   `Carousel`, `Splitter`, `Link`, and `Icon` (D4: a `path` does it) --
   with their dispatch, ticking, painting, accessibility, and `Node`
   methods.
2. The interaction state layer and ripple (D8), the MD3 elevation levels,
   the shape-morph library, and the named motion curves.
3. File conveniences (D6); the full standing chain.

Each step: the examples and tests for what it removes go with it, the docs
follow, the tracker, a local commit, memory.

## Status

Phase 1 and Phase 2 Step 1 done (2026-09-26). Step 2, the MD3 behavior, next.
