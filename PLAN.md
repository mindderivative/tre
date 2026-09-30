# PLAN — Branch `0.4.2`: Milestones 11–13

*(Replaces the M9 plan — `v0.4.1` is released. Every step is in
`BUILD_TRACKER.md`.)*

User (2026-09-29): "yes, scope them, we will stay on 0.4.x, they both can
be on 0.4.2".

## M11 — CSS Grid (issue #23)

Taffy 0.14 already implements grid; expose it in `node_layout.rs`.
1. `display`; `grid_template_columns`/`rows` (string or list: px numbers,
   `fr`, `%`, `auto`, `min_content`, `max_content`, `minmax()`,
   `repeat()`); `grid_column`/`grid_row` (line, `span n`, `a / b`, `auto`).
2. `grid_auto_rows`/`columns`, `grid_auto_flow`, `row_gap`/`column_gap`,
   `justify_items`, `justify_self`, `align_content`; `get()` round-trips.
3. Tests, layout guide, `examples/grid.py`, stub.

## M12 — Scroll views (issue #24)

Decided: keys scroll the nearest scroll view around the focused node when it
doesn't use the key; `scroll` fires on any change, found by comparison;
keyboard/`scroll_into_view` scrolls jump.
1. `Key::PageUp`/`PageDown`.
2. Keyboard scrolling: arrows 40 px, Page Up/Down a viewport, Home/End.
3. `node.scroll_into_view()` + the a11y action + revealing focus.
4. The `scroll` event (`old_value`/`new_value`).
5. Docs note on scroll-view layout; tests; examples.

## M13 — Release `0.4.2`

Each step: the full chain, docs, tracker, a local commit.

1. Documentation audit: every MkDocs page, README, ARCHITECTURE, the stub (done).
2. The project named Tesserae Engine in the docs (done).
3. PR to `main`, CI, merge, tag `v0.4.2`, release, PyPI -- after the user says to push.
4. Tesserae moves to `0.4.2`.
