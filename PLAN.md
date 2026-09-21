# PLAN — M46: Reentrant-Notification Guard for the Read-Before-Write Hazard

## Goal
Fix the real hazard M45's own scoping named and deliberately left
unfixed: a `Signal.get()`/method read of a different Signal before
writing to it, while an outer recording scope is open, can produce a
binding subscribed to a Signal its own evaluation writes to —
unbounded reentrant recursion with no error. Per the approved plan
(`/home/phil/.claude/plans/reflective-sleeping-falcon.md`).

## Steps
1. Re-traced the original failure: the dangerous consequence is always
   a single Signal/Computed's own `_notify()` re-entering itself, not
   a genuinely unbounded chain — a narrow, well-defined shape a
   per-object reentrancy guard can catch for any cause.
2. Deliberate scope decision: fix the *consequence* (crash -> clear
   error), not the *cause* (over-broad attribution) — the latter would
   need every `Signal.get()` to open its own isolated frame, invasive
   for a case `untrack()` already targets.
3. Found a real gap: `batch()`'s own flush loop doesn't call
   `_notify()` at all (bypasses a guard placed only there).
4. `python/tre/__init__.py`: factored `Signal`/`Computed`'s already-
   duplicated `_subscribe`/`_unsubscribe`/`_notify` into a shared
   `_Notifiable` base; `_notify(already_invoked=None)` now carries a
   `_notifying` reentrancy flag (raises a clear `RuntimeError` on
   re-entry) and the dedup/snapshot logic in one place. `batch()`'s
   flush threads its shared dedup set into `_notify` instead of
   duplicating the loop — closing the batch-path gap with the same
   code the immediate path uses.
5. Confirmed `Computed`/`Effect` don't need a second guard on
   `_recompute`/`_run` — both already unsubscribe-before-run/resubscribe
   -after, so a write inside `fn` can't self-reenter that method.
6. Fixed `__all__` to include `Computed`/`Effect`/`batch`/`untrack`
   (missing since M45, a real small correction found while here).
7. 6 new pytest tests (`tests/test_reactivity.py`): isolated guard
   proof, guard-resets-after-exception, legitimate non-cyclic chain
   doesn't trip it, batch-path hits the same guard, the original
   real hazard now raises cleanly, and `untrack()` genuinely fixes it
   (verified end to end, not just asserted).
8. `BUILD_TRACKER.md`/artifact updated.

## Status
Complete, single phase. No Rust changes needed. Full verification
chain green: `pytest tests/` (629 passed, up from 623, +6, 1 skipped
unchanged), all 82 examples, showcase demo. Full pre-existing
`test_reactivity.py` M45 suite (17 tests) confirmed passing unmodified
— the `_Notifiable` refactor is behavior-preserving. **M46 -- Reentrant-
Notification Guard -- is now fully complete.** This closes the
milestone — per the standing "push after a full milestone closes"
convention, a `git push` is now appropriate.
