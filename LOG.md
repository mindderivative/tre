# LOG — M45: Richer Reactivity (`Computed`, `Effect`, `batch()`, `untrack()`)

- User's own governing instruction: M44's own scoping named item 2
  ("richer reactivity") and deferred it explicitly ("no BUILD_TRACKER.md
  milestone number is claimed for item 2 by this entry"). The user later
  said simply "Start it." Entered Plan Mode fresh (a different task from
  M44's own plan file), did real investigation, wrote and got approval
  for a formal plan before implementing.
- Investigation, direct reads before designing: `Signal`/`RECORDING`/
  `_record_read` (`python/tre/__init__.py`, `crates/engine-py/src/
  view.rs`). **Real correction to M44's own scoping text:** `begin_
  recording`/`end_recording` were plain private `fn`s, never exposed to
  Python -- M44's own item-2 note had assumed otherwise. **Real,
  load-bearing bug found by tracing an actual reachable scenario:**
  `RECORDING` was `RefCell<Option<Vec<Py<PyAny>>>>` -- a flat slot. The
  binding grammar (`engine_spec::binding::Expression::Call`) already
  permits a zero-arg method call with real side effects; a binding
  evaluating inside `begin_recording()`/`end_recording()` whose own
  expression calls a method that writes a Signal would fire that
  Signal's `_notify()` synchronously, mid-evaluation -- if any
  subscriber opens its *own* nested recording scope (exactly what an
  eager `Computed`/`Effect` does), the inner `end_recording()` cleared
  the whole flat slot, silently and permanently destroying the outer
  binding's own already-recorded dependencies. Unreachable before this
  milestone; `Computed`/`Effect`'s own eager re-tracking is precisely
  the mechanism that would first make it reachable.
- `crates/engine-py/src/view.rs`: `RECORDING` -> `RefCell<Vec<Vec<Py
  <PyAny>>>>` (a real stack). `_record_read` now touches only `.last_
  mut()`. `begin_recording`/`end_recording` widened to `#[pyfunction]
  pub(crate) fn _begin_recording()`/`_end_recording() -> Vec<Py
  <PyAny>>`; confirmed directly (compiled, not assumed) that a
  `#[pyfunction]`-annotated fn stays a normal, directly-callable Rust
  item under its own name, so the two internal call sites in `attach_
  bindings_and_handlers` needed only a rename. **Real design
  simplification found during this same investigation:** `untrack(fn)`
  needs no third primitive -- `_begin_recording()`/`_end_recording()`
  around `fn()`, discarding the result, already hides its reads from the
  frame beneath (`_record_read` only touches the top) -- exactly
  `untrack`'s contract, for free.
- `crates/engine-py/src/lib.rs`: registered both new pyfunctions.
  `python/tre/_core.pyi`: matching stubs added next to `_record_read`'s.
- Verified the stack fix directly from Python (`_begin_recording`/
  `_end_recording` nesting correctly, isolated frames) before writing
  any Python-side reactivity code -- ran clean on the first try.
- `python/tre/__init__.py`: `Computed`, `Effect`, `batch`, `untrack`
  added, following the approved plan's design closely. `Signal.set`/
  `.update`'s final `self._notify()` call became `_schedule_notify
  (self)` -- confirmed a true no-op for every pre-M45 code path (nothing
  calls `batch()` yet anywhere existing) via the full pre-existing suite
  passing unmodified.
- Manual, interactive verification (before writing formal pytest
  coverage) caught two real bugs immediately, neither anticipated during
  scoping:
  1. **`batch()`'s first working version deduplicated by Signal, not by
     callback.** A `Computed` depending on two Signals both written
     inside one `batch()` recomputed *twice*, directly contradicting the
     milestone's own stated goal -- caught by a direct manual repro
     before the formal test suite even existed. Root cause: flushing by
     calling each pending Signal's own `._notify()` still re-invokes a
     shared subscriber once per Signal. Fixed by collecting a `set()` of
     callbacks across all pending signals' subscriber lists and
     invoking each exactly once -- confirmed first, via a tiny isolated
     script, that Python bound methods compare/hash by `(__self__,
     __func__)` identity even though each attribute access creates a
     structurally distinct wrapper object, before relying on that for
     the dedup.
  2. **`Signal._notify`/`Computed._notify` iterated `self._subscribers`
     live**, not a snapshot -- both had this shape since before M45.
     Nothing previously mutated a `_subscribers` list from inside one of
     its own callbacks; a `Computed`-of-`Computed` chain does exactly
     that (the first subscriber invoked can itself unsubscribe-then-
     resubscribe from that very list as part of its own recompute).
     Caught concretely by `examples/reactivity.py`'s own `Effect` log
     coming out `[10, 60, 60]` instead of the expected `[10, 30, 60,
     100]` -- a wrong, observable result, not a crash, exactly the kind
     of thing inspection alone would have missed. Fixed by snapshotting
     (`list(self._subscribers)`) before iterating, in both `_notify`
     implementations and in `batch()`'s own flush loop (the third live-
     iteration site, same root cause).
- **A third, related, real hazard found and deliberately routed around
  while writing the `RECORDING`-stack regression test itself (not
  fixed -- a genuinely separate, pathological pattern):** the first
  draft of the nested-recording regression test used a `Signal`
  subclass whose `.get()` override read `self.b.get()` (to compute an
  increment) *before* writing `self.b.set(...)`, while the outer "text"
  binding's own recording frame was still open. Since recording is
  ambient (it has no notion of call-stack depth), `b` got recorded into
  the *outer* frame too -- meaning the "text" binding ended up
  subscribed to `b`, and since its own evaluation also wrote to `b`,
  every `a.set()` triggered infinite reentrant recursion (confirmed via
  a `sys.setrecursionlimit`-bounded repro and stack tracing before
  concluding this, not guessed at). This is a real, separate hazard --
  a plain ViewModel method with the identical shape (read-before-write
  on a different Signal, during an open outer scope) would trigger it
  the same way, with or without `Computed`/`Effect` involved -- and
  isn't something this milestone's own `RECORDING`-stack fix can or
  should close (would need every plain `Signal.get()` to open its own
  isolated frame, a much larger change nothing in this codebase's
  existing binding semantics calls for). Fixed the *test*, not the
  library: `trigger_b` now sets a value it already has in hand (a plain
  counter), never reading the target Signal first -- isolates the test
  to the one real bug (real-bug-2's own discovery scenario) it exists
  to prove, and the example script's own `Signal.update()`-based writes
  were already immune (`update()` reads `self._value` directly, not via
  `.get()`/`_record_read`, so it never had this exposure).
- New pytest tests (`tests/test_reactivity.py`, +17): `Computed`
  (derivation, recompute-on-change, skip-notify-if-unchanged, Computed-
  of-Computed chaining, an unread Signal never triggering it, a `{{ }}`
  binding pointed directly at one -- the real duck-typing composition
  proof); `Effect` (runs on construction, reruns once per real
  dependency change, `dispose()` stops reruns, branch-dependent tracking
  only captures what was actually read last run); `batch()` (collapses
  writes into one pass, a shared Computed recomputes exactly once --
  bug-1's own regression proof, nested batch flushes only at the
  outermost exit, pending notifications still flush through a raised
  exception); `untrack()` (hides a read, returns the wrapped result);
  and the nested-recording regression test itself.
- New live example `examples/reactivity.py` + `.yaml`: a Computed-of-
  Computed price/quantity/total/label chain, a `{{ }}` binding on the
  outer Computed, a real Effect log, `batch()` collapsing two related
  writes per click into exactly one recompute -- verified end to end,
  including fixing one arithmetic error in my own first draft's
  assertions (expected recompute count off by one, since the counting
  wrapper is installed *after* `Computed.__init__`'s own first, real
  recompute already ran).
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (`engine-py` 15, unchanged -- the RECORDING stack is inherently pyo3/
  GIL-bound, matching this crate's own established split, no new Rust-
  level `#[test]`s needed); `maturin develop --release` rebuilt; `pytest
  tests/` (623 passed, up from 606, +17, 1 skipped, unchanged); all 82
  examples run individually with zero failures; `demo/showcase.py` (all
  5 phases, exit 0).
- `BUILD_TRACKER.md`: new Milestone 45 section, Top Metrics row, "Just
  closed" entry added; `tools/generate_tracker_artifact.py` confirmed 45
  milestones/137 phases/231 items (up from 44/136/229, the correct
  +1/+1/+2 for one new milestone, one phase, two steps); Build Tracker
  artifact republished to the existing URL.
- Explicitly out of scope, named: the self-referential read-before-write
  hazard (real, separate, needs a much larger redesign); diffing
  `Computed`/`Effect`'s dependency resubscription instead of always
  unsubscribing-then-resubscribing every one (deliberate simplicity, the
  real lists here are small); the `tesserae` re-export of `Computed`/
  `Effect`/`batch`/`untrack` (a small, separate follow-up, that repo's
  own convention).

**M45 -- Richer Reactivity: `Computed`, `Effect`, `batch()`, `untrack()`
-- is now fully complete, single phase.** This closes the milestone --
per the standing "push only after a full milestone closes" convention,
a `git push` is now appropriate.
