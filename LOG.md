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

**Step 2 done (2026-09-28).** Nine animation tests tick real animations
through 12 frames, comparing a partial-redraw window with a full-redraw
one byte for byte every frame: opacity + corner radius, retargeted fill,
scale + shadow, shadows, translation, rotation, a group's border, path
morph + trim, scroll offset. Shrinking every rect by 3 px fails 9 of 16,
so the tests see a too-small rect. cargo 364.

Next: Step 3, measurement against v0.3.5.1.
