# Log: M19 Phase 1 — Real Hot-Reload Wiring (§16.4)

Corresponds to `BUILD_TRACKER.md` M19 Phase 1. `ViewWatcher` reaches a
real, live `View` for the first time, polling for a real file change
and calling `Reconciler::reconcile` to patch the live `Tree` in place.

## Investigation before writing code

**Real, significant, previously-unregistered finding:** `View` has no
live-window/render-loop concept of its own at all — confirmed via
direct read of `view.rs`'s own module doc comment and, decisively,
`examples/two_way_binding.py`'s own docstring, stated in plain words:
"`View` has no live-window/render-loop concept of its own... there's
no `App.run()` here." `Window`/`App` never reference `View` at all
(confirmed via grep) — `View` and the real, GPU-backed render loop are
two entirely separate, never-connected systems today. This meant
"wire `ViewWatcher` into a real running `App`'s frame loop" (the
scoping paragraph's own literal words) wasn't literally buildable as
stated — there is no such frame loop to hook into. The honest
translation: `View` gained its own real, explicit, caller-invoked poll
method instead, matching §16.4's own "the caller's own frame loop
calls `poll_changed()` once per tick" as closely as `View`'s real,
pre-existing, stated architecture allows.

`View::new` read its own `path` but never stored it — fixed, since a
reload needs to re-read the same file later.

**Real, stated scope boundary, not a silent gap:** `bindings:`/
`handlers:`/`two_way:` are resolved entirely separately from
`Reconciler::reconcile`, by `View::_attach`'s own `BindingResolver`
logic against a `viewmodel` this phase's new method has no access to.
`poll_reload` patches structure/paint/layout only — a hot-reloaded
view that adds a genuinely *new* binding or handler needs `_attach`
called again, the caller's own responsibility, the same as after
adding one imperatively. Re-running `_attach` automatically on every
reload risked double-subscribing a `Signal`'s re-evaluation callback
(not verified safe) and needed remembering the `viewmodel` too — real,
additional scope this phase deliberately did not take on.

## What happened

`View` gained `path: String` (remembered from construction) and
`watcher: Option<ViewWatcher>` — `None` only if the initial watch
genuinely failed (an unusual filesystem with no real
inotify-equivalent), non-fatal, the same "real, expected,
gracefully-handled" policy this codebase already applies to
no-GPU/no-display.

New `View.poll_reload(&mut self) -> PyResult<bool>`: `false` with no
watcher or no real change detected; otherwise re-reads `path`, calls
`self.reconciler.reconcile(...)`, and returns `true` — the real, first
Python-facing entry point for reconciliation, `ViewWatcher`/
`Reconciler` both already existed as tested `engine-spec` primitives
but nothing ever called them together from a real `View` before this.

New `tests/test_hot_reload.py` (3 tests, all passed first run, real
temp-file edits polled in a bounded loop matching `engine-spec::
watch`'s own established discipline, not a single fixed sleep): no
change reported before any write; a real edit reaches the live `Tree`
(a widget's `corner_radius` genuinely changes); an unchanged widget
keeps its real `NodeId` across a reload — mutated through a `Node`
obtained *before* the reload, read back through a fresh one obtained
*after*, the real end-to-end proof of `Reconciler`'s own documented
claim, reached through the Python FFI surface for the first time (the
`engine-spec` unit tests already proved it at the `Reconciler` level
in isolation). New `examples/hot_reload.py` + `hot_reload.yaml`: a
real file edit, detected and reconciled live, restoring the YAML to
its original content afterward so a repeat run stays clean.

Full `cargo test --workspace --release`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean — every
prior test passed unmodified (no new Rust-level tests were needed;
this phase's real coverage is at the FFI/integration level, proving
wiring already-tested `engine-spec` primitives together for the first
time). `maturin develop --release` + full `pytest tests/` (166 passed,
up from 163, 1 skipped) and all twenty-five examples confirmed clean.
