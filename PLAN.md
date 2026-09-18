# PLAN — M30 Phase 7 Step 2: Time Picker (closes Phase 7)

## Goal
Add `Window.add_time_input_field`/`add_period_selector` — the real
MD3 Time Input variant (digital hour:minute entry), not the analog
clock-face dial.

## Steps
1. Confirm the real MD3 Time variants (time-input vs time-picker) via
   the same GitHub directory listing Date Picker's own investigation
   already found.
2. Decide scope: Time Input, since the analog dial needs a genuinely
   new drag-to-angle engine capability this project doesn't have.
3. Fetch real Time Input tokens: 96x72dp field, surface_container_
   highest, corner-small, Display Medium label (45px/400 weight);
   period selector 52x72dp, corner-small, tertiary_container selected.
4. Design: reuse TextField directly for the hour/minute field
   (Search Bar's own precedent); period selector as two independent
   Rect+Text buttons (honest caveat: exact sub-anatomy beyond overall
   dimensions wasn't in the fetched tokens).
5. Implement `add_time_input_field`/`add_period_selector` in
   `window_factory.rs`.
6. Add `.pyi` stubs for both.
7. Write `tests/test_time_picker.py`, including a real typing/focus
   round-trip proving TextField reuse actually works.
8. Write `examples/time_picker.py`, headless-CI-safe.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
10. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 7 heading now
    ✅, Step 2 and Step 3 both closed), regenerate + republish the
    Build Tracker artifact. Closes Phase 7 entirely.
11. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (415 pytest
passed/1 skipped, 56 examples, showcase demo, 43 Rust test binaries).
Phase 7 (Date & Time) is now fully complete, all 3 steps.
