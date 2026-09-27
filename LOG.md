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

## Status

**In progress.** Phase 1 Step 3 (`engine-md3`) next.
