# Plan: M17 Phase 1 — Real Clipboard Copy/Cut/Paste (§8)

Corresponds to `BUILD_TRACKER.md` M17 Phase 1's own scoping: a real
clipboard crate wired to Ctrl+C/Ctrl+X/Ctrl+V on a focused `TextField`'s
own real selection.

## Investigation before writing code

- `arboard = "3.6.1"` real and cached (confirmed via `cargo search`) —
  uses `x11rb` for X11, an optional `wl-clipboard-rs` for Wayland
  (confirmed via direct read of its own `Cargo.toml`). `default-
  features = false` used since this phase only needs `get_text`/
  `set_text` (confirmed real via direct source read), not the default
  `image-data` feature.
- **The genuine, stated unknown, resolved by actually testing it, not
  assuming:** a throwaway `#[test]` calling `arboard::Clipboard::new()`
  + `set_text`/`get_text` confirmed a real, live clipboard is
  genuinely reachable in this environment (`$DISPLAY`/`$WAYLAND_
  DISPLAY` both set, a real `/tmp/.X11-unix/X0` socket present) —
  `SET: Ok(())`, `GET: Ok("tre-clipboard-probe")`. Kept as a real,
  permanent regression test afterward (`app.rs::tests::arboard_
  genuinely_round_trips_through_a_real_clipboard`), gracefully
  skipping (not failing) if a future/different environment has no
  reachable clipboard — the same "genuinely different environment"
  tolerance already applied to GPU/display absence.
- **Real, non-obvious finding, changing the whole design:**
  `winit::event::KeyEvent.logical_key` is documented as "affected by
  all modifiers except Ctrl" (confirmed via direct source read) — a
  real Ctrl+C press produces `logical_key: Character("c")`, identical
  to a bare `c` press. `translate_key` doesn't claim `Character(_)` at
  all, so this falls through to the `TextInput` fallback (M15 Phase
  2) — meaning **a real Ctrl+C press on a focused `TextField`, before
  this phase, inserts a literal "c" character**, a genuine pre-
  existing latent bug this phase's own Ctrl-modifier detection also
  fixes, not a separate patch.
- Real architectural split: `engine-core` has zero OS/platform access
  (§4). Paste reuses the *existing* `InputEvent::TextInput` mechanism
  completely (M15 Phase 2) — `engine-py` reads the real clipboard,
  then dispatches the string exactly like typed text. Copy/Cut need a
  real read (Cut: read + mutate) of the focused field's own selection
  — pure `engine-core` work, exposed as direct `Tree` methods
  `engine-py` calls straight from its own `on_input` closure, not
  through `Tree::dispatch`'s `DispatchOutcome` set (no mechanical
  dispatch decision is involved).
- **Real design gap, found while writing the pytest coverage, not
  anticipated:** a real Cut genuinely edits content (mirroring
  Backspace/Delete), but `cut_text_field_selection` is called directly
  on `Tree`, never through `Tree::dispatch` — so it never produces a
  `DispatchOutcome::Changed` the way keyboard editing gets "for free."
  Both `Window.cut()` and the real winit-driven `InputEvent::Cut`
  handling needed an explicit `call_handler(..., EventKind::Change,
  ...)` call added, mirroring `Node.set_checked`/`set_text`'s own
  established "direct mutation, direct Change fire" pattern.
- A real Cut must never destroy the user's own selection if the real
  clipboard write fails — `engine-py`'s own `Cut` handling writes to
  the clipboard *first* (using the pure read, not the mutating
  method), and only actually deletes the selection once that write
  genuinely succeeds.

## Design

- `InputEvent` gains three zero-payload intent variants: `Copy`, `Cut`,
  `PasteRequested` — `Tree::dispatch` treats all three as true no-ops
  (plumbing only, the same shape `ThemeChanged`/`Scroll` already use);
  the real work happens in `engine-py`'s own raw-event match, the same
  way dock-drag/theme-switch handling already does.
- New `Tree::text_field_selected_text(field) -> Option<String>` (pure
  read) and `Tree::cut_text_field_selection(field) -> Option<String>`
  (read + delete, reusing the existing `delete_selection` helper).
- `engine-platform::translate_clipboard_shortcut` detects a real
  Ctrl+C/X/V (case-insensitive), checked in the `WindowEvent::
  KeyboardInput` handler *before* the `TextInput` fallback — fixing
  the latent bug as a real side effect of the correct ordering.
- `engine-py::app.rs`'s own `on_input` closure handles `Copy`/`Cut`/
  `PasteRequested` directly: looks up the focused node, does the real
  `arboard` I/O, fires `Change` on an actual cut. Clipboard failures
  are logged via `tracing::warn!` and non-fatal (M16 Phase 2's own
  established policy).
- `Window` gains `copy()`/`cut()`/`paste(text)` — deliberately
  **hermetic** synthetic entry points (never touch the real OS
  clipboard): unlike `press_key`/`type_text`, which dispatch through
  `Tree::dispatch` exactly like a real `winit` event would, a real
  Ctrl+C only ever originates from an actual OS-level keyboard event
  reaching `engine-platform` directly — there is no synthetic way to
  drive that specific path from Python, a real, stated scope boundary.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests for the two new `Tree` methods; new `engine-
platform` tests for `translate_clipboard_shortcut`; the real,
permanent clipboard round-trip test in `engine-py`; `maturin develop
--release`; new `tests/test_clipboard.py` (hermetic `Window.copy`/
`cut`/`paste` coverage, including that Copy never fires `Change` but
Cut does); new `examples/clipboard.py`; every example re-run; `LOG.md`
/`BUILD_TRACKER.md`/tracker artifact/commit/memory.
