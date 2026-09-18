# PLAN — M30 Phase 8 Step 5: Status Bar

## Goal
Add `Window.add_status_bar` — MD3 has no official page. Real,
deliberate reuse of AppShell's own already-real `status_bar` region
(build_shell, §14 step 13) rather than new shell-level wiring.

## Steps
1. Confirm no official MD3 Status Bar page.
2. Confirm build_shell's own status_bar parameter already exists and
   accepts a pre-built Node directly (re-read its signature).
3. Design: thin 24dp bar, surface_container fill, Label Small text
   (reuse BADGE_LABEL_FONT_SIZE/_WEIGHT from Badge's own earlier
   finding).
4. Implement `add_status_bar` in `window_factory.rs`.
5. Add `.pyi` stub.
6. Write `tests/test_status_bar.py`, including a real test proving
   the returned Node passes directly into build_shell's own status_bar
   parameter with no adapter.
7. Write `examples/status_bar.py`, composing a real menu bar, toolbar,
   and status bar together into one AppShell.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, step line), regenerate
   + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (440 pytest
passed/1 skipped, 61 examples, showcase demo, 43 Rust test binaries).
