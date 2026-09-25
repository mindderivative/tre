# LOG — Branch `0.3.4`

- User-directed: "Approved, release 0.3.3 and start M93."

## Done

1. `v0.3.3` released: PR #9 merged as `bc5e9b6` after CI went green on
   all three platforms; `main` re-verified; release note `73e0711`;
   annotated tag `v0.3.3` on the merge commit, pushed.
2. Branch `0.3.4` created off `main` at `73e0711`; version bumped
   0.3.3 → 0.3.4; `Cargo.lock` updated via `cargo check`.
3. M93 started: a raw inventory of all 171 public signatures generated
   from `_core.pyi`, plus the engine's internal input events, dispatch
   outcomes, node kinds, paint properties, motion curves, and the docking
   model, read from source.

4. Phases 1–2 done: `docs/design/target-api.md` — naming convention,
   R1–R8, classes, nine node kinds, properties, events, Window, docking,
   MD3 rebuild table, Tesserae's needs, full migration table (`5d8eb59`).
5. Tesserae reviewed the spec: "approve with changes", 15 items; the
   user agreed all 15. Revision 2 folds them in, adds R9–R12, and places
   each item in M94–M96 in `BUILD_TRACKER.md`. All 176 public names still
   covered; `mkdocs build --strict` clean.

## Status

**M93 Phase 3 Step 2: awaiting the user's approval of revision 2.**
