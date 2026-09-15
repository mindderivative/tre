# Demo: Phase 14 Step 14.3 -- Native File Dialogs (`tre.pick_file` etc.)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase14_step14_3
../../.venv/bin/python demo.py
```

**What this proves.** Recommendation #5 from the [tre GUI Readiness
assessment](https://claude.ai/code/artifact/2d7cafa8-bf78-4c65-ade1-a2f3c0362196):
real native file dialogs, via `rfd` -- backed by the XDG desktop portal
(`ashpd`) on Linux, confirmed against this machine's real, live KDE
Plasma portal implementation (both the GTK and KDE portal backends are
registered) before touching any real `Cargo.toml`.

**A real bug caught by testing the actual blocking call, not just
linking it**: the first feasibility pass built and linked cleanly with
rfd's `tokio` feature, but the very first real `pick_file()` call
PANICKED at runtime -- `"there is no reactor running, must be called
from the context of a Tokio 1.x runtime"`. rfd's blocking API drives the
portal's async calls itself via `pollster::block_on`, not a Tokio
executor, so `tokio` was the wrong runtime feature despite compiling
fine. Switching to `async-std` fixed it -- confirmed by calling the real
blocking API (not just constructing the builder) in the same isolated
scratch crate, three repeated runs, before changing the real dependency.
This is the reason this project's own "verify feasibility in complete
isolation" discipline (established for `shaderc`/Step 13.8 and
`arboard`/Step 14.1) now explicitly means *calling* the real API, not
just linking against it -- a lesson this step adds to that precedent.

**Architecture**: `tre_platform::file_dialog` (new
`crates/tre-platform/src/file_dialog.rs`) is a set of plain functions
(`pick_file`/`pick_files`/`pick_folder`/`save_file`), not a struct --
unlike `Clipboard`, a file dialog has no persistent connection to hold;
each call opens a fresh native dialog and blocks until the user
responds. `tre-python` binds these directly as top-level `tre.pick_file`
etc., releasing the GIL for the blocking call via `py.detach` (matching
`PyHeadlessRenderer::submit_and_read_bgra`'s own established precedent
for real blocking work) -- a real native dialog can block on user
interaction for an unbounded amount of real wall-clock time, and other
Python threads must keep running while it does.

**Real, disclosed limit on what an automated demo can prove**: a native
file dialog needs a real human to click something in it -- there is no
way to script a GTK/portal chooser window the way other demos in this
project drive an in-process GPU renderer or a system service like the
clipboard. `demo.py` proves what it honestly can, automatically, with
real assertions:

- Malformed arguments (e.g. `filters` given the wrong shape) raise a
  real `TypeError` from PyO3's own argument extraction, before any
  dialog opens.
- The GIL is genuinely released during the blocking call: a background
  thread calls `pick_file()` while the main thread keeps making real,
  counted progress (an exact iteration count, not just "the process
  didn't freeze").
- The call genuinely blocks on a real, live portal round trip rather
  than erroring out or returning instantly (a silent no-op would return
  immediately instead).

**Not yet done**: full interactive click-through (does the dialog
actually return the path a human picked?) needs a human at a real
desktop running this demo -- real, separate manual verification, not
something this automated test suite can perform on its own.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug.
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes.
