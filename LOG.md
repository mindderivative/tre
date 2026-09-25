# LOG — Milestone 87, Phase 1: Callback Queue, `LoopHandle`, Drain

- User-directed: "Post the comment and start M87" (direction comment
  posted on issue #6).

## What shipped

1. New `engine-py/src/thread_handle.rs`: `CallQueue` (queue of
   `Py<PyAny>` + an `Option<EventLoopWaker>` slot behind one
   `Arc<Mutex<...>>`) and the `LoopHandle` pyclass (`frozen`,
   `Send + Sync`) with `call_soon(fn)`.
2. `App` owns a `CallQueue`; `App.thread_handle()` returns a
   `LoopHandle` over it. `run()`'s `setup` closure installs the waker,
   the per-frame closure drains the queue first thing (before the
   `active` re-sync), and the waker is cleared when `run()` returns.
3. Drain takes the whole queue under the lock and releases it before
   running Python, so a callback that calls `call_soon` itself can't
   deadlock. Exceptions go through `dispatch::log_uncaught_exception`
   (widened to `pub(crate)`). After running anything, the loop is
   woken once more so every window re-checks its dirty flag.
4. `tre.LoopHandle` exported; `_core.pyi` stubs for
   `App.thread_handle` and `LoopHandle.call_soon`.
- Tests: 4 Rust tests (FIFO, raising callable doesn't stop the rest,
  re-queue during drain runs next drain, non-callable `TypeError`); 9
  pytest cases, the live ones in a fresh subprocess each with an
  honest skip when no frame ran.
- Found: pyo3 refuses an `unsendable` object on another thread with a
  `PanicException`, which derives from `BaseException` — pinned in a
  test, since it is exactly why `LoopHandle` exists.
- Not tested end-to-end: a callable queued after `run()` returns
  waiting for a later `run()` — needs two `App.run()` calls in one
  process.
- Verification: `fmt --check`/`clippy -D warnings` clean; `maturin
  develop --release`; `pytest tests/` 923 passed, 2 skipped; `mypy
  --strict` on `_core.pyi` clean.

## Status

**Phase 1 complete.** Next: Phase 2 — example, docs, full chain.
