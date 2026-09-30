# LOG — Branch `0.4.2`: Milestones 11–13

## Status

**Scaffolded (2026-09-29).** Branch `0.4.2` off `main` at `30c7f27`,
versions 0.4.2. User: scope #23 (Grid) and #24 (scroll views) for 0.4.2;
decisions: nearest scroll view for keys, `scroll` on any change by
comparison, jump (no easing).

**M11 complete (2026-09-29).** grid.rs (parse/format, 4 unit tests),
13 layout properties, tests/test_grid.py (15), docs, examples/grid.py.
cargo 377, pytest 467 + 1 skipped. Next: M12.

**M12 complete (2026-09-29).** Page Up/Down keys; keyboard scrolling of the
nearest scroll view; `scroll_into_view`, its a11y action, and focus reveal;
the `scroll` event (`old_value`/`new_value`); guide, ARCHITECTURE §11.7a,
`examples/scroll_keys.py`. The planned `flex_shrink=0` docs note was wrong
and dropped. cargo 377, pytest 481 + 1 skipped.

**Docs audit (2026-09-29).** Before the 0.4.2 release: every MkDocs page,
README, ARCHITECTURE, and the stub checked against the code, snippets run.
Next: M13 (release 0.4.2).
