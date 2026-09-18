# PLAN — M30 Phase 4 Step 2: Snackbar

## Goal
Add `Window.add_snackbar`/`open_snackbar`/`close_snackbar` — a real
MD3 transient notification, anchored to the desktop bottom-left
corner. Unlike every prior overlay component, its action and close
affordances must be independently clickable, not decorative.

## Steps
1. Verify real MD3 Snackbar tokens via WebFetch against Material
   Web's own `_md-comp-snackbar.scss`.
2. Trace `inverse-primary` (no direct hex in the snackbar token file)
   through `_md-sys-color.scss` and `_md-ref-palette.scss` to its real
   hex (`#D0BCFF`, `primary80`).
3. Check for an existing timer/scheduler primitive (grep) — confirm
   none exists, so auto-dismiss-after-duration is out of scope, the
   app's own responsibility.
4. Check `Chip`'s `removable` icon precedent (decorative, not
   independently clickable) and `Segmented Button`'s `Vec<Node>`
   precedent (multiple independently-interactive nodes) before
   deciding `add_snackbar`'s own return shape.
5. Implement `add_snackbar` (returns `(container, action, close)`,
   `action`/`close` real independent `Node`s when requested) and
   `open_snackbar`/`close_snackbar` (reusing `open_dialog`'s synthetic-
   anchor technique, positioned bottom-left instead of centered).
6. Add `Md3Baseline::INVERSE_PRIMARY`.
7. Add `.pyi` stubs for all three methods.
8. Write `tests/test_snackbar.py`, including real click-dispatch
   proofs that `action`/`close` are independently clickable.
9. Write `examples/snackbar.py`, headless-CI-safe.
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
11. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
    + republish the Build Tracker artifact.
12. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (318 pytest
passed/1 skipped, 45 examples, showcase demo, 43 Rust test binaries).
