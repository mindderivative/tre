# LOG — M32 Phase 4: Terminal Ctrl+C / SIGINT and Ctrl-Letter Shortcuts

- Confirmed the exact real gap via direct read of `terminal.rs`'s own
  `input_bytes_for` doc comment (written at M30 Phase 9 Step 4, stating
  the gap honestly rather than silently dropping it): `engine_
  platform::translate_clipboard_shortcut`'s own real Ctrl+C/X/V
  detection happens at the raw winit layer, before an `InputEvent`
  even exists, today only ever producing `Copy`/`Cut`/`PasteRequested`
  -- never a real terminal-bound byte.
- Added `InputEvent::ControlChar(char)` (`engine-core::input.rs`) for
  every real Ctrl+`<letter>` press besides c/x/v (unchanged, still
  `Copy`/`Cut`/`PasteRequested`). Plumbing only in `Tree::dispatch`,
  the identical shape `Copy`/`Cut`/`PasteRequested` already established
  (`engine-core` has zero PTY access, §4).
- Widened `translate_clipboard_shortcut` (`engine-platform`) to the
  full ASCII alphabet -- restructured to extract exactly one char from
  the `Character` payload (guarding against a real, if rare, multi-
  char IME/dead-key sequence, which now correctly still produces
  `None` rather than silently picking the first char). c/x/v keep
  their own exact real behavior; every other letter now produces
  `ControlChar` instead of `None`. Updated the one pre-existing test
  that asserted `Character("a") -> None` (now `Some(ControlChar('a'))`
  -- the real fix this phase exists for) and added 2 new tests.
- Added `terminal::control_byte_for` (`engine-py`): the real Ctrl+
  `<letter>` -> ASCII control-code mapping (`letter - 'A' + 1`, the
  identical real formula every terminal emulator uses -- Ctrl+A=0x01
  through Ctrl+Z=0x1A, Ctrl+C=0x03=`ETX`/SIGINT). **Real, deliberate
  design decision, not an accident:** mapped `Copy`/`Cut`/
  `PasteRequested` to their own real underlying letters here too, so
  when a `Terminal` is genuinely focused, Ctrl+C/X/V mean their own
  real terminal-control bytes, not clipboard ops -- matching every
  real terminal emulator's own actual behavior (none of them treat a
  bare Ctrl+C as "copy"). When no terminal is focused, this function
  is simply never reached for those three (the call sites below only
  invoke it after confirming a real `Terminal` is focused), so
  ordinary `TextField` copy/cut/paste stays completely unaffected --
  zero regression risk for the non-terminal case.
- Wired `control_byte_for` into `app.rs`'s `on_input` closure --
  extended the existing real terminal-keyboard-routing block
  (`input_bytes_for`'s own early-return check) with a parallel one for
  the new function, sharing one `focused_terminal` lookup between both
  (`NodeId: Copy`, confirmed via a successful compile with no move
  errors). Added the identical real routing to the synthetic, no-live-
  window testing path (`window_input.rs`): a new `route_control_char_
  to_terminal` helper mirroring `route_to_terminal`'s own shape, and a
  new `Window.press_ctrl(letter: str) -> bool` pymethod -- deliberately
  narrower in scope than `press_key`/`type_text` (never falls through
  to ordinary `Tree::dispatch`; when no terminal is focused it just
  returns `False`, touching nothing else, since `copy`/`cut`/`paste`
  already own the separate, hermetic `TextField`-clipboard surface).
- Real Rust unit tests: `engine-py::terminal.rs` gained its first-ever
  `#[cfg(test)] mod tests` (this file had none before), 4 new tests for
  `control_byte_for` (the real SIGINT byte value, the full a-z range,
  Cut/Paste's own real letters, and a real negative case for non-
  control-char events). `engine-platform` gained 2 new tests for the
  widened `translate_clipboard_shortcut`. All passed on the first run
  -- no bugs found this phase.
- Full Rust verification chain green: `cargo check`/`clippy -D
  warnings`/`fmt --check` clean (one real clippy fix needed: a
  collapsible-if in `app.rs`, folded into a single `if let ... &&`
  chain matching this file's own established style elsewhere).
- Rebuilt the Python extension. Ran a real, direct empirical script
  before writing any pytest -- the definitive real proof this whole
  phase exists for: spawned a real shell, ran a genuine `sleep 100`,
  waited 0.2s real wall-clock time for the shell to actually fork/exec
  it, called `press_ctrl("c")`, then queued a distinguishable follow-up
  command. The terminal's own final text showed the real `^C` echo and
  the follow-up command's own real output -- proof `sleep 100` was
  genuinely killed, not merely that the call didn't raise. Passed on
  the first run.
- **Respected the established "only one real `App.run()` call across
  the whole pytest process" rule**
  ([[feedback_no_second_app_run_in_pytest]]): rather than adding a new
  test function with its own `App.run()` call, extended `test_
  terminal.py`'s own existing real-shell test to also queue the sleep/
  Ctrl+C/echo-after sequence before its one shared `App.run()` call.
  Added 3 new synchronous (no `App.run()` needed) tests for `press_
  ctrl`'s own return-value contract (`False` with nothing focused,
  `True` with a real terminal focused, `ValueError` for anything that
  isn't exactly one ASCII letter). Ran `pytest tests/` for the whole
  suite (not just this file) to confirm zero cross-test pollution, the
  same concrete check that memory's own "how to apply" section
  recommends.
- Extended `examples/terminal.py` with the identical real sleep/
  Ctrl+C/echo-after proof (its own separate process when run standalone
  -- no pytest cross-test concern there) and removed its own now-stale
  "no Ctrl+C/SIGINT" line from the module doc comment.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --release` (`engine-py` +4 unit tests -- its first
  ever, `engine-platform` +2), `maturin develop --release`, `pytest
  tests/` 517 passed/1 skipped (3 new, up from 514, zero regressions,
  confirmed no cross-test pollution from the extended real-shell test),
  all 71 examples (including the updated `examples/terminal.py`) and
  the showcase demo re-run clean, `mypy --strict` clean against
  `examples/terminal.py`.
- Updated `BUILD_TRACKER.md` (Top Metrics row, Phase 4 heading and Step
  1) -- verified the parser's own reported item count before/after,
  regenerated and republished the Build Tracker artifact.
