# Plan: M17 Phase 2 — Real IME Composition Preview (§8)

Corresponds to `BUILD_TRACKER.md` M17 Phase 2's own scoping, closing
M17 (and the whole M15/M16/M17 sequence) entirely: `winit::event::
Ime::{Preedit, Commit}` reach a real `InputEvent`, rendering a real
preedit underline at the cursor and committing real text via M15
Phase 2's own existing character-insertion mechanism on `Commit`.

## Investigation before writing code

- `WindowEvent::Ime(Ime)` is real (confirmed via direct source read);
  `Ime::{Enabled, Preedit(String, Option<(usize, usize)>), Commit
  (String), Disabled}` already confirmed at scoping time.
- **Real, load-bearing finding, would have silently dead-ended the
  whole feature:** `winit::window::Window::set_ime_allowed`'s own doc
  comment states plainly **"IME is not allowed by default"** — without
  calling it, `WindowEvent::Ime` never fires at all, confirmed via
  direct source read. Also real and relevant: "during the preedit
  phase the window will NOT get `KeyboardInput` events" — composing
  and plain typing are mutually exclusive at the `winit` level, not
  something this codebase needs to coordinate itself. Every window
  needs `window.set_ime_allowed(true)` called once, at creation
  (`engine-platform`'s own `user_event`'s `OpenWindow` handling, the
  one real place a `Window` is actually created).
- Real, deliberate scope narrowing: `Ime::Preedit`'s own `Option<
  (usize, usize)>` names a *sub-cursor range within the preedit text
  itself* (for showing exactly where composition input lands inside a
  multi-candidate string) — real, but strictly more detail than "a
  preedit underline" needs; dropped, kept as `Option<String>` only
  (the composition text itself). A real, stated simplification, not
  silently lost — the underline still covers the whole preedit span
  correctly regardless.
- Real design: `Ime::Commit(text)` needs no new `engine-core`
  primitive at all — it's exactly `InputEvent::TextInput(text)`, the
  identical mechanism a real keypress already uses (M15 Phase 2).
  `Ime::Preedit` does need new state: a real composition preview isn't
  committed content, so it can't just be spliced into `content`
  directly — `TextFieldState` gains `preedit: Option<String>`, a
  purely visual, uncommitted string, cleared to `None` on `Commit`
  (defensive — a stale preedit must never survive past the text it
  was composing).
- Rendering design: `TextRenderer::draw_field` already builds one
  shared `parley::Layout` per paint call (M15 Phase 1) and already
  reuses `Selection::geometry` for the selection-highlight rect (M15
  Phase 1) — a real preedit span is geometrically the same shape (a
  real `[start, end)` byte range within the shaped text), so the
  identical mechanism paints a real underline instead of a fill, no
  new geometry primitive needed. The real content actually *shaped*
  for one paint call becomes `content` with `preedit` spliced in at
  `cursor` when composing — `content` itself stays uncommitted the
  whole time, exactly matching real IME behavior (nothing is "typed"
  until a real `Commit`).

## Design

- `TextFieldState` gains `preedit: Option<String>`.
- `engine-platform`: `window.set_ime_allowed(true)` at window creation;
  `WindowEvent::Ime(ime)` translates `Preedit(text, _)` (dropping the
  sub-cursor detail) to a new `InputEvent::ImePreedit(String)` (empty
  string means "cleared," matching `winit`'s own real convention) and
  `Commit(text)` directly to the existing `InputEvent::TextInput
  (text)` — no new variant for Commit at all.
- `Tree::dispatch`'s new `ImePreedit` arm sets/clears the focused
  `TextField`'s own `preedit` — a true `DispatchOutcome::None` (a
  preview isn't a real content change, no `Change` fires). The
  existing `TextInput` arm additionally clears `preedit` on every real
  insertion (defensive, always correct).
- `TextRenderer::draw_field` splices `preedit` into the shaped text at
  `cursor` (display-only) when composing, paints a real underline
  under that span (reusing `Selection::geometry`'s own real
  mechanism), and shows the caret at the end of the spliced-in preedit
  while composing.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests for `ImePreedit` dispatch and `TextInput`
clearing a stale preedit; new `engine-platform` translation test; new
`engine-render` pixel test proving a real preedit underline paints
only while composing; `maturin develop --release`; pytest coverage
for the hermetic FFI surface (if any is needed once the real design is
implemented — investigate whether IME needs any new Python-facing
entry point at all, given the real winit-only origin of `Ime` events,
the same real scope boundary M17 Phase 1 already established for
Ctrl+C/X/V); every example re-run; `LOG.md`/`BUILD_TRACKER.md`/tracker
artifact/commit/memory — closing M17 entirely (both phases) and the
whole M15/M16/M17 sequence.
