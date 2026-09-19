# PLAN — M30 Phase 9 Step 4: Terminal

## Goal
Add `Window.add_terminal` — a real, live pseudo-terminal. The single
largest step in this catalog: needed two new external Rust crates
(real PTY spawning, real VT/ANSI parsing) and a whole second,
non-text-flow rendering pipeline. Explicitly checked with the user
before starting, given the scale; the user chose "full real terminal"
over a scoped-down frame-sink v1.

## Steps
1. Read pyCopper's own real `Terminal` widget for its real design and
   every real, hard-won finding it already made live (TERM fallback,
   deferred spawn until real layout size known, no PTY mutation off
   the engine thread, a repeat animation to wake an idle event loop).
2. Choose real Rust crates: `portable-pty` (0.9.0, wezterm's own PTY
   crate) for spawning, `vt100` (0.16.2, a small pure-Rust VT100
   parser) for interpreting the byte stream — verified their real
   APIs directly from vendored source before designing around them.
3. Design: `NodeKind::Terminal(TerminalState)` in engine-core holds
   only the already-VT-interpreted cell grid (inert data, the
   TextFieldState precedent); `engine-render` gets a new
   `draw_terminal` painting real background/glyph runs on an analytic
   grid; `engine-py` gets a new `terminal.rs` module owning the real
   PTY session, background reader thread, and VT100 parser.
4. Implement `TerminalSession` (spawn, drain_into, write_input,
   resize) — compiled and unit-verified in isolation before wiring.
5. Wire the per-frame drain into `app.rs`'s own render loop, widening
   `any_active` so a live terminal keeps the event loop ticking (the
   same real fix pyCopper's own `Terminal` already needed for the
   identical "background thread can't otherwise wake an idle loop"
   problem).
6. Wire real keyboard routing: a shared `terminal::input_bytes_for`
   translates `InputEvent` into real terminal bytes, reused by both
   the genuine winit path (`app.rs`) and the synthetic testing path
   (`Window.press_key`/`type_text`, `window_input.rs`).
7. Add `Window.add_terminal` in `window_factory.rs`; extend
   `Node.get_text()` to read a Terminal's own cell grid back as plain
   text (needed for any real test beyond "didn't crash").
8. Full Rust verification chain (check/clippy/fmt/test), rebuild the
   Python extension.
9. **Ran a real, direct empirical end-to-end test before writing any
   pytest suite** (spawn `/bin/sh`, click, type a command, run real
   frames, read back the cell grid) — it genuinely failed: the typed
   command never reached the shell.
10. Root-caused: click-to-focus (M18 Phase 1) was deliberately scoped
    to `TextField` only. Fixed by widening it to include `Terminal`;
    re-ran the same empirical test, which then passed for real.
11. Write `tests/test_terminal.py` and `examples/terminal.py` —
    checked for filename collisions first
    ([[feedback_check_before_new_example_file]]).
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all examples, showcase demo, mypy
    --strict.
13. Update `BUILD_TRACKER.md` — hit a real parser bug while doing so
    (a multi-paragraph writeup broke the generator's own balanced-
    parens requirement; fixed and recorded as a new memory,
    [[feedback_build_tracker_balanced_parens]]), regenerate + republish
    the Build Tracker artifact.
14. Update memory, commit, push.

## Status
Complete. All steps done; full verification chain green (`engine-core`
158 tests unchanged in count but with a real widened click-to-focus
match, 482 pytest passed/1 skipped up from 474, all 66 examples,
showcase demo, 44 Rust test binaries). A real, live shell process
genuinely responds to real typed input, ANSI color escapes included,
confirmed by direct observation of the returned cell-grid text.
