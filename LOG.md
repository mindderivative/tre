# LOG — M30 Phase 9 Step 4: Terminal

- Checked with the user before starting: Terminal needed two new
  external Rust crates and a whole second, non-text-flow rendering
  pipeline, an order of magnitude bigger than Video/Node Graph/Code
  Editor. Presented the real gap (no PTY spawning, no VT/ANSI parser,
  no cell-grid rendering exist in this codebase at all) via
  AskUserQuestion; user chose "full real terminal" over a scoped-down
  frame-sink v1.
- Read pyCopper's own real `Terminal` widget directly
  (`/home/phil/pyDev/projects/pyCopper/src/pycopper/widgets/
  terminal.py`): real PTY spawning via `pexpect`, real VT/ANSI parsing
  via `bittty`, real hard-won findings already made live there --
  `TERM` falling back to "dumb" corrupts real shell plugin redraw
  sequences (fixed by defaulting `TERM`, `env.get(..., default)` so an
  app's own explicit value still wins); no PTY mutation off the engine
  thread (ARCHITECTURE.md's own "the engine thread owns everything
  mutable"); a `repeat=True` animation to guarantee a repaint since a
  background PTY thread has no other way to wake an idle app.
- Chose real Rust crates: `portable-pty` 0.9.0 (wezterm's own real,
  actively-maintained PTY-spawning crate) and `vt100` 0.16.2 (a small,
  widely-used pure-Rust VT100/ANSI parser). Verified their real APIs
  directly from vendored source (`portable_pty::examples::bash.rs`,
  `vt100::Parser`/`Screen`/`Cell`'s own real public methods) before
  designing around them, not assumed from crate names alone.
- Designed the real crate-boundary split: `NodeKind::Terminal
  (TerminalState)` in engine-core holds only the already-VT-
  interpreted cell grid (inert data, the identical real
  `TextFieldState` precedent); `engine-render` gets a new
  `TextRenderer::draw_terminal`; `engine-py` gets a new `terminal.rs`
  module owning the real PTY session.
- Implemented `TerminalState`/`TerminalCell` in `engine-core::node.rs`,
  plus a shared `terminal_cell_size(font_size)` helper (both
  `engine-render` and `engine-py` derive the identical analytic grid
  from it, avoiding any risk of drift between the two).
- Implemented `TextRenderer::draw_terminal` in `engine-render::
  text.rs` -- real background/glyph *runs* (a contiguous span of
  cells sharing one bg, or one fg/bold pair), not one draw call per
  character, the identical real precedent `bittty` already
  established. Real ANSI color: the conventional 16-color palette
  reused directly from pyCopper's own real, already-tuned values
  (converted from their own sRGB floats to real 0-255 u8 triples), plus
  the real, standard xterm 256-color formula (6x6x6 cube, then a
  grayscale ramp) for indices 16-255.
- Implemented `TerminalSession` in a new `engine-py::terminal.rs`
  module: `spawn` opens a real PTY, sets `TERM`/`COLUMNS`/`LINES`,
  spawns the shell, and starts a background thread whose only job is
  appending raw bytes to a lock-guarded `Vec<u8>`; `drain_into` (called
  from the engine thread only) feeds those bytes to a real
  `vt100::Parser` and rebuilds the Tree's own `TerminalState`
  wholesale via `Tree::get_mut` (M29's own real dirty-marking
  chokepoint); `write_input` writes real bytes to the shell's stdin.
- Wired the per-frame drain into `app.rs`'s own render loop, alongside
  `sync_image_textures`/`evict_stale_layouts`. Widened the per-frame
  closure's own `any_active` return to also mean "a real terminal
  session is still alive" -- the identical real fix pyCopper's own
  `Terminal` already needed for the same real problem (a background
  PTY thread producing new output has no other way to wake an
  otherwise-idle `ControlFlow::Wait` event loop, M29 Phase 2).
- Wired real keyboard routing: a new shared `terminal::
  input_bytes_for(event)` translates `InputEvent` into real terminal
  bytes (`\r` for Enter, `\x7f` for Backspace, real standard xterm CSI
  sequences for arrows/Home/End) -- reused by *both* the real winit
  path (`app.rs`'s own `on_input` closure, inspecting the raw event
  directly, the identical "meaning-dependent, not routed through
  DispatchOutcome" precedent `Docking`'s own real wiring already
  established) and the synthetic, no-window-needed testing path
  (`Window.press_key`/`type_text`, `window_input.rs`, via a new shared
  `route_to_terminal` helper).
- Real, deliberately deferred v1 gap, found and stated while designing
  keyboard routing, not silently missed: no Ctrl+C/SIGINT or any other
  Ctrl+letter shortcut -- `InputEvent` carries no real modifier state
  for a plain keypress; `engine_platform::translate_clipboard_
  shortcut`'s own real Ctrl-key detection happens earlier, at the raw
  winit layer, and today only ever produces `Copy`/`Cut`/`Paste`.
- Implemented `Window.add_terminal` in `window_factory.rs`; extended
  `Node.get_text()` with a new `NodeKind::Terminal` arm (rows joined
  by `\n`, each trimmed) -- the load-bearing read-back this step's own
  test suite needed to prove anything beyond "didn't crash."
- Full Rust verification chain green on the first pass after wiring
  everything: `cargo check`/`clippy -D warnings`/`fmt --check`/`cargo
  test --workspace --release` all clean, 44 binaries.
- Rebuilt the Python extension. **Ran a real, direct empirical
  end-to-end test before writing any pytest suite** (spawn `/bin/sh`,
  click, type "echo HELLO_FROM_TERMINAL", press Enter, run real
  frames, read the cell grid back) -- the shell's own real prompt
  ("sh-5.3$") arrived correctly, but the typed command genuinely never
  reached the shell: `is_focused()` returned `False` even after a real
  click.
- Root-caused directly, not guessed: `Tree::dispatch`'s own real
  click-to-focus (M18 Phase 1) was deliberately scoped to `TextField`
  only, confirmed via direct source read of its own real doc comment.
  Fixed by widening the check to `TextField | Terminal`. Re-ran the
  exact same empirical test -- passed for real: `is_focused()` became
  `True`, and the shell's own real response ("HELLO_FROM_TERMINAL")
  appeared in the returned cell-grid text.
- Wrote `tests/test_terminal.py` (8 tests) -- checked for a filename
  collision first (`ls`/`git status`, applying the lesson from M30
  Phase 9 Step 2's own mistake). All passed on the first run,
  including a real end-to-end shell-response proof (not a mock).
- Wrote `examples/terminal.py` -- a real two-command live shell
  session (a plain echo plus a `printf` with real ANSI color escape
  codes), also checked for a filename collision first. Clean on the
  first run: real command echoing, real line-wrapping at the terminal's
  own 48-column width, and real ANSI red/green color codes correctly
  parsed and stripped from the returned text. `mypy --strict` initially
  failed (missing `.pyi` stub, fixed by adding it) then passed clean.
- Full verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean, `cargo test --workspace --release` (44 binaries green),
  `maturin develop --release`, `pytest tests/` (482 passed, 1 skipped,
  up from 474), all 66 examples clean, showcase demo clean, `mypy
  --strict` clean against `examples/terminal.py`.
- Updating `BUILD_TRACKER.md`: **hit a real parser bug** -- a first
  draft's multi-paragraph writeup (blank lines between real findings)
  made `tools/generate_tracker_artifact.py` silently drop the whole
  bullet (caught only by comparing the printed "Parsed N items" count
  before/after, an established discipline). Merged into one unbroken
  line; a second mistake in the same edit swallowed the *next* bullet
  (`Step 5: Carousel`) onto the same line for lack of a newline, and
  the merge itself left the parens unbalanced by one (no final closing
  `)`) -- both caught the same way, both fixed, both re-verified by
  grepping the regenerated HTML for each step's own distinct text.
  Recorded as a new memory, `feedback_build_tracker_balanced_parens`,
  for future large writeups.
- Updated `BUILD_TRACKER.md` (Top Metrics row now 97%, Step 4 line,
  "Just closed"/"Up next" trailer), regenerated and republished the
  Build Tracker artifact at
  https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty.
