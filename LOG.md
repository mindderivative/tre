# LOG — M32 Phase 5: Terminal Scrollback

- Investigated the vendored `vt100 = "0.16.2"` source directly before
  designing anything: real, built-in scrollback already exists
  (`Grid.scrollback: VecDeque<Row>`, `Parser::new`'s own third
  `scrollback_len` param, `Screen::set_scrollback`/`scrollback()`) --
  `TerminalSession::spawn` was simply calling `vt100::Parser::new(rows,
  cols, 0)`, always zero. `Screen::cell`/`rows()` already read from the
  current real `scrollback_offset` internally (confirmed via direct
  read of `Grid`'s own row-lookup logic), and pushing a new line while
  scrolled back auto-increments the offset to keep the viewer's own
  position stable rather than jumping to the bottom -- both real,
  already-correct behaviors, not something this phase needed to build.
- `TerminalSession::spawn` gained a real `scrollback_lines: usize`
  parameter, threaded to `vt100::Parser::new`. `add_terminal` gained
  the matching `scrollback_lines: usize = 1000` parameter (a real,
  sensible default, not a hardcoded internal-only choice).
- Refactored `drain_into`'s cell-extraction logic (rebuilding
  `TerminalState` from the parser's current `Screen`) into a shared
  `sync_state` private method -- a real scroll changes what `Screen::
  cell` returns with zero new PTY bytes involved, so it needs its own
  real sync call independent of `drain_into`'s own "only when new
  bytes arrived" early-return gate.
- Added `TerminalSession::scroll_by(tree, node_id, delta_lines)`. The
  real position arithmetic itself (`current` + signed `delta_lines`,
  clamped at the lower bound since `usize` can't go negative) was
  extracted as a pure, dedicated `scrollback_target(current, delta)`
  free function specifically so it's unit-testable without spawning a
  real PTY/shell -- the identical "Rust proves the pure logic, a real
  script/pytest proves the live integration" split this whole session
  already established for `test_checkbox.py`'s own precedent.
  `vt100::Screen::set_scrollback`'s own real clamping (confirmed via
  direct source read: "clamped to the actual size of the scrollback")
  covers the upper bound, so `scrollback_target` only needed the lower
  one.
- Wired real scrolling into both real input paths: `app.rs`'s
  `on_input` closure gained a new `InputEvent::Scroll` arm -- hit-tests
  at the wheel's own real position, checks whether the hit node is a
  `Terminal`, and if so calls `scroll_by` directly. `Tree::dispatch`'s
  own existing `VirtualList`/`Carousel` wheel-bubbling already ran
  harmlessly for this same event just above (a true no-op for a
  `Terminal`, which has neither ancestor kind) -- no interference. The
  synthetic, no-live-window `Window.scroll(node, delta_y)`
  (`window_input.rs`) got the identical real check: if `node` is
  itself a `Terminal`, bypass `Tree::dispatch` entirely and call
  `scroll_by` directly (simpler than the live path -- no hit-test
  needed, `node` names the target explicitly).
- Real Rust unit tests (`scrollback_target`, 3 new): positive delta
  moves further into history, negative moves back toward the bottom,
  a pathological huge negative delta and an already-at-bottom scroll
  both clamp to `0` rather than underflowing. All passed on the first
  run.
- Full Rust verification chain green: `cargo check`/`clippy -D
  warnings`/`fmt --check` clean.
- Rebuilt the Python extension. Ran a real, direct empirical script
  before writing any pytest: a real 5-row terminal, filled with more
  real shell output than its viewport could hold -- confirmed the
  bottom (unscrolled) view showed only the most recent lines, then
  confirmed a real `Window.scroll` call revealed the real, previously-
  scrolled-off first lines. **Real, useful discovery made along the
  way, not assumed in advance:** no second `App.run()` call was needed
  at all to prove this -- `scroll_by` re-syncs `TerminalState`
  synchronously (`sync_state`'s own real, immediate call), so
  `get_text()` reflects a scroll the instant `Window.scroll` returns,
  no live render loop required. This meant the real scrollback pytest
  coverage could share the file's own single already-existing
  `App.run()` call (used to generate real PTY content) without needing
  a second one at all -- fully compatible with [[feedback_no_second_app_run_in_pytest]]
  by construction, not by careful avoidance.
- Extended `test_terminal.py`'s own sole `App.run()`-based test a third
  time (now proving shell response, Ctrl+C/SIGINT, and scrollback
  together) -- required reordering the typed commands so each real
  claim stays independently checkable against a small, deliberately
  narrow 5-row viewport without earlier assertions getting pushed out
  of view by later ones (a real, iterative fix: the first two drafts
  failed for exactly that reason, caught immediately by running the
  test, not discovered later). Added 2 new synchronous (no `App.run()`
  needed) tests: scrolling an empty terminal doesn't raise, and
  `Window.scroll` on a non-`Terminal` node still bubbles to
  `VirtualList` exactly as before this phase (a real regression guard).
- Extended `examples/terminal.py` with the identical real scrollback
  proof and removed its own now-stale "no scrollback" doc-comment line.
  Also fixed a separately-noticed stale "no scrollback, no Ctrl+C"
  line in `add_terminal`'s own `.pyi` docstring -- missed during Phase
  4's own pass, corrected here while already editing this exact text
  for the real `scrollback_lines` parameter -- and updated `Window.
  scroll`'s own previously-undocumented `.pyi` stub.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-py` +3 unit tests),
  `maturin develop --release`, `pytest tests/` 519 passed/1 skipped (2
  new, up from 517, zero regressions, confirmed no cross-test
  pollution), all 71 examples (including the updated `examples/
  terminal.py`) and the showcase demo re-run clean, `mypy --strict`
  clean against `examples/terminal.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row, Phase 5 heading and Step
  1) -- verified the parser's own reported item count before/after,
  regenerated and republished the Build Tracker artifact.
