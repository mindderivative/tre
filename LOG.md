# Log: M15 Phase 2 — Real Keyboard-Driven Editing (§8, §10)

Corresponds to `BUILD_TRACKER.md` M15 Phase 2. `winit::event::KeyEvent
.text` reaches a real `InputEvent` for the first time, `Tree::dispatch`
inserts characters into a focused `TextField` and handles real
Backspace/Delete/Left/Right/Home/End, and `EventKind::Change` fires on
every real edit.

## Investigation before writing code

`Key` had exactly four variants, confirmed by direct read — no
character-producing key at all; `winit`'s own real `KeyEvent.text:
Option<SmolStr>` was completely discarded. **Real design conflict,
found while designing:** `NamedKey::Space` is matched by `translate_
key` before any `TextInput` fallback would ever see it, but `Key::
Space` already means "activate a focused node" — a focused `TextField`
needs `Space` to insert a literal space instead. Resolved by having
`Tree::dispatch` check the focused node's own kind first, before the
generic Tab/Enter/Space/Escape handling. `InputEvent` derived `Copy`;
adding `TextInput(String)` needed dropping it (kept `Clone`) — the one
real call site that needed a `.clone()` (`app.rs`'s `on_input`
closure, which uses `event` twice) was found by attempting the build,
not by guessing every call site in advance.

## What happened

`Key` gains `Backspace`/`Delete`/`ArrowLeft`/`ArrowRight`/`Home`/`End`
(all real `NamedKey` variants, confirmed via direct source read before
adding). `InputEvent` gains `TextInput(String)`.

New private `Tree::dispatch_text_field_key(field, key) ->
Option<DispatchOutcome>`: `None` for `Tab`/`Escape` (falls through to
the generic handling — a focused field must still lose focus on Tab
and still dismiss overlays on Escape); `Some(Changed(field))` for any
real content edit; `Some(None)` for pure cursor movement or a genuine
no-op (`Backspace` at `cursor == 0`, `Delete` at the real end) —
`Change` only ever means "the bound value genuinely changed." `Enter`
is consumed without inserting a newline (real, stated single-line-
field scope) or activating. `Tree::dispatch`'s `KeyPressed` arm checks
the focused node first; a new `TextInput` arm inserts at `cursor` and
advances it. Every mutation stays on real `char_indices` boundaries.

`engine-platform::translate_key` widened; the real `WindowEvent::
KeyboardInput` handler now falls back to firing `TextInput` when
`translate_key` returns `None` and `key_event.text` is real, press
only. New `Node.set_text` mirrors `set_checked`'s own shape exactly,
including always firing `Change` — protected against a two-way-
binding feedback loop by the exact `Signal`-level fix M14 Phase 3
already made, no new fix needed. `Window.press_key` widened to the six
new named keys; new `Window.type_text(text)` mirrors `press_key`'s own
synthetic-dispatch pattern for `TextInput`.

New `engine-core` tests (12): real insertion at an arbitrary cursor
position; Backspace/Delete both as a real edit and as a genuine no-op
(neither reports `Changed`); pure cursor movement never reports
`Changed`; Space inserts rather than activates; Enter is consumed
without inserting or activating; Tab still moves focus away from a
focused field; a real multi-byte UTF-8 character (`"café"`'s own "é")
removed whole, not corrupted — all 12 passed on the first run.
`engine-platform`'s own vocabulary test widened for the six new keys,
passed unmodified otherwise. New `tests/test_text_field.py` additions
(9 tests): `type_text` inserts into the focused field and is a safe
no-op with none focused; Backspace/Delete/arrow/Home/End all reach the
real field via `press_key`; `set_text` overwrites content and fires a
real `on_change`; `set_text` rejects a non-`TextField` node; typing
fires `on_change` with the real current text each time; pure cursor
navigation and genuine no-op edits do *not* fire `on_change` — all 16
tests in the file (7 existing + 9 new) passed on the first run. Updated
`examples/text_field.py`: real typing past the end, `Home` + insert at
the start, `End` + `Backspace`, each step asserted against the exact
real resulting content.

Full `cargo test --workspace --release` (`engine-core` 108, up from
96; `engine-platform` unchanged at 6, widened coverage)/`cargo clippy
--workspace --all-targets -- -D warnings`/`cargo fmt --check` all
clean — every prior test passed unmodified. `maturin develop --release`
+ full `pytest tests/` (147 passed, up from 138, 1 skipped) and all
twenty-three examples confirmed clean.
