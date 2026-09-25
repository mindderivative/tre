# LOG — Milestone 87, Phase 2: Example, Docs, Verification

- User-directed: "Post the comment and start M87". Phase 1 (`1c1013f`)
  added `App.thread_handle()` / `LoopHandle.call_soon`.

## What shipped

1. `examples/threadsafe_reload.py`: a dependency-free `os.stat`
   watcher thread that reads the changed file itself and hands
   `view.reconcile(source=text)` to the loop via `call_soon`. Works on a
   temp copy of `hot_reload.yaml`. Asserts the reload landed whenever a
   frame ran; skips only when none did.
2. Docs: `api/python/app.md` gains `thread_handle` and `LoopHandle`;
   `declarative-views.md` gains "Hot reload inside `App.run()`";
   `api/python/index.md` lists `LoopHandle` (and drops a class count
   that had been wrong since M85).

## Real finding

A `max_frames`-bounded run counts idle frames too, and an idle frame
costs microseconds — 6000 frames finished in 63 ms. The example's first
draft never saw its reload, and the Phase 1 live pytest was passing
only because its worker thread happened to be fast. Both now make the
first-frame callable wait for the background thread to finish queueing:
deterministic, and the queueing still happens on a different thread.

## Verification

`mkdocs build --strict` clean; `fmt --check`/`clippy -D warnings`
clean; `cargo test --workspace --release` 47 suites, 540 passed, 0
failed; `pytest tests/` 923 passed, 2 skipped; all 89 examples plus
`demo/showcase.py` run clean.

## Status

**M87 complete.** Nothing further is scoped on the `0.3.2` branch.
