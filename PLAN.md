# PLAN — M45: Richer Reactivity (`Computed`, `Effect`, `batch()`, `untrack()`)

## Goal
Build the richer-reactivity layer M44's own scoping named and deferred
(item 2), on top of `tre.Signal`'s existing dependency-recording
primitive, per the approved plan
(`/home/phil/.claude/plans/reflective-sleeping-falcon.md`, entered fresh
after the user said "Start it").

## Steps
1. Investigated `Signal`/`_record_read`/`RECORDING` directly -- found
   `begin_recording`/`end_recording` were NOT already Python-exposed
   (correcting M44's own scoping assumption), and found `RECORDING` was
   a flat `Option<Vec<...>>`, not a stack -- a real, load-bearing
   correctness bug reachable the moment anything opens a nested
   recording scope (exactly what `Computed`/`Effect`'s eager
   re-tracking does).
2. `crates/engine-py/src/view.rs`: `RECORDING` -> `RefCell<Vec<Vec<Py
   <PyAny>>>>`; `_record_read` reads/writes only `.last_mut()`;
   `begin_recording`/`end_recording` widened to `#[pyfunction] pub
   (crate) fn _begin_recording`/`_end_recording`, registered in
   `lib.rs`. Internal call sites in `attach_bindings_and_handlers`
   renamed to match. `python/tre/_core.pyi` stubs added.
3. `python/tre/__init__.py`: `Computed`, `Effect`, `batch`, `untrack`.
   `Signal.set`/`.update` route through new `_schedule_notify` instead
   of calling `._notify()` directly.
4. Real bugs found and fixed by actually running the new tests/example:
   - `batch()` deduped by Signal, not by callback -- a shared `Computed`
     recomputed once per Signal it depended on, not once per batch.
     Fixed: dedupe by callback (`set()`, using bound-method equality).
   - `Signal._notify`/`Computed._notify` iterated `_subscribers` live --
     a `Computed`-of-`Computed`'s own recompute mutates that very list
     mid-iteration (unsubscribe+resubscribe), silently skipping/
     duplicating callbacks. Fixed: snapshot before iterating, in both
     `_notify`s and `batch()`'s own flush loop.
   - A separate, genuinely pathological hazard found while writing the
     `RECORDING`-stack regression test (a Signal read-before-write
     during an open outer recording scope gets misattributed as a
     dependency of that outer scope) -- named, deliberately routed
     around in the test, not fixed (out of scope, needs a larger
     redesign nothing else in this codebase calls for).
5. New pytest tests (`tests/test_reactivity.py`, +17): Computed/Effect/
   batch/untrack coverage plus the nested-recording regression proof.
6. New live example `examples/reactivity.py` + `.yaml`.
7. `BUILD_TRACKER.md`/artifact updated.

## Status
Complete, single phase. Full verification chain green: `cargo check`/
`clippy -D warnings`/`fmt`, `cargo test --workspace --release`
(`engine-py` 15, unchanged), `maturin develop --release`, `pytest
tests/` (623 passed, up from 606, +17, 1 skipped unchanged), all 82
examples (+1), showcase demo. **M45 -- Richer Reactivity -- is now
fully complete.** This closes the milestone -- per the standing "push
after a full milestone closes" convention, a `git push` is now
appropriate.
