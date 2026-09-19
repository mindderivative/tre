# PLAN — M32 Phase 5: Terminal Scrollback

## Goal
Close the real, stated v1 gap M30 Phase 9 Step 4 (Terminal) named: no
scrollback. A user must be able to scroll a terminal's own viewport
back into history and see real, previously-scrolled-off output.

## Steps
1. Investigated the vendored `vt100 = "0.16.2"` source directly before
   writing any code: it already has a real, built-in scrollback buffer
   (`Grid.scrollback: VecDeque<Row>`, `Parser::new`'s own third
   `scrollback_len` parameter, `Screen::set_scrollback`/`scrollback()`)
   -- simply never turned on (`Parser::new(rows, cols, 0)`). `Screen::
   cell`/`rows()` already read from the current real `scrollback_offset`
   internally, and a new line pushed while scrolled back auto-adjusts
   the offset to keep the viewer's own position stable (confirmed via
   direct source read of `Grid`'s own row-push logic) -- both real,
   already-correct behaviors this phase only needed to expose, not
   build.
2. `TerminalSession::spawn` gained a real `scrollback_lines: usize`
   parameter, threaded through to `vt100::Parser::new`. `add_terminal`
   gained the matching `scrollback_lines: usize = 1000` parameter.
3. Refactored `drain_into`'s cell-extraction logic into a shared
   `sync_state` helper (`TerminalSession`) -- a real scroll changes
   what `Screen::cell` returns with zero new PTY bytes involved, so it
   needs its own real sync call, not just `drain_into`'s own "only
   when new bytes arrived" gate.
4. Added `TerminalSession::scroll_by(tree, node_id, delta_lines)`:
   moves the real scrollback position and immediately re-syncs
   `TerminalState`. The position arithmetic itself is a pure,
   dedicated `scrollback_target` free function (unit-testable without
   a real PTY).
5. Wired real scrolling into both real input paths: `app.rs`'s
   `on_input` (a real mouse wheel over a `Terminal`, hit-tested at the
   wheel's own position -- `Tree::dispatch`'s own `VirtualList`/
   `Carousel` wheel-bubbling already ran harmlessly for this same
   event, a true no-op for a `Terminal` with neither ancestor) and the
   synthetic, no-live-window `Window.scroll(node, delta_y)` (checks if
   `node` is itself a `Terminal` first, bypassing `Tree::dispatch`
   entirely for that case).
6. Real Rust unit tests (`scrollback_target`): positive delta moves
   further into history, negative moves back toward the bottom, never
   underflows past `0`.
7. Real, direct empirical script before pytest: generated more real
   shell output than a 5-row terminal's viewport could hold, confirmed
   the bottom view showed only recent lines, then confirmed `Window.
   scroll` revealed the real, previously-scrolled-off first lines --
   and discovered along the way that **no second `App.run()` call is
   needed at all**, since `scroll_by` re-syncs state synchronously.
8. Extended `test_terminal.py`'s own sole `App.run()`-based test again
   (now proving three real claims: shell response, Ctrl+C/SIGINT,
   scrollback) plus 2 new synchronous (no `App.run()`) tests.
9. Extended `examples/terminal.py` with the identical real scrollback
   proof, updating its own module doc comment (removed the stale "no
   scrollback" line). Also fixed a separately-noticed stale "no
   scrollback, no Ctrl+C" line in `add_terminal`'s own `.pyi` docstring
   (missed during Phase 4, corrected here since already touching this
   exact text) and updated `Window.scroll`'s own `.pyi` docstring.
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite, checked for cross-test pollution),
    all 71 examples, showcase demo, mypy --strict.
11. Update `BUILD_TRACKER.md`, regenerate + republish the artifact,
    update memory, commit.

## Status
Complete. All steps done; full verification chain green (`engine-py`
gains 3 new unit tests, `pytest tests/` 519 passed/1 skipped, up from
517, all 71 examples, showcase demo, mypy --strict clean). A real
5-row terminal's own scrolled-off history was genuinely revealed by a
real scroll, not simulated.
