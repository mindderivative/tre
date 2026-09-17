# Log: M17 Phase 1 — Real Clipboard Copy/Cut/Paste (§8)

Corresponds to `BUILD_TRACKER.md` M17 Phase 1. A real clipboard crate
wired to Ctrl+C/Ctrl+X/Ctrl+V on a focused `TextField`'s own real
selection.

## Investigation before writing code

`arboard = "3.6.1"` real and cached, `default-features = false`
(only `get_text`/`set_text` needed). **The genuine, stated unknown,
resolved by actually testing it:** a throwaway probe confirmed a real,
live clipboard is genuinely reachable in this environment — kept
afterward as a real, permanent regression test, gracefully skipping
(not failing) in an environment with no reachable clipboard. **Real,
non-obvious finding, changing the whole design:** `winit`'s own
`logical_key` is "affected by all modifiers except Ctrl" — a real
Ctrl+C press produces `Character("c")`, identical to a bare `c`,
meaning **before this phase, a real Ctrl+C press on a focused
`TextField` inserted a literal "c" character** — a genuine latent bug
this phase's own Ctrl-modifier detection fixes as a real side effect,
not a separate patch. Real architectural split: Paste reuses the
existing `InputEvent::TextInput` mechanism completely; Copy/Cut are
pure `engine-core` `Tree` methods `engine-py` calls directly, not
through `Tree::dispatch`.

## What happened

`InputEvent` gains `Copy`/`Cut`/`PasteRequested` (zero-payload intent
signals, true no-ops in `Tree::dispatch`, the same "plumbing only"
shape `ThemeChanged` already uses). New `Tree::text_field_selected_
text`/`cut_text_field_selection` (pure read; read + delete, reusing
`delete_selection`). New `engine-platform::translate_clipboard_
shortcut`, checked before the `TextInput` fallback — fixing the latent
bug. `engine-py::app.rs`'s `on_input` closure does the real `arboard`
I/O for all three, with clipboard failures logged via `tracing::warn!`
and non-fatal.

**Real design gap, found while writing the pytest coverage, not
anticipated in `PLAN.md`:** a real Cut genuinely edits content, but
`cut_text_field_selection` is called directly on `Tree`, never through
`Tree::dispatch` — so it never produces `DispatchOutcome::Changed` the
way keyboard editing gets "for free." Both `Window.cut()` and the real
winit-driven `InputEvent::Cut` handling needed an explicit `call_
handler(..., EventKind::Change, ...)` added, mirroring `Node.
set_checked`/`set_text`'s own established pattern — caught by a test
(`test_copy_fires_no_on_change_but_cut_does`) that would otherwise have
silently passed with `cut()` never firing `Change` at all. A real Cut
also writes to the clipboard *before* deleting the selection, and only
actually deletes once that write genuinely succeeds — a failed
clipboard write must never destroy the user's own selected text.

New `Window.copy()`/`cut()`/`paste(text)`: deliberately **hermetic**
(never touch the real OS clipboard) — unlike `press_key`/`type_text`,
a real Ctrl+C only ever originates from an actual OS-level keyboard
event reaching `engine-platform` directly, and there is no synthetic
way to drive that specific path from Python at all; a real, stated
scope boundary, not an oversight.

New `engine-core` tests (5, all passed first run): `text_field_
selected_text` reads without mutating, is `None` with no real
selection or a non-`TextField` node; `cut_text_field_selection` reads
and genuinely deletes, is a true no-op with no real selection. New
`engine-platform` tests (3): the real Copy/Cut/Paste vocabulary,
case-insensitivity (a real Ctrl+Shift+C is the same shortcut), every
other character/named key ignored. New, permanent `engine-py` test
(the first Rust unit test in this crate ever): a real `arboard`
set/get round trip, gracefully skipping if no clipboard is reachable.
New `tests/test_clipboard.py` (8 tests, one fixed after the real
Change-firing gap above): copy/cut/paste all reach the real focused
field's own selection correctly; copy never fires `on_change`, cut
does. New `examples/clipboard.py`: a real live field, select/copy/
cut/paste all proved via the hermetic surface, each step asserted.

Full `cargo test --workspace --release` (`engine-core` 122, up from
117; `engine-platform` 9, up from 6; `engine-py` 1, new)/`cargo clippy
--workspace --all-targets -- -D warnings`/`cargo fmt --check` all
clean — every prior test passed unmodified. `maturin develop --release`
+ full `pytest tests/` (161 passed, up from 153, 1 skipped) and all
twenty-four examples confirmed clean.
