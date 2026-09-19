# PLAN — M33 Phase 1: Terminal Real PTY/Grid Resize

## Goal
Close the real, stated v1 gap M32's own trailer named: "Terminal real
resize-with-window." A live, running terminal's own grid (and the real
kernel-level PTY underneath it) must be resizable.

## Steps
1. Investigated `portable_pty`/`vt100`'s own real resize APIs before
   writing anything: `MasterPty::resize(PtySize)` and `Screen::
   set_size(rows, cols)` are both real and already used by this
   codebase's own dependencies. **Real, surprising discovery:** a
   `TerminalSession::resize` method already existed in `terminal.rs`,
   written at M30 Phase 9 Step 4 and kept `#[allow(dead_code)]` "for
   when that real need arrives" -- never wired to a real caller until
   now.
2. Checked the sibling `pyCopper` project's own real `Terminal` widget
   for precedent and found a real, load-bearing, already-reproduced
   gotcha: resizing a real PTY *after* the shell has already drawn a
   full prompt at the old width can corrupt that shell's own redraw
   for some shells (zsh-syntax-highlighting among them) -- a real,
   inherent PTY/shell-level phenomenon, not a bug to fix, stated
   honestly in the new method's own doc comment.
3. Widened `TerminalSession::resize` to also take `tree`/`node_id` and
   call the existing `sync_state` (the identical real "no new PTY
   bytes involved, this is the only way the change reaches the Tree"
   shape `scroll_by` already established, M32 Phase 5).
4. Added `EngineError::NotATerminal` (mirroring `NotAVirtualList`/
   `NotACanvas`'s own exact shape).
5. Added `Window.resize_terminal(node, cols, rows)`: resizes the real
   PTY + `vt100` screen, then recomputes the node's own real layout
   box from `cols`/`rows` via the exact same cell-metrics formula
   `add_terminal` itself uses at construction, pushed through `Tree::
   set_layout_style` (the real, correct way to mutate a node's style,
   confirmed the hard way by M32 Phase 2's own bug: a direct
   `layout_style` field mutation silently desyncs taffy's own internal
   copy).
6. Real, direct empirical script before pytest: resized a live
   terminal and confirmed via the shell's own real `stty size` output
   that the kernel-level PTY genuinely reported the new size (a real,
   if instructive, first failure using two `App.run()` calls
   reconfirmed the known cross-test hazard applies to standalone
   scripts too, not just pytest -- fixed by using one).
7. Added `tests/test_terminal.py` tests for the synchronous state-sync
   half (no `App.run()` needed, confirmed feasible the same way M32
   Phase 5's `scroll_by` already was) -- 3 new tests. The real kernel-
   PTY-resize claim itself stays verified by the empirical script +
   code review rather than folded into the file's own already-dense
   shared `App.run()` test, a deliberate choice stated in the new
   test's own doc comment.
8. Extended `examples/terminal.py` with the identical real `stty
   size`-based resize proof.
9. Full verification chain: cargo check/clippy/fmt/test, maturin
   develop, pytest (full suite), all 71 examples, showcase demo,
   mypy --strict.
10. Update `BUILD_TRACKER.md`, regenerate + republish the artifact,
    update memory, commit.

## Status
Complete. All steps done; full verification chain green (`pytest
tests/` 526 passed/1 skipped, up from 523, all 71 examples, showcase
demo, mypy --strict clean). A real shell's own `stty size` genuinely
reported a new PTY size after a real resize, not simulated.
