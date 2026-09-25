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

4. Phase 2 done:
   - `create` for every kind; `set`/`get` for every property -- layout,
     transform, visibility, z-order, text, text input, image, canvas,
     scroll view, virtual list, terminal -- and the `layout_*` reads.
   - A virtual list's `materialize` returns the framework's own node;
     `tre` keeps the visible rows built at every layout.
   - Spec corrections: `window.batch()` and `window.set_content` dropped.
     A `set` never runs layout (0.4 µs; a 500-node layout pass is
     0.09 ms), and the structure methods show any screen.
   - Found and fixed: a real mouse wheel scrolled backwards.
   - pytest 1116 passed; cargo release 586 passed; 89 examples and the
     showcase clean.

5. Phase 3 done:
   - `window.measure_text(...)` on the per-thread shaper, laid out by the
     painter's own code, so a measurement matches what's painted.
   - `font_style`, `letter_spacing`, `wrap`, `max_lines`, and `overflow`
     on text nodes: ellipsis by binary search per cut line, clipping
     otherwise, synthesized italics.
   - pytest 1129 passed; cargo release 590 passed; 89 examples and the
     showcase clean.

## Status

**M96 Phases 1-3 done.** Phase 4 (layers) next.
