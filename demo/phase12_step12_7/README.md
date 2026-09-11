# Demo: Phase 12 Step 12.7 -- Drag-and-Drop / IME / Cursor Passthrough (`tre-python`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase12_step12_7
../../.venv/bin/python demo.py
```

**What this proves.** Three real, previously-missing capabilities,
found free inside `winit` (tre's own pinned platform backend) during a
GUI-framework gap assessment, now forwarded through `tre-engine`'s
`InputEvent` and exposed to Python:

- **Drag-and-drop**: `InputEvent` gained `FileDropped`/`FileHovered`/
  `FileHoverCancelled`, translated from winit's own real
  `WindowEvent::DroppedFile`/`HoveredFile`/`HoveredFileCancelled`
  (verified against winit 0.30.13's actual source, not assumed).
- **IME composition**: `InputEvent` gained `ImeEnabled`/`ImePreedit`/
  `ImeCommit`/`ImeDisabled`, translated from winit's real `WindowEvent::
  Ime(..)`. A new `WindowedRenderer.set_ime_allowed(window, bool)`
  mirrors the real platform requirement: winit never emits any IME event
  for a window until this has been called with `True` for it.
- **Cursor customization**: a new `tre.CursorIcon` enum (the real,
  complete CSS3 cursor set winit itself exposes) plus
  `WindowedRenderer.set_cursor(window, icon)`.

`InputEvent` lost its `Copy` derive as a real, disclosed consequence --
`FileDropped`/`FileHovered`'s own `PathBuf` and the IME variants' own
`String` payload aren't `Copy`. Every real call site already took
`InputEvent` by value, so this needed no call-site rewrites beyond one
`*event` dereference in `windowed_renderer.rs`'s own `Resized` handling,
which became a plain reference match.

**Real, disclosed scope limit:** this sandbox has no input-injection
tool (`xdotool`/`wtype`/`ydotool` all confirmed absent), so a real file
drop or real IME composition can't be synthesized here to prove event
*delivery* end to end the way a human dragging a file onto the window
would. What's verified instead: the real `WindowEvent` translation logic
was checked directly against winit 0.30.13's own source (not assumed),
and every new API call (`set_cursor`, `set_ime_allowed`) succeeds
against a real window with no error.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --
  -D warnings` / `cargo build --workspace --all-targets` / `cargo test
  --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in prior steps' READMEs --
  still flagged as separate follow-up work, not fixed here.
- `demo.py` run against real GPU hardware and a real window via `maturin
  develop --release`: exits 0, all new API calls succeed.

**Not yet done:** scale/rotation are still not exposed on any Python
shape (found during this same gap assessment, but out of this step's
explicit scope); native drag targets for in-app drag gestures (as
opposed to OS file drops, which this step covers) remain a UI-framework
concern, per this project's own established input-event scope boundary.
