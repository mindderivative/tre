# PLAN — M30 Phase 7 Step 1: Date Picker

## Goal
Add `Window.add_date_picker_day` — the docked Date Picker variant's
own real day-cell anatomy, deliberately scoped to just the cell (no
engine-owned calendar arithmetic).

## Steps
1. Find the real Date/Time Picker token files via a GitHub directory
   listing (docked vs modal variants for Date Picker; time-input vs
   time-picker for Time).
2. Fetch the real docked-variant day-cell tokens: 48x48dp,
   corner-full, selected (primary fill/on_primary label), today
   (1dp primary outline/primary label), outside-month
   (on_surface_variant).
3. Decide scope: only the day cell is real new anatomy. A real
   calendar grid needs date arithmetic that's genuinely app logic
   (Python's own calendar/datetime modules) -- no engine-owned
   calendar primitive.
4. Implement `add_date_picker_day` in `window_factory.rs`.
5. Add `.pyi` stub.
6. Write `tests/test_date_picker.py`, including a real test building
   a genuine month grid via Python's calendar.monthcalendar.
7. Write `examples/date_picker.py`, headless-CI-safe, positioning
   cells into a real 7-column grid.
8. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all examples, showcase demo, mypy
   --strict.
9. Update `BUILD_TRACKER.md` (Top Metrics row, Phase 7 heading now
   🚧, step line), regenerate + republish the Build Tracker artifact.
10. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (407 pytest
passed/1 skipped, 55 examples, showcase demo, 43 Rust test binaries).
