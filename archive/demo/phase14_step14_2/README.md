# Demo: Phase 14 Step 14.2 -- Cut/Copy/Paste (`EditableText` + `Clipboard`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase14_step14_2
../../.venv/bin/python demo.py
```

**What this proves.** Wires the real system `Clipboard` (Step 14.1)
into the real single-line text editor `EditableText` (Phase 13 Step
13.5) -- the natural pairing both steps' own READMEs already named as
the obvious next step once each shipped independently.

`EditableText.copy(clipboard)`/`.cut(clipboard)`/`.paste(clipboard)`
take a `Clipboard` instance explicitly rather than `EditableText`
owning one internally -- a real design choice, not an oversight: a real
app has **one** system clipboard connection shared by every text field
in it, not one per field, and `EditableText` never pays the real cost
of opening a clipboard connection (a live platform service handle)
unless a caller actually invokes cut/copy/paste. `PyClipboard::get_text`/
`set_text` were widened from private to `pub(crate)` so `EditableText`
can call them directly as plain Rust methods, with no new public
surface on `Clipboard` itself.

Every check verifies a real, exact behavior:

- `copy()` writes the active selection to the clipboard and leaves
  `text` completely unchanged.
- `cut()` writes the selection to the clipboard, removes it from
  `text`, and leaves the caret at the cut's own start.
- `paste()` inserts the clipboard's current text at a plain caret, and
  correctly *replaces* an active selection instead when one exists --
  the same real semantics `insert()` already has everywhere else in the
  class, reused rather than duplicated.
- `copy()`/`cut()` with **no** active selection are real no-ops: the
  clipboard's own prior content is left completely untouched, matching
  every real text field's own standard behavior (there is no
  "clipboard cleared" surprise).
- A real UTF-8 selection (`"café"`, where `"é"` is a genuine 2-byte
  character) cuts and round-trips through the clipboard byte-exact,
  reusing `delete_selection`'s own already-real char-boundary safety
  rather than needing new UTF-8 handling here.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes.

**Not yet done**: keyboard-shortcut detection (Ctrl+X/C/V) is
deliberately not wired here -- matching this project's own established
"layout-aware translation is the UI framework's job" stance for
`KeyboardKey`'s own raw key codes, a real caller decides when to invoke
these three methods from its own key-handling code.
