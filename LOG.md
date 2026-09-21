# LOG — M46: Reentrant-Notification Guard for the Read-Before-Write Hazard

- User's own governing instruction: M45's own `BUILD_TRACKER.md` entry
  named a real hazard and deliberately left it unfixed. The user then
  asked directly: "Scope and fix the Signal read-before-write on a
  different Signal during an open recording scope hazards." Entered
  Plan Mode fresh (a different task from M45's own plan file), did real
  investigation, wrote and got approval for a formal plan before
  implementing.
- Investigation, direct reasoning before designing: re-traced the
  original failure (kept from M45's own `sys.setrecursionlimit` +
  instrumented-print debugging session) and confirmed the dangerous
  *consequence* always has the same narrow shape regardless of cause --
  a single Signal/Computed's own `_notify()` re-entering itself several
  frames down, never a genuinely unbounded chain of distinct objects.
  This is what makes a per-object reentrancy guard the right fix: it
  catches the *pattern*, not just the one scenario already found.
- **Real, deliberate scope decision, stated not assumed:** considered
  and rejected fixing the *cause* (over-broad dependency attribution --
  a Signal read buried in a call stack getting recorded into whatever
  outer recording frame happens to be open) -- would need every plain
  `Signal.get()` call to open its own isolated recording frame, an
  invasive change to the hot path of every Signal read, to control for
  a call-stack-depth distinction nothing else in this engine's binding
  semantics has ever needed. `untrack()` (M45) already exists as the
  exact, targeted escape hatch for the read that shouldn't count as a
  dependency -- the real, scoped gap is that hitting the hazard today
  fails as unbounded recursion instead of a clear, actionable error.
- **Real gap found while designing the fix, checked directly:**
  `batch()`'s own flush loop (`python/tre/__init__.py`, M45) doesn't
  call `signal._notify()` at all -- it iterates subscriber lists and
  invokes callbacks directly (a deliberate M45 fix for cross-signal
  callback dedup). A guard placed only inside the ordinary immediate-
  notify path would never trigger during a batch flush, leaving the
  identical hazard silently reachable there. This shaped the whole
  design: the guard needed to live somewhere both paths actually go
  through, not just the one originally observed.
- `python/tre/__init__.py`: `Signal`/`Computed` already had byte-for-
  byte identical `_subscribe`/`_unsubscribe` and near-identical
  `_notify` bodies (both independently fixed for the same live-
  iteration-during-mutation bug in M45) -- real, pre-existing
  duplication that made "add the guard to both separately" clearly the
  wrong move. Factored a new `_Notifiable` base class: `_subscribers`,
  a new `_notifying` boolean, `_subscribe`/`_unsubscribe` (moved
  verbatim), and `_notify(already_invoked=None)` -- raises a clear
  `RuntimeError` immediately if `self._notifying` is already `True`,
  otherwise sets the flag, walks a snapshot of `_subscribers`, dedupes
  against an optional shared `already_invoked` set, and always clears
  the flag in a `finally`. `Signal(_Notifiable)`/`Computed(_Notifiable)`
  call `super().__init__()` and drop their now-redundant copies.
  `batch()`'s own flush loop drops its duplicated snapshot/dedup logic
  and calls `signal_like._notify(invoked_callbacks)` per pending
  signal instead -- the batch-path gap closed with the *same* guarded
  code the immediate path uses, not a second copy of the fix.
- **Real design question resolved, not assumed:** does `Computed`/
  `Effect`'s own re-tracking (`_recompute`/`_run`) need a *second*,
  matching guard? Re-read both bodies directly: both already
  unsubscribe from their old dependencies *before* running `fn`/
  `self._fn`, and only resubscribe *after* it returns -- so a write
  inside `fn` to one of its own (about-to-be-re-established)
  dependencies can't directly re-invoke that same `_recompute`/`_run`
  through a dependency's own `_notify()`, since it isn't currently
  subscribed to anything at that moment. Confirmed the one guard on
  `_notify` was the whole real gap, not the first of two.
- Real, small correction made while already touching this file: `__all__`
  was missing `Computed`/`Effect`/`batch`/`untrack` since M45 shipped
  them (`from tre import *` never exported any of the four, even though
  direct `from tre import Computed` always worked) -- fixed in the same
  pass, not separately scoped.
- Manual, interactive verification before writing formal tests: ran the
  exact original pathological scenario (a `TriggeringSignal` override
  reading-then-writing `b` while the "text" binding's own recording
  scope is open) -- confirmed the guard now raises one clear
  `RuntimeError` (wrapped once by the Rust-side binding-evaluation
  error path, not dozens of times as before) instead of recursing.
  Separately confirmed the error message's own suggested remedy
  actually works: wrapping the *offending read* (not the write, which
  the first draft of the message incorrectly suggested -- caught by
  actually testing the suggestion before shipping it, since `untrack()`
  only affects dependency *recording*, not notification, so it's
  specifically the read that caused the over-broad subscription that
  needs wrapping) in `untrack()` resolves the hazard with no error and
  correct behavior.
- New pytest tests (`tests/test_reactivity.py`, +6):
  `test_reentrant_notify_raises_a_clear_error_instead_of_recursing`
  (the minimal, isolated proof -- a bare Signal whose own subscriber
  writes back to it, no View/binding machinery); `test_reentrant_
  notify_guard_resets_after_a_caught_exception` (the flag clears via
  `finally`, a later unrelated write still works);
  `test_a_legitimate_non_cyclic_chain_does_not_trip_the_guard` (a real
  Signal -> Computed -> Computed -> Effect chain with no path back
  notifies cleanly -- the guard is for self-reentry, not depth);
  `test_batch_flush_also_hits_the_reentrant_notify_guard` (the real
  proof the batch-path gap is closed); `test_the_original_read_before_
  write_binding_hazard_now_raises_clearly` (the exact, real,
  originally-observed YAML-binding scenario); `test_untrack_is_the_
  real_documented_fix_for_the_read_before_write_hazard` (the
  documented remedy, verified end to end).
- Re-ran the full pre-existing `test_reactivity.py` suite (all 17 M45
  tests): all passed unmodified, confirming the `_Notifiable` refactor
  is a true behavior-preserving no-op.
- Full verification chain, all green: no Rust changes this milestone
  (entirely within `python/tre/__init__.py`), so no `cargo`/`maturin`
  steps were needed; `pytest tests/` (629 passed, up from 623, +6, 1
  skipped, unchanged); all 82 examples run individually with zero
  failures; `demo/showcase.py` (all 5 phases, exit 0).
- `BUILD_TRACKER.md`: new Milestone 46 section, Top Metrics row, "Just
  closed" entry added; `tools/generate_tracker_artifact.py` confirmed
  46 milestones/138 phases/232 items (up from 45/137/231, the correct
  +1/+1/+1 for one new milestone, one phase, one step); Build Tracker
  artifact republished to the existing URL.
- Explicitly out of scope, named: eliminating the over-broad dependency
  attribution itself (the real cause, needs an invasive per-read
  isolation change nothing else calls for); a `Computed`/`Effect`-level
  guard on `_recompute`/`_run` (confirmed unnecessary); a `tesserae`-
  level change (nothing new to propagate -- `Computed`/`Effect`/
  `batch`/`untrack` were already re-exported there since M45's own
  follow-up commit, and this milestone touches only `tre` itself).

**M46 -- Reentrant-Notification Guard for the Read-Before-Write Hazard
-- is now fully complete, single phase.** This closes the milestone --
per the standing "push only after a full milestone closes" convention,
a `git push` is now appropriate.
