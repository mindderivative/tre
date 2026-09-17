# Plan: M15 Phase 2 — Real Keyboard-Driven Editing (§8, §10)

Corresponds to `BUILD_TRACKER.md` M15 Phase 2's own scoping:
`winit::event::KeyEvent.text` reaches a real `InputEvent` for the
first time, `Tree::dispatch` inserts characters into a focused
`TextField` and handles real Backspace/Delete/Left/Right/Home/End
(plain UTF-8 char-boundary logic inside `engine-core`), and
`EventKind::Change` fires on every real edit.

## Investigation before writing code

- `Key` (`engine-core::input.rs`) had exactly four variants (`Tab`/
  `Enter`/`Space`/`Escape`), confirmed by direct read — no character-
  producing key, no Backspace/Delete/arrow keys at all.
  `engine-platform::translate_key` only matched those four against
  `winit::keyboard::Key::Named`; `key_event.text: Option<SmolStr>`
  (winit's own real per-keypress produced text, confirmed via direct
  source read of the pinned `winit = "0.30.13"`) was completely
  discarded.
- **Real design conflict, found while designing, not after:**
  `NamedKey::Space` is matched by `translate_key` *before* any
  `TextInput` fallback would ever see it, so a literal space keypress
  can never reach `TextInput` — but `Key::Space` already has a real,
  established meaning (`Enter | Space => Activated`, button
  activation). A focused `TextField` genuinely needs `Space` to insert
  a space character instead. Resolved by having `Tree::dispatch`
  check whether the *focused* node is a `TextField` first, before the
  generic Tab/Enter/Space/Escape handling — the same real, deliberate
  "meaning depends on what's focused" case a keyboard model has to
  handle, not an edge case to gloss over.
- `InputEvent` currently derives `Copy` — confirmed via direct read,
  every real caller takes it by value, so adding a `TextInput(String)`
  variant (needed to carry real produced text, `String` isn't `Copy`)
  is a safe, additive change to the derive list, not a breaking one to
  any call site's own shape.
- `crates/engine-py/src/app.rs`'s own `on_input` closure uses `event`
  twice (once consumed by `dispatch`, once matched again for the real
  dock-drag/theme-switch handling) — the one real call site that
  actually needed a `.clone()` once `Copy` is gone, confirmed by
  attempting the build first rather than guessing every call site in
  advance.

## Design

- `Key` gains `Backspace`/`Delete`/`ArrowLeft`/`ArrowRight`/`Home`/
  `End` (all real `NamedKey` variants, confirmed via direct source
  read of `winit`'s own `keyboard.rs` before adding them).
- `InputEvent` gains `TextInput(String)`, `derive`s `Clone` only (not
  `Copy` anymore).
- New private `Tree::dispatch_text_field_key(field, key) ->
  Option<DispatchOutcome>`: `None` for `Tab`/`Escape` (fall through to
  generic handling); `Some(Changed(field))` for any real content edit
  (character insert, Backspace/Delete that actually removed
  something, a literal Space); `Some(None)` (the outcome) for pure
  cursor movement (`ArrowLeft`/`ArrowRight`/`Home`/`End`) or a genuine
  no-op (`Backspace` at `cursor == 0`, `Delete` at `cursor ==
  content.len()`) — `Change` only ever means "the bound value
  genuinely changed," not "a key was pressed." `Enter` is consumed
  (no activation, no newline — a real, stated single-line-field
  scope).
- `Tree::dispatch`'s `KeyPressed` arm checks the focused node first;
  a new `TextInput` arm inserts at `cursor`, advances it by the
  inserted byte length, clears any selection, and reports `Changed`.
  Both mutate `content`/`cursor` only ever at real `char_indices`
  boundaries.
- `engine-platform::translate_key` widened; the `WindowEvent::
  KeyboardInput` handler falls back to firing `TextInput` when
  `translate_key` returns `None` and `key_event.text` is real, on
  press only.
- `Node.set_text(content)` (`engine-py`) mirrors `set_checked`'s own
  shape exactly, including always firing `Change` — the same
  established convention, protected against a two-way-binding feedback
  loop by the exact `Signal`-level fix M14 Phase 3 already made, no
  new fix needed here.
- `Window.press_key` widened to the six new named keys; new `Window.
  type_text(text)` mirrors `press_key`'s own synthetic-dispatch
  pattern for `TextInput`.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests (insertion, Backspace/Delete both real-edit
and genuine-no-op cases, cursor movement never reports `Changed`,
Space inserts rather than activates, Enter is consumed without
inserting, Tab still moves focus away, a real multi-byte UTF-8
character removed whole); `engine-platform` test widened for the new
vocabulary; `maturin develop --release`; new `tests/test_text_field.py`
coverage for `type_text`/widened `press_key`/`set_text`, including
that pure navigation and genuine no-op edits do *not* fire
`on_change`; updated `examples/text_field.py` demonstrating real
typing/navigation/deletion; every example re-run; `LOG.md`/
`BUILD_TRACKER.md`/tracker artifact/commit/memory.
