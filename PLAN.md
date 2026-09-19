# PLAN — M39 Phase 5: `Tree::tick_all` Active-Set Optimization (closes M39)

## Goal
Answer the phase's own scoping question honestly: is `tick_all`'s
naive whole-tree walk a real, load-bearing per-frame cost at this
catalog's own realistic node counts, or a real-but-negligible one --
measured first, per M34 Phase 1's own precedent, before designing
anything.

## Steps
1. A real, throwaway scratch benchmark (`crates/engine-core/tests/
   scratch_tick_all_bench.rs`, written, run, then deleted -- never
   committed, mirroring M34 Phase 1's own identical "scratch, not
   permanent" methodology): three scenarios --
   - 2000-node idle tree (every 5th node opted into `InteractionState`,
     a real clickable-row proportion, nothing actively animating):
     ~32µs/call.
   - The identical scene with 1% of nodes carrying a real, live
     `opacity` animation: ~31-45µs/call across repeated runs (within
     normal noise of the idle case) -- confirms `Animated::tick`'s own
     per-field early return already makes the *animation* work cheap;
     the real cost is the per-node walk itself, largely independent of
     how much is actually animating.
   - A 10x-scale sanity check (20,000 nodes, an unrealistic single-
     screen count for this catalog): ~1.09ms/call -- confirms linear
     scaling (no hidden quadratic blowup), still only ~6.5% of a
     16.67ms/frame budget even there.
2. Real, honest conclusion: the numbers do not justify building the
   active-set mechanism. At this catalog's own realistic node counts,
   `tick_all`'s real cost is under 0.3% of frame budget -- an even
   smaller real cost than M34 Phase 1's own "modest, not dramatic"
   tessellation-caching win. A new `active: HashSet<NodeId>` (touching
   every animation-starting call site with a new "never forget to
   update it" correctness invariant) would trade real, ongoing
   maintenance risk for an imperceptible per-frame saving.
3. No production code changed -- this phase's own real deliverable is
   the investigation and its documented, measured conclusion, matching
   this whole project's own "measured, not assumed" discipline: a
   benchmark validly concluding "no action needed" is a complete
   result, not a placeholder for future work.
4. `BUILD_TRACKER.md` Phase 5 flipped to done with the full real
   measurement writeup; milestone status line flipped to "✅ Complete
   — all 5 phases done"; Top Metrics row to 100%; a new "Just closed"
   trailer added for M39's own closure, above the pre-existing M38
   one. Artifact regenerated (39/127/218, unchanged) and republished.

## Status
Complete. **M39 -- Hardening III: Second Follow-Up Gap Sweep -- is now
fully complete, all 5 phases done:** Code Editor Horizontal Scroll;
Loading Indicator + Time Picker Dial; Shape-Morphed Border Inset Fix;
Terminal Cell Text Attributes; `Tree::tick_all` Active-Set
Optimization (this phase, concluding no code change was warranted).
This closes the whole milestone -- the real "push after every
milestone closes" convention now applies.
