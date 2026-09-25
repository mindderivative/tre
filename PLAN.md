# PLAN — Milestone 87: Thread-Safe Callbacks into a Running `App`

*(Replaces the M86 plan — M86 is complete. Full scope, design
decisions, and step list live in `BUILD_TRACKER.md`'s own "Milestone
87" section.)*

## Goal

[`tre` issue #6](https://github.com/mindderivative/tre/issues/6):
nothing can hot-reload a live window, because `App.run()` never calls
back into Python and `App`/`Window`/`View` are single-threaded. User's
direction: "I would prefer the threadsafe option more and tesserae will
end up moving to a watcher driven by file-change events."

`App.thread_handle()` returns a `Send + Sync` `LoopHandle`; a
background thread calls `handle.call_soon(fn)`, the loop wakes, and
`fn` runs on the event-loop thread — the same queue + `EventLoopWaker`
pattern `Terminal`'s PTY reader thread already uses.

## Phases

1. **Callback queue, `LoopHandle`, drain** — done.
2. **Example, docs, verification:** `examples/threadsafe_reload.py`,
   MkDocs (`app.md`, declarative-views hot-reload section), full chain.

## Status

Phase 1 complete; Phase 2 next.
