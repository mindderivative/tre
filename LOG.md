# LOG — Branch `0.3.3`

- User-directed: "Scope M90 on a new 0.3.3 branch. We wont move to
  0.4.0 until vello_hybrid fix/clone feature is put into place."

## Done

1. Branch `0.3.3` created off `main` at `cb9eea2`.
2. `Cargo.toml`/`pyproject.toml` bumped 0.3.2 → 0.3.3; `Cargo.lock`
   updated via `cargo check --workspace`.
3. M90 scoped from a scripted audit of all 60 `Window.add_*` factories'
   parameters, the `Node` accessors, and every declarative schema
   field — eight findings (A–H) recorded in `BUILD_TRACKER.md` with a
   recommended fix each.

## Status

**Scoped, not started.** Waiting on the user to confirm or amend
findings A–H before Phase 1.
