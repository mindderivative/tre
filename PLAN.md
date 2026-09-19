# PLAN — M32 Phase 4: Terminal Ctrl+C / SIGINT and Ctrl-Letter Shortcuts

## Goal
Close the real, stated v1 gap M30 Phase 9 Step 4 (Terminal) named
directly: no Ctrl+C/SIGINT or any other Ctrl+letter shortcut reaches a
focused terminal. Ctrl+C must send a real SIGINT, killing a running
process, matching every real terminal emulator's own behavior.

## Steps
1. Confirmed the exact real gap via direct read of `terminal.rs`'s own
   `input_bytes_for` doc comment: `engine_platform::translate_
   clipboard_shortcut`'s own real Ctrl+C/X/V detection happens at the
   raw winit layer, today only ever producing `Copy`/`Cut`/
   `PasteRequested` -- never a real terminal-bound byte.
2. Added `InputEvent::ControlChar(char)` (`engine-core`) -- a real
   Ctrl+`<letter>` press for any letter besides c/x/v (which keep their
   own real `Copy`/`Cut`/`PasteRequested` meaning, unchanged). Plumbing
   only in `Tree::dispatch` (`engine-core` has no PTY access, §4).
3. Widened `translate_clipboard_shortcut` (`engine-platform`) to the
   full ASCII alphabet: c/x/v unchanged, every other letter now
   produces `ControlChar` instead of `None`.
4. Added `terminal::control_byte_for` (`engine-py`): the real Ctrl+
   `<letter>` -> ASCII control-code mapping (Ctrl+A=0x01 .. Ctrl+Z=
   0x1A, Ctrl+C=0x03=SIGINT). **Real, deliberate v1 choice:** `Copy`/
   `Cut`/`PasteRequested` map to their own real letters here too -- a
   genuinely focused `Terminal` treats Ctrl+C/X/V as their own real
   terminal-control bytes, not clipboard ops, matching every real
   terminal emulator (a bare Ctrl+C has never meant "copy" in any of
   them); when no terminal is focused, this function is simply never
   called for those three, so ordinary `TextField` copy/cut/paste is
   completely unaffected.
5. Wired `control_byte_for` into `app.rs`'s `on_input` closure
   (extending the existing real terminal-keyboard-routing block) and
   into `window_input.rs`'s synthetic path via a new `Window.
   press_ctrl(letter) -> bool` method and `route_control_char_to_
   terminal` helper -- the identical real "one translation, two real
   callers" shape every other input capability here already has.
6. Real Rust unit tests (`engine-py::terminal.rs`, a fresh `#[cfg(test)]
   mod tests` -- this file had none before): SIGINT byte value, full
   a-z range, Copy/Cut/Paste's own real letters, and a real negative
   case. Plus 2 new `engine-platform` tests for the widened
   `translate_clipboard_shortcut`.
7. Real, direct empirical script before pytest: a genuine `sleep 100`,
   interrupted by a real `press_ctrl("c")`, proven by a later command
   actually executing (would never run if `sleep` were still blocking
   the shell) -- passed on the first run.
8. **Respected the established "only one real `App.run()` call across
   the whole pytest process" rule** ([[feedback_no_second_app_run_in_pytest]]):
   extended `test_terminal.py`'s own existing `App.run()`-based test
   rather than adding a new one, plus 3 new synchronous (no-`App.run()`)
   tests for `press_ctrl`'s own return-value/error contract.
9. Extended `examples/terminal.py` with the identical real sleep/
   Ctrl+C/echo-after proof (its own separate process, no pytest
   cross-test concern).
10. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite, checked for cross-test pollution),
    all 71 examples, showcase demo, mypy --strict.
11. Update `BUILD_TRACKER.md`, regenerate + republish the artifact,
    update memory, commit.

## Status
Complete. All steps done; full verification chain green (`engine-py`
gains 4 new unit tests -- this crate had none before this phase --
`engine-platform` +2, `pytest tests/` 517 passed/1 skipped, up from
514, all 71 examples, showcase demo, mypy --strict clean). A real
`sleep 100` was genuinely killed by a real Ctrl+C in both the
empirical check and the example, not simulated.
