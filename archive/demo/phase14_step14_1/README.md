# Demo: Phase 14 Step 14.1 -- System Clipboard (`tre.Clipboard`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase14_step14_1
../../.venv/bin/python demo.py
```

**What this proves.** Recommendation #4 from the [tre GUI Readiness
assessment](https://claude.ai/code/artifact/2d7cafa8-bf78-4c65-ade1-a2f3c0362196):
real system clipboard text access, via `arboard` -- the exact crate the
assessment itself named ("a small, self-contained cross-platform crate
with no architectural entanglement with the rest of tre"). Feasibility
(does this machine's real clipboard service actually work end to end
through `arboard`?) was verified in a throwaway scratch crate, outside
the repository, before touching any real `Cargo.toml` -- the same
discipline used for the `shaderc` dependency in Phase 13 Step 13.8.

**Real, disclosed v1 scope**: plain text only. `arboard`'s own
`default-features = false` drops its `image-data` feature entirely --
the identical "smallest real slice first" precedent `tre-svg`'s own
`usvg = { default-features = false }` already establishes. Image
clipboard support is real, separate future work once a caller actually
needs it.

**Architecture**: `tre_platform::Clipboard` (new `crates/tre-platform/
src/clipboard.rs`) is a thin wrapper -- `new()`/`get_text()`/
`set_text()`, each mapping a real `arboard::Error` into
`PlatformError::Other` with a real, specific message (`"failed to open
the system clipboard: ..."`, etc.), not a generic failure. `tre-python`
binds this directly as `tre.Clipboard`, marked `unsendable` (matching
`PlatformConnection`'s own precedent for platform-connection state that
isn't safely `Send`).

Every check in `demo.py` round-trips through the **real, live OS
clipboard service** on this machine -- not a mock or an in-process
stand-in:

- A basic `set_text`/`get_text` round-trip, using a fresh UUID marker
  per run (the system clipboard is real, shared, stateful OS state that
  may already hold unrelated content -- the test never assumes it
  starts empty).
- A second `set_text` call fully replaces the first.
- Real multi-byte UTF-8 content -- accented Latin, a CJK phrase, and a
  real 4-byte-UTF-8 emoji -- round-trips byte-exact, proving this isn't
  an ASCII-only path through `arboard`/the platform clipboard service.

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-platform` 0 -> 1 test: a
  real round-trip against this machine's own live clipboard service,
  not mocked -- matching this project's own "real demos/tests as the
  correctness oracle" precedent for platform-level code).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion passes against this machine's real clipboard.

**Not yet done**: image/rich-text clipboard content (a real `arboard`
feature, deliberately deferred); `EditableText` (Phase 13 Step 13.5)
has no `.cut()`/`.copy()`/`.paste()` convenience methods wired to this
`Clipboard` yet -- a real, natural next pairing now that both pieces
exist independently.
