# LOG — Branch `0.3.5`: Milestone 99

- M97 complete: Tesserae confirmed, with the switch in CI, that it uses
  nothing 0.3.5 removes. The user: "push and start M99".
- The user, on the 61 example files M99 affects: "Delete them now" -- M102
  writes the new set and showcase.

## Done

1. Phase 1 Step 1: the MD3 factories, legacy overlays and menus, the shell,
   the container transform, theming, and `set_context_menu` removed; 62
   examples deleted; cargo 453, pytest 491, 15 examples, docs clean.
2. Phase 1 Step 2: docking reduced to D10's bare bones -- `set_active_panel`,
   `start_panel_drag(panel)`, and the `dock_target`/`dock_drop` window events;
   the drag moved into `process_input`, so live and simulated drags share
   one path; cargo 453, pytest 492, 15 examples, docs clean.
3. Phase 1 Step 3: `engine-md3` deleted, with `add_icon` and `add_text`'s
   `typography_role` (Tesserae uses neither); cargo 426, pytest 477, 15
   examples, docs clean.
4. Phase 2 Step 1: the 12 MD3 kinds deleted from `engine-core`,
   `engine-render`, and `engine-py`, with their `Node` methods; cargo 363,
   pytest 468, 15 examples, docs clean.
5. Phase 2 Step 2: the state layer and ripple, elevation, the shape
   library, and the MD3 curves deleted; two examples rewritten on shadows
   and path morphing; cargo 326, pytest 465, 15 examples, docs clean.
6. Phase 2 Step 3: `add_image(path)`, the `image` crate, and the Working
   with Files page removed (D6); full chain: cargo 326, pytest 456, 15
   examples, mypy and mkdocs clean, 85 names documented.

## Status

**Complete (2026-09-26).** M100, the naming convention, is next.
