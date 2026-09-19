//! M30 Phase 9 Step 4 (§5, §8, §10): `Terminal`'s own real PTY/VT100
//! session -- the identical real "spawn a real pseudo-terminal is
//! OS-specific process management; interpreting its byte stream is
//! the VT/ANSI state machine every real terminal emulator implements
//! identically -- neither is this widget's own concern to reinvent"
//! split the sibling `pyCopper` project's own real `Terminal` widget
//! already established, verified directly rather than assumed to
//! transfer (`Cargo.toml`'s own doc comment on the two new real
//! dependencies this needs, `portable-pty`/`vt100`).
//!
//! Owns exactly the real, mutable OS/parser state `engine-core` must
//! never touch (§4): the spawned shell's own real PTY handles, a
//! background thread draining its output into a shared buffer, and a
//! real `vt100::Parser`. `drain_into` is the one real point of
//! contact with a `Tree` -- it rebuilds a `NodeKind::Terminal`'s own
//! `TerminalState` wholesale from the parser's current `Screen`,
//! mirroring `Video`'s own real "engine only displays the latest
//! snapshot a real external process produced" design (M30 Phase 9
//! Step 1) rather than diffing incrementally.
//!
//! **No PTY mutation happens off the engine thread**, the identical
//! real discipline pyCopper's own `Terminal` already established
//! (ARCHITECTURE.md §8, "the engine thread owns everything mutable"):
//! the background reader thread's only job is appending raw bytes to
//! a lock-guarded `Vec<u8>` -- feeding those bytes to the real VT100
//! parser and writing the result into the `Tree` both happen later,
//! back on the engine thread, inside `drain_into`, called once per
//! real frame tick (`app.rs`'s own per-window closure).

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;

use engine_core::{InputEvent, Key, NodeId, NodeKind, TerminalCell, Tree};
use engine_platform::EventLoopWaker;
use peniko::Color;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Translates a real `InputEvent` into the real bytes a genuine
/// terminal emulator would send its own shell for it -- `None` for
/// anything that isn't real keyboard input at all (pointer/scroll/
/// clipboard/theme events all fall through). `Enter` maps to `\r` (a
/// real terminal convention, not `\n`); `Backspace` to `\x7f` (`DEL`,
/// the real modern-terminal default); arrows/`Home`/`End` to their
/// real standard xterm CSI sequences. Shared by both real real-window
/// keyboard input (`app.rs`'s own `on_input` closure) and the
/// synthetic, no-window-needed testing entry points (`Window.
/// press_key`/`type_text`, `window_input.rs`) -- the identical real
/// "one real translation, two real callers" shape this codebase
/// already uses throughout for input handling.
///
/// **Real, deliberately deferred v1 gap, not silently missed:** no
/// Ctrl+C SIGINT (or any other Ctrl+letter shortcut) -- `engine_core::
/// InputEvent` carries no real modifier-key state for a plain
/// `KeyPressed`/`TextInput` at all (`engine_platform::translate_
/// clipboard_shortcut`'s own real Ctrl+C/X/V detection happens earlier,
/// at the raw `winit` layer, before an `InputEvent` even exists, and
/// today only ever produces `InputEvent::Copy`/`Cut`/`Paste`, never a
/// real terminal-bound byte) -- reaching a focused terminal today
/// requires a real, separate change to that earlier translation layer,
/// out of this step's own scope.
pub(crate) fn input_bytes_for(event: &InputEvent) -> Option<Vec<u8>> {
    match event {
        InputEvent::TextInput(text) => Some(text.as_bytes().to_vec()),
        InputEvent::KeyPressed { key, .. } => Some(
            match key {
                Key::Enter => "\r",
                Key::Space => " ",
                Key::Backspace => "\x7f",
                Key::Delete => "\x1b[3~",
                Key::Tab => "\t",
                Key::Escape => "\x1b",
                Key::ArrowLeft => "\x1b[D",
                Key::ArrowRight => "\x1b[C",
                Key::ArrowUp => "\x1b[A",
                Key::ArrowDown => "\x1b[B",
                Key::Home => "\x1b[H",
                Key::End => "\x1b[F",
            }
            .as_bytes()
            .to_vec(),
        ),
        _ => None,
    }
}

/// A real, live terminal session.
pub(crate) struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    // Kept alive for the session's own real lifetime -- dropping it
    // would end the child process; never read otherwise (this v1 has
    // no real "did the shell exit" surface, a real, stated gap).
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    parser: vt100::Parser,
    /// Shared with the background reader thread below -- real bytes
    /// read from the PTY, appended there, drained (and cleared) here
    /// on the engine thread only.
    incoming: Arc<Mutex<Vec<u8>>>,
    /// M31 Phase 6 (§5, §6): shared with the background reader thread
    /// the identical real way `incoming` already is, so a real waker
    /// registered later (`set_waker`, called from `run_windowed_
    /// multi`'s own `setup` closure -- the one real place able to
    /// reach a fresh `EventLoopWaker`, `engine-platform`'s own doc
    /// comment) is still visible to a thread that was already running
    /// before it existed (a `TerminalSession` is always spawned by a
    /// real `add_terminal` call, which always happens before `App.
    /// run()` -- and therefore before any real `EventLoopWaker` exists
    /// -- in every real caller this codebase has). `None` until `set_
    /// waker` runs -- the real, honest "no live loop to wake yet"
    /// state a synthetic, no-`App.run()`-needed test correctly stays
    /// in forever.
    waker: Arc<Mutex<Option<EventLoopWaker>>>,
}

impl TerminalSession {
    /// Opens a real PTY at `cols`x`rows` and spawns `shell` on it.
    /// Real, honest v1 scope: POSIX only, the identical real
    /// "architected for, not built, since nothing here could verify
    /// it rather than guess" choice pyCopper's own real `Terminal`
    /// already made for Windows/ConPTY.
    pub(crate) fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        let mut cmd = CommandBuilder::new(shell);
        // Real finding, directly reused from pyCopper's own real
        // `Terminal` widget (found live there, 2026-09-08, its own
        // module doc comment): a shell spawned with no real `TERM`
        // falls back to its own internal "dumb" default, and several
        // real shell plugins (zsh-syntax-highlighting among them)
        // genuinely corrupt their own per-keystroke redraw sequences
        // under "dumb" -- reproduced there directly, not assumed.
        // `env.get(..., default)`, not an unconditional overwrite, so
        // an application's own explicit `TERM` still wins.
        if std::env::var_os("TERM").is_none() {
            cmd.env("TERM", "xterm-256color");
        }
        cmd.env("COLUMNS", cols.to_string());
        cmd.env("LINES", rows.to_string());

        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        // The slave end belongs to the child process now -- dropping
        // this side's own handle is the real, documented way to stop
        // holding it open here (`portable_pty::examples::bash.rs`'s
        // own real precedent, confirmed via direct source read).
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let incoming = Arc::new(Mutex::new(Vec::new()));
        let incoming_for_thread = Arc::clone(&incoming);
        let waker: Arc<Mutex<Option<EventLoopWaker>>> = Arc::new(Mutex::new(None));
        let waker_for_thread = Arc::clone(&waker);
        thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut guard = incoming_for_thread
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        guard.extend_from_slice(&buf[..n]);
                        drop(guard);
                        // M31 Phase 6 (§5, §6): a real new PTY byte
                        // arriving is exactly the real event this whole
                        // phase exists to wake an idle event loop for
                        // -- a no-op (`None`) until a real `App.run()`
                        // has registered one via `set_waker`.
                        if let Some(waker) = waker_for_thread
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .as_ref()
                        {
                            waker.wake();
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            _child: child,
            parser: vt100::Parser::new(rows, cols, 0),
            incoming,
            waker,
        })
    }

    /// Drains whatever real bytes the background reader thread has
    /// appended since the last call, feeds them into the real VT100
    /// parser, and -- only if any real bytes actually arrived --
    /// rebuilds `node_id`'s own `TerminalState` wholesale from the
    /// parser's own current `Screen`, via `Tree::get_mut` (the same
    /// real dirty-marking chokepoint every other content mutator in
    /// this crate already goes through, M29's own dirty-tracking).
    /// Returns `true` exactly when it did -- the caller's own real
    /// "does this frame need to repaint" signal.
    pub(crate) fn drain_into(&mut self, tree: &mut Tree, node_id: NodeId) -> bool {
        let bytes = {
            let mut guard = self
                .incoming
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.is_empty() {
                return false;
            }
            std::mem::take(&mut *guard)
        };
        self.parser.process(&bytes);
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let mut cells = Vec::with_capacity(usize::from(rows) * usize::from(cols));
        for row in 0..rows {
            for col in 0..cols {
                cells.push(screen_cell_to_terminal_cell(screen.cell(row, col)));
            }
        }
        let (cursor_row, cursor_col) = screen.cursor_position();
        let cursor_visible = !screen.hide_cursor();

        if let Some(node) = tree.get_mut(node_id)
            && let NodeKind::Terminal(state) = &mut node.kind
        {
            state.cols = cols;
            state.rows = rows;
            state.cells = cells;
            state.cursor_col = cursor_col;
            state.cursor_row = cursor_row;
            state.cursor_visible = cursor_visible;
        }
        true
    }

    /// M31 Phase 6 (§5, §6): registers the real handle this session's
    /// own background reader thread uses to wake an idle event loop
    /// the moment real new PTY bytes arrive, closing the real, stated
    /// v1 cost M30 Phase 9 Step 4 left open (continuously widening
    /// `any_active` instead). Called once per real session from `App.
    /// run()`'s own `setup` closure -- the one real place able to
    /// reach a fresh `EventLoopWaker` at all.
    pub(crate) fn set_waker(&self, waker: EventLoopWaker) {
        *self
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(waker);
    }

    /// Writes real bytes to the shell's own stdin -- keystrokes
    /// translated by `input_bytes_for_key`/`input_bytes_for_text`
    /// below, or a real Ctrl+C SIGINT byte.
    pub(crate) fn write_input(&mut self, bytes: &[u8]) {
        // A closed/dead PTY write failing is a real, unremarkable
        // "the shell already exited" condition, not a bug to surface
        // -- `Window.add_video`'s own `push_frame` has no error path
        // for "the app kept sending frames after tearing something
        // down" either; consistent, not a new precedent.
        let _ = self.writer.write_all(bytes);
    }

    /// Resizes both the real kernel-level PTY (so the shell's own
    /// `SIGWINCH`-driven reflow, e.g. a wrapped `$PS1`, sees the real
    /// new size) and the VT100 parser's own screen buffer. **Real,
    /// stated v1 gap, not wired up yet:** nothing in this codebase
    /// resizes any node's own box when its window resizes today (a
    /// real, separate, un-scoped capability no other component in
    /// this catalog has either) -- kept as a real, available method
    /// for when that real need arrives, not dead speculative API.
    #[allow(dead_code)]
    pub(crate) fn resize(&mut self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.parser.screen_mut().set_size(rows, cols);
    }
}

fn screen_cell_to_terminal_cell(cell: Option<&vt100::Cell>) -> TerminalCell {
    let Some(cell) = cell else {
        return TerminalCell::blank();
    };
    // Real, stated v1 simplification: takes only the first real `char`
    // of a cell's own contents -- a real wide-character's own
    // continuation cell (`Cell::is_wide_continuation`) and real
    // combining-character sequences both collapse to this one glyph,
    // not rendered precisely. `bittty`-backed real terminals handle
    // both; this v1 doesn't yet, a real, separate gap from the four
    // this milestone's own `BUILD_TRACKER.md` entry already names.
    let ch = cell.contents().chars().next().unwrap_or(' ');
    TerminalCell {
        ch,
        fg: vt100_color_to_peniko(cell.fgcolor(), DEFAULT_FG),
        bg: vt100_color_to_peniko(cell.bgcolor(), Color::TRANSPARENT),
        bold: cell.bold(),
    }
}

/// The real base ink color a cell's own `vt100::Color::Default`
/// foreground resolves to -- the identical real default `TextField`'s
/// own `text_tint` already uses before a theme is set (`TextFieldState
/// ::new`'s own hardcoded `0x1C1B1F`), reused directly rather than a
/// second, differently-chosen "default text color."
const DEFAULT_FG: Color = Color::from_rgba8(0x1C, 0x1B, 0x1F, 0xFF);

fn vt100_color_to_peniko(color: vt100::Color, default: Color) -> Color {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color::from_rgba8(r, g, b, 0xFF),
        vt100::Color::Idx(index) => ansi_index_to_color(index),
    }
}

/// The conventional 16-color ANSI palette (indices 0-15), reused
/// directly from the sibling `pyCopper` project's own real, already-
/// tuned `_ANSI_COLORS` table (its own real sRGB float values,
/// converted to the 0-255 `u8` triples `peniko::Color::from_rgba8`
/// expects) -- not M3/MD3-sourced, the identical real "no semantic
/// role exists to map any of this onto" reasoning `Code Editor`'s own
/// syntax-highlighting scoping note already gives for a comparable
/// literal-color need. Indices 16-255 use the real, standard xterm
/// 256-color formula (a 6x6x6 color cube, then a grayscale ramp) --
/// deterministic and well-known, not invented here.
const ANSI_16: [(u8, u8, u8); 16] = [
    (28, 28, 33),
    (222, 89, 89),
    (140, 191, 102),
    (217, 178, 89),
    (102, 153, 230),
    (191, 128, 217),
    (102, 191, 204),
    (204, 204, 209),
    (102, 107, 117),
    (242, 115, 115),
    (166, 217, 128),
    (242, 204, 115),
    (140, 178, 242),
    (217, 153, 242),
    (140, 217, 230),
    (242, 242, 247),
];

fn ansi_index_to_color(index: u8) -> Color {
    if let Some(&(r, g, b)) = ANSI_16.get(usize::from(index)) {
        return Color::from_rgba8(r, g, b, 0xFF);
    }
    if index < 232 {
        // The real xterm 6x6x6 color cube: index 16 is (0,0,0), each
        // of the three channels steps through the same real six-value
        // ramp (0, 95, 135, 175, 215, 255) -- the well-known standard
        // formula, not invented here.
        let n = index - 16;
        let levels = [0u8, 95, 135, 175, 215, 255];
        let r = levels[usize::from(n / 36)];
        let g = levels[usize::from((n / 6) % 6)];
        let b = levels[usize::from(n % 6)];
        return Color::from_rgba8(r, g, b, 0xFF);
    }
    // 232-255: the real xterm grayscale ramp, 24 steps from near-black
    // to near-white.
    let level = 8 + (index - 232) as u16 * 10;
    let level = level.min(255) as u8;
    Color::from_rgba8(level, level, level, 0xFF)
}
