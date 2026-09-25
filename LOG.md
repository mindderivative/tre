# LOG — Branch `0.3.4`: Milestone 96

- User-directed: "Push, then start M96."

## Done

1. `0.3.4` pushed to origin (M95 and the M94 focus follow-up).
2. M96 scoped against the source. Findings and the five-phase plan are
   in `PLAN.md`; the steps are in `BUILD_TRACKER.md`. The two items
   likely to leave the spec, `window.batch()` and `window.set_content`,
   are measured and decided in Phase 2, not assumed.

3. Phase 1 done:
   - `insert_child`, `children`, `parent`, `remove()` detaching (R5), and
     `destroy()`. A move keeps identity, listeners, focus, and running
     animations.
   - Issue #10: every `tre` object is a `Send + Sync` shell around a
     thread-checked state; an off-thread drop is finished on the owning
     thread. Use off-thread still raises. Spec corrected: `destroy()` is
     thread-bound like every other method.
   - Detached nodes from `create`/`remove()` are freed with their last
     handle, listeners too.
   - `window.advance(ms)`, a per-window pinned clock.
   - pytest 1082 passed; cargo release 582 passed; 89 examples and the
     showcase clean.

## Status

**M96 Phase 1 done.** Phase 2 (creation and properties) next.
