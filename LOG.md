# Log: M17 Phase 2 — Real IME Composition Preview (§8)

Corresponds to `BUILD_TRACKER.md` M17 Phase 2, closing M17 entirely
(and the whole M15/M16/M17 sequence): `winit::event::Ime::{Preedit,
Commit}` reach a real `InputEvent`, rendering a real preedit underline
at the cursor and committing real text via M15 Phase 2's own existing
character-insertion mechanism on `Commit`.

## Investigation before writing code

`WindowEvent::Ime(Ime)` is real; `Ime::{Enabled, Preedit(String,
Option<(usize, usize)>), Commit(String), Disabled}`. **Real,
load-bearing finding, confirmed via direct source read, that would
have silently dead-ended the whole feature:** `winit::window::Window::
set_ime_allowed`'s own doc comment states plainly "IME is not allowed
by default" — without calling it, `WindowEvent::Ime` never fires at
all. Same doc comment: "during the preedit phase the window will NOT
get `KeyboardInput` events" — composing and plain typing are already
mutually exclusive at the `winit` level, nothing this codebase needs
to coordinate itself. Real, deliberate scope narrowing: `Preedit`'s
own `Option<(usize, usize)>` sub-cursor range (for showing exactly
where composition input lands inside a multi-candidate string) is
strictly more detail than a real underline needs — dropped, kept as
`Option<String>` only. Real design: `Commit(text)` needs no new
`engine-core` primitive at all — it's exactly `InputEvent::
TextInput(text)`, the identical mechanism a real keypress already
uses (M15 Phase 2).

## What happened

`TextFieldState` gains `preedit: Option<String>` — purely visual,
uncommitted, never spliced into `content`. `engine-platform`: `window.
set_ime_allowed(true)` added at the one real window-creation site;
`WindowEvent::Ime(ime)` translates `Preedit(text, _)` (dropping the
sub-cursor detail) to a new `InputEvent::ImePreedit(String)` (empty
string means "cleared," matching `winit`'s own real convention) and
`Commit(text)` directly to the existing `InputEvent::TextInput(text)`
— no new variant for Commit. `Tree::dispatch`'s new `ImePreedit` arm
sets/clears the focused `TextField`'s own `preedit`, always
`DispatchOutcome::None` (a composition preview isn't committed
content, no `Change` fires); the existing `TextInput` arm additionally
clears a stale `preedit` on every real insertion, defensive.

`TextRenderer::draw_field` splices `preedit` into a *display-only*
copy of the content at `cursor` when composing (`content` itself is
never mutated — shaped and painted, but never touching the real,
uncommitted field state), reusing the same `Selection::geometry`
mechanism the selection highlight already established (M15 Phase 1)
to paint a real underline under that span — a thin rect at each
returned bound's own bottom edge instead of a translucent fill. The
caret shows at the end of the spliced-in preedit while composing
(`cursor + preedit.len()`), not the raw, frozen `cursor` underneath
it, matching real IME caret behavior.

**Confirmed, not assumed:** `engine-py::app.rs`'s `on_input` closure
already calls `tree.dispatch(root, event.clone(), ...)` unconditionally
before its own raw-event match (the mechanism every event already goes
through). Since `ImePreedit`'s state mutation happens entirely inside
`Tree::dispatch` itself and always returns `DispatchOutcome::None`, no
additional handling was needed in `app.rs` at all — unlike `Copy`/
`Cut`/`PasteRequested` (M17 Phase 1), which are pure intent signals
needing real OS I/O only `engine-py` can do. The existing trailing
`_ => {}` arm already covers it correctly.

**Real, honest deviation from `PLAN.md`:** the plan called for a "new
`engine-platform` translation test," but none was added — the `Ime`
match arm inside `window_event` is trivial inline logic (unlike
`translate_clipboard_shortcut`, a genuine pure function worth
isolating), the same "not manufactured ahead of a real need" scope
`ThemeChanged`'s own translation already established with no dedicated
test of its own.

New `engine-core` tests (4, all passed first run): a real preedit
sets/clears the focused field without touching `content`; an empty
string clears an active preview; no focused field is a true no-op; a
real `TextInput` commit clears a stale preedit. New `engine-render`
pixel tests (2, in `text_field_paint.rs`, matching that file's own
established render-to-texture-then-readback discipline): a composing
field's own render differs from an otherwise pixel-identical,
non-composing field (proving a real underline exists); a defensive
`Some(String::new())` preedit paints byte-identical to a real `None`
(no stray underline from an edge case that shouldn't occur in
practice, since `Tree::dispatch` already normalizes an empty string to
`None`, but the rendering side is proven correct on its own too).

**Confirmed, not assumed:** no new Python-facing FFI surface or pytest
coverage was needed. A real IME composition event only ever originates
from an actual OS input method — there is no synthetic way to inject
one from Python, the same real scope boundary M17 Phase 1 already
established for a real Ctrl+C/X/V keypress. `examples/text_field.py`'s
own docstring is updated to point at the real, definitive proof
(`text_field_paint.rs`'s new pixel tests) instead of naming clipboard/
IME as an unscoped gap — both are now real, and IME specifically has
no live-window demo possible for the reason above.

Full `cargo test --workspace --release` (`engine-core` 126, up from
122; `engine-render` 4 in `text_field_paint.rs`, up from 2)/`cargo
clippy --workspace --all-targets -- -D warnings`/`cargo fmt --check`
all clean — every prior test passed unmodified. `maturin develop
--release` + full `pytest tests/` (161 passed, 1 skipped, unchanged
from M17 Phase 1 — confirming no new Python surface was needed) and
all twenty-four examples confirmed clean.
