# Plan: M19 Phase 1 — Real Hot-Reload Wiring (§16.4)

Corresponds to `BUILD_TRACKER.md` M19 Phase 1: `ViewWatcher` reaches a
real, live `View` for the first time, polling for a real file change
and calling `Reconciler::reconcile` to patch the live `Tree` in place.

## Investigation before writing code

- **Real, significant, previously-unregistered finding:** `View` has
  **no live-window/render-loop concept of its own at all** — confirmed
  via direct read of `view.rs`'s own module doc comment and, decisively,
  `examples/two_way_binding.py`'s own docstring, stated in plain words:
  "`View` has no live-window/render-loop concept of its own... there's
  no `App.run()` here." `Window`/`App` (`window.rs`/`app.rs`) never
  reference `View` at all (confirmed via grep) — `View` and the real,
  GPU-backed render loop are two entirely separate, never-connected
  systems today. This means "wire `ViewWatcher` into a real running
  `App`'s frame loop" (the scoping paragraph's own literal words) isn't
  literally buildable as stated — there is no such frame loop to hook
  into. The honest translation: `View` gains its own real, explicit,
  caller-invoked poll method, matching §16.4's own "the caller's own
  frame loop calls `poll_changed()` once per tick" as closely as
  `View`'s real, pre-existing, stated architecture allows.
- `View::new` reads its own `path` via `std::fs::read_to_string` but
  never stores it — a real, necessary fix, since a reload needs to
  re-read the same file later.
- `Reconciler::reconcile(&mut self, tree, yaml, sheet, scheme) ->
  Result<(), SpecError>` is real and already used correctly by
  `engine-spec`'s own tests (keyed diffing, unchanged nodes keep their
  real `NodeId`/focus/animations, confirmed via direct read).
- **Real, stated scope boundary, not a silent gap:** `bindings:`/
  `handlers:`/`two_way:` are resolved entirely separately from
  `Reconciler::reconcile` — by `View::_attach`'s own `BindingResolver`
  logic, against a `viewmodel: Py<PyAny>` that `View` doesn't currently
  store either. A hot-reloaded view whose YAML adds a *new* binding or
  handler needs `_attach` called again — this phase's own module doc
  comment already half-anticipates this split ("a change that adds a
  new binding or handler calls back into the `BindingResolver`" is
  named as a distinct case from plain styling patches). Re-running
  `_attach` automatically on every reload risks double-subscribing a
  `Signal`'s re-evaluation callback (not verified safe) and needs
  remembering the `viewmodel` too — real, additional scope this phase
  deliberately does not take on. `poll_reload` patches structure/
  paint/layout only; an app that adds a genuinely new binding after a
  hot-reload must call `_attach` again itself, the same way it would
  after adding one imperatively.
- A construction-time `ViewWatcher::watch` failure (an unusual
  filesystem with no real inotify-equivalent) is treated as non-fatal,
  the same "real, expected, gracefully-handled" policy this codebase
  already applies to no-GPU/no-display (TRE v1 finding #261) — `View`
  still works, `poll_reload` just always reports no change.

## Design

- `View` gains `path: String` (remembered from construction) and
  `watcher: Option<ViewWatcher>` (`None` if the initial watch failed,
  logged via `tracing::warn!`, non-fatal).
- New `View.poll_reload(&mut self) -> PyResult<bool>`: `false` with no
  watcher or no real change detected (`ViewWatcher::poll_changed()`);
  otherwise re-reads `path`, calls `self.reconciler.reconcile(...)` to
  patch the live `Tree`, and returns `true`. Structural/paint/layout
  changes only — bindings/handlers are the caller's own responsibility
  to re-attach, per the stated scope boundary above.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
`maturin develop --release`; new `tests/test_hot_reload.py` — a real
temp file, edited on disk between two `poll_reload()` calls, proving a
real structural change (a widget's style/content) reaches the live
`Tree` and an *unchanged* widget's own `NodeId` survives (matching
`Reconciler`'s own existing `engine-spec` unit-test claim, now proven
end to end through the real Python FFI surface for the first time);
every example re-run; `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/
commit/memory.
