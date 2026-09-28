# LOG — Branch `0.4.0`: Milestone 5

- M4 complete: `DamageTracker` (compare painted rects and fingerprints,
  merge to 4 rects or full) with 18 tests; cargo 348.

## Status

**Step 1 done (2026-09-28).** Each window's frame computes its damage
after layout: `None` re-presents the kept frame, `Rects` renders a scene
clipped to the rects with `ClearSettings::Rects`, `Full` as before. Resets
on target recreate, font registration, and a failed surface acquire.
`window.set(partial_redraw=False)` switches it off. Seven GPU tests find
partial redraw byte-identical to full; a live animation renders 1 full
frame then 59 partial ones. cargo 355, pytest 431 + 1 skipped, 20 examples,
mkdocs strict, mypy strict.

Next: Step 2, partial vs full across the examples' animations.
