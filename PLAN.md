# Plan: M16 Phase 2 — Migrate Real Call Sites to `tracing` (§9)

Corresponds to `BUILD_TRACKER.md` M16 Phase 2's own scoping, closing
M16 entirely: `dispatch.rs`'s `call_handler`/`run_completions` and
`app.rs`'s two `eprintln!` sites become real `tracing::error!`/
`tracing::warn!` events, preserving the *full* real traceback text
`PyErr::print` already gives.

## Investigation before writing code

- **Real finding beyond the original scoping:** `run_completions` has
  its own real `err.print(py)` site the M16 scoping paragraph never
  named (only `call_handler` and the two `app.rs` sites were listed)
  — confirmed via grep, a third real call site to migrate, not two.
- `PyErr` has no single method that both formats the *full* traceback
  (frames included) and returns it as an owned `String` — `PyErr::
  print`/`display` both write straight to `sys.stderr`; `PyErr`'s own
  `Display` impl gives only the exception's type/message, no frames
  (confirmed via direct source read of both). `err.traceback(py)`'s
  own real `.format()` (`pyo3::types::PyTracebackMethods`, confirmed
  via direct source read — its own doc example is literally `format!
  ("{}{}", traceback.format()?, err)`) is the real way to get the
  identical full text as an owned `String`.

## Design

- New `dispatch::log_uncaught_exception(err, py)`: renders the full
  traceback via `err.traceback(py)`'s `.format()`, falling back to
  plain `Display` if the traceback is missing or fails to format;
  emits `tracing::error!(%traceback, "uncaught exception in a Python
  callback")`. Both `call_handler` and `run_completions` call it in
  place of `err.print(py)`.
- `app.rs`'s two `eprintln!` sites (no-GPU-adapter, no-display) become
  `tracing::warn!` — real, worth logging, but expected/gracefully-
  handled conditions, not errors.

## Verification plan (see also the real findings recorded in `LOG.md`)

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
`maturin develop --release`; full `pytest tests/` run to catch any
real behavioral change the migration causes (not assumed safe); every
example re-run, including a manual `RUST_LOG=info` check against the
default (unset) case to prove verbosity is genuinely dialable and
genuinely quiet by default; `LOG.md`/`BUILD_TRACKER.md`/tracker
artifact/commit/memory — closing M16 entirely (both phases).
