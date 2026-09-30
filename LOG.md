# LOG — Branch `0.4.3`: Milestones 14–17

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

**M16 complete (2026-09-30).** Issue #27 (from Tesserae's M74): a bare
number is a one-track list for the four track-list properties. 3 pytest
cases; property reference, grid guide, 0.4.3 migration section. cargo
377, pytest 494 + 1 skipped. Release is now M17, still held. Next: M15.

**M15 complete (2026-09-30).** `Tree::max_scroll`; `Node.set` and
`animate` clamp `scroll_offset` at once (one `scroll` event; animations
ease to the real end); `create` sets `reported` to its offset. 5 pytest
cases; property reference, guide, migration section, ARCHITECTURE §11.7a.
cargo 377, pytest 499 + 1 skipped. PR #26 merged into `main` and `main`
merged into 0.4.3. Next: M17 (release), held for more fixes.

**Scoped (2026-09-30).** User: "Scope 1, 2, and 3 for 0.4.3". M17
Shift+wheel horizontal, M18 core scroll tests, M19 stub drift in CI; the
release is now M20, after them if nothing else joins. Next: M17.

**M17 complete (2026-09-30).** `shift_wheel` in engine-py turns Shift+wheel
(no x part) horizontal before dispatch; core's wheel walk passes a view it
can't move on (found: a carousel in a page swallowed a plain wheel). 5
pytest cases plus Shift on the terminal scrollback wheel; guide, events,
migration, ARCHITECTURE §11.7a. cargo 377, pytest 504 + 1 skipped.
Next: M18.

**M18 complete (2026-09-30).** 9 engine-core tests for `max_scroll`,
`scroll_view_for_key`, `scroll_by_key`, `scroll_into_view`,
`take_scroll_changes`, and M17's wheel rule (shown to fail without it).
cargo 386, pytest 504 + 1 skipped. Next: M19.

**M19 complete (2026-09-30).** Stub: `__all__`, `@final` on the six PyO3
classes, `Window.__new__`, positional-only `Node.__eq__`;
`tools/stubtest_allowlist.txt` (`Color`). CI's Linux job runs `mypy
--strict` and `stubtest`; contributing guide and README list stubtest.
stubtest 21 -> 0. Next: M20, the release, if nothing else joins.
