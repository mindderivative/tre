# PLAN — M30 Phase 4 Step 1: Dialog

## Goal
Add `Window.add_dialog`/`open_dialog`/`close_dialog` — a real MD3 modal
dialog. Distinct from every existing overlay in this catalog: a real
modal must block interaction with everything behind it, which the
existing `dismiss_on_outside_click` model doesn't do on its own.

## Steps
1. Investigate the real gap in `Tree::dispatch`/`dismiss_overlays_outside`
   directly — confirm there is genuinely no way today to block
   background interaction without also dismissing on outside click.
2. Add `OverlayMeta.modal: bool` (purely additive, `false` = no-op) and
   `Tree::press_blocked_by_modal_overlay`, wired into `dispatch`'s
   `PointerPressed` arm right after the existing outside-dismiss check.
3. Update all pre-existing `OverlayMeta { ... }` construction sites with
   `modal: false` to preserve their exact existing behavior.
4. Write two new `engine-core` unit tests proving the fix and proving
   `modal: false` is a true no-op (regression guard).
5. Verify real MD3 Dialog tokens via WebFetch against Material Web's own
   `_md-comp-dialog.scss` before writing any component code.
6. Implement `add_dialog`/`open_dialog`/`close_dialog` in
   `window_factory.rs`, reusing `open_overlay`'s anchor-relative math via
   a synthetic zero-size anchor pinned at the origin, combined with flex
   centering on a full-window scrim.
7. Add `.pyi` stubs for all three methods.
8. Write `tests/test_dialog.py`, including a real end-to-end click-
   dispatch proof that the modal blocking actually works through the
   Python API, not just that the calls don't raise.
9. Write `examples/dialog.py`, headless-CI-safe.
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
11. Update `BUILD_TRACKER.md` (Top Metrics row, phase heading, step
    line), regenerate + republish the Build Tracker artifact.
12. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (308 pytest
passed/1 skipped, 44 examples, showcase demo, 43 Rust test binaries).
