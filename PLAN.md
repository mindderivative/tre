# PLAN — Branch `0.3.4`: Milestone 93, Target API Spec and Naming Convention

*(Replaces the M87 plan — `v0.3.3` is released. The approved M93–M103
program, its decisions D1–D11, and every step live in `BUILD_TRACKER.md`'s
"Program" section.)*

## Goal

Design only, no code. Produce the target API of `tre` as a minimal
building-block engine: every surviving class, function, property, event,
and animation, with final names under one written naming convention, and
an old-to-new migration table covering every current public name.

## Phases

1. **Inventory and classification:** every current public name classed as
   keep, replace-with-primitive, move-to-framework, or remove; the exact
   primitives each MD3 widget needs; the bare-bones docking surface (D10);
   every Tesserae capability mapped to a named primitive.
2. **Naming convention:** the written rules, applied to every survivor.
3. **Spec review:** published as a docs design page, sent to Tesserae,
   approved by the user. Nothing in M94 onward starts before approval.

## Deliverable

`docs/design/target-api.md`.

## Status

Phases 1–2 done. Phase 3: revision 2 (Tesserae's review folded in)
awaits the user's approval of R1–R12.
