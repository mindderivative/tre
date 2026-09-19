# PLAN — M32 Phase 6: Terminal Mouse Text Selection

## Goal
Close the real, stated v1 gap M30 Phase 9 Step 4 (Terminal) named: no
mouse text selection or copy. This closes M32 itself, all 6 phases.

## Steps
1. Checked the sibling `pyCopper` project's own real `Terminal` widget
   (the reference every prior Terminal phase this session grounded
   itself in) and confirmed it explicitly excludes mouse selection too
   -- "deliberately out of scope for this pass... since there is
   nothing to copy without a selection." No real reference
   implementation existed anywhere to design this from, the identical
   real situation M31 Phase 5 (Code Folding) was in. Paused and asked
   the user directly via `AskUserQuestion`; the user chose "Full real
   selection + clipboard copy."
2. Added `TerminalState.selection_start`/`selection_end: Option<(u16,
   u16)>` (real `(row, col)` cell coordinates, `engine-core`).
3. New `Tree` methods: `set_terminal_selection_start`/`extend_
   terminal_selection` (mutation, the identical real "collapse on
   press, grow on drag" shape `set_text_field_cursor`/`extend_text_
   field_selection` already established) and `terminal_selected_text`
   (a pure read -- real *linear*, reading-order selection, each row's
   own trailing whitespace trimmed, matching `Node.get_text()`'s own
   established convention).
4. Added `engine-render::TextRenderer::terminal_hit_cell`: turns a
   real local point into its real `(row, col)` cell via the identical
   `monospace_cell_size` metrics `draw_terminal` already positions
   every cell on -- much simpler than `TextField`'s own per-glyph
   `hit_test_position` (a uniform grid needs only plain division).
   `draw_terminal` paints a real selection highlight (`with_opacity(
   at.color, 0.3)`, the identical real convention `TextField`'s own
   selection painting already uses).
5. Wired real mouse-drag selection into `app.rs`'s `on_input` closure
   -- a new `runtime.terminal_drag: Option<NodeId>` field mirrors
   `text_drag`'s own exact shape for `PointerPressed`/`PointerMoved`/
   `PointerReleased`.
6. Added the real Ctrl+Shift+C copy shortcut: widened `translate_
   clipboard_shortcut` to take an explicit `shift: bool` (a real
   correctness fix -- inferring Shift from `Character` case would have
   conflated it with Caps Lock) and produce the new `InputEvent::
   TerminalCopyRequested` for Ctrl+Shift+C specifically. **Real,
   deliberate design:** a bare Ctrl+C on a focused terminal still means
   SIGINT (M32 Phase 4, unchanged); Ctrl+Shift+C is the separate real
   shortcut that copies, matching every real terminal emulator's own
   actual convention.
7. Real Rust unit tests: 10 for the selection model (`engine-core`),
   2 pixel-diff tests proving the selection highlight genuinely paints
   (`engine-render`), 2 for `terminal_hit_cell`'s own geometry, 2 new
   for the widened `translate_clipboard_shortcut`.
8. Added `Node.set_terminal_selection`/`Window.copy_terminal_selection`
   -- the real, hermetic, no-live-window-needed synthetic entry points,
   mirroring `Window.copy()`'s own established scope boundary (real
   mouse drag and real Ctrl+Shift+C stay winit-only, the identical real
   limitation `Window.copy()`'s own doc comment already states for a
   plain Ctrl+C).
9. Real, direct empirical script before pytest: seeded a real
   selection over genuinely echoed shell output and read it back.
10. Extended `test_terminal.py`'s own sole `App.run()`-based test a
    fourth time (now proving shell response, Ctrl+C/SIGINT,
    scrollback, and selection together) plus 4 new synchronous tests.
11. Extended `examples/terminal.py` with the identical real selection
    proof.
12. Full verification chain: cargo check/clippy/fmt/test, maturin
    develop, pytest (full suite), all 71 examples, showcase demo,
    mypy --strict.
13. Update `BUILD_TRACKER.md` (closing Phase 6 and M32 itself),
    regenerate + republish the artifact, update memory, commit, push
    (a full milestone closing).

## Status
Complete. All steps done; full verification chain green (`engine-core`
+10 tests, `engine-render` +4, `engine-platform` +2, `pytest tests/`
523 passed/1 skipped, up from 519, all 71 examples, showcase demo,
mypy --strict clean). **M32 -- Hardening: Closing Stated v1 Gaps is
now fully complete, all 6 phases.**
