# LOG — Branch `0.4.3`: Milestones 14–16

## Status

**Scaffolded (2026-09-30).** Branch `0.4.3` off `main` at `6b7c70f`,
versions 0.4.3. User: scope Tesserae's two 0.4.2 observations as 0.4.3 --
modified keys scrolling (M14), and `scroll_offset` not clamped when set
(M15), plus the create-time offset missing from the `scroll` event's
baseline, found while scoping. Next: M14.

**M14 complete (2026-09-30).** `keyboard_scroll` leaves keys with Ctrl,
Alt, or Meta held alone; Shift still scrolls. 10 pytest cases; guide,
events guide, 0.4.3 migration section, ARCHITECTURE §11.7a. cargo 377,
pytest 491 + 1 skipped. Release held for more fixes (user). Next: M15.
