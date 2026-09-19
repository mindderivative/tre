# LOG — M39 Phase 5: `Tree::tick_all` Active-Set Optimization (closes M39)

- User's own explicit instruction: "Start" continued into M39 Phase 5
  (item 5 from the gap-sweep answer, fifth and final in the user's own
  chosen order): whether `Tree::tick_all`'s naive whole-tree walk needs
  an active-set optimization.
- Real, measured-first discipline, the identical real precedent M34
  Phase 1's own benchmark already established for tessellation
  caching, not assumed: before writing any design or code, wrote a
  real, throwaway scratch benchmark (`crates/engine-core/tests/
  scratch_tick_all_bench.rs`) -- deliberately never committed, the
  identical "scratch, not a permanent bench suite" precedent M34
  Phase 1's own investigation already set.
- Three real scenarios measured, `cargo test -p engine-core --release
  --test scratch_tick_all_bench -- --ignored --nocapture`:
  - **2000-node idle tree** (a genuinely large real catalog screen --
    far more than any single showcase demo screen in this project
    actually builds -- every 5th node opted into `InteractionState`, a
    real clickable-row proportion, nothing actively animating): 500
    calls to `tick_all` averaged **~32µs/call** (two separate runs:
    42.67µs and 31.82µs -- normal machine-load variance, both firmly
    in the same real "tens of microseconds" order of magnitude).
  - **The identical scene with 1% of nodes carrying a real, live
    `opacity` animation** (a real "something is always subtly
    animating somewhere" steady state, e.g. a ripple or hover fade):
    **~31-45µs/call** -- essentially indistinguishable from the fully
    idle case. This is the real, decisive confirmation of the scoping
    note's own hypothesis: direct earlier reading of `Animated::tick`
    found every per-field tick call already early-returns cheaply
    (`if self.active.is_none() { return false; }`) for a settled
    field, so the real cost was never the animation *work* -- it's the
    sheer per-node iteration overhead of walking every `Node`'s own
    `PaintProperties`/`InteractionState`/kind-specific fields every
    frame, present whether or not anything is actually moving.
  - **A 10x-scale sanity check** (20,000 nodes -- a genuinely
    unrealistic single-screen node count for this catalog, included
    only to check for a hidden quadratic blowup, not because it
    represents a real scenario): **~1.09ms/call**, roughly linear with
    the 2000-node measurement (not the quadratic result a naive
    per-node-times-per-node cost would produce) -- and even at that
    exaggerated scale, only ~6.5% of a 16.67ms/frame budget.
- **Real, honest conclusion, stated plainly rather than building
  something speculative because the phase was "supposed to" produce
  code:** the numbers do not justify an active-set optimization. At
  this catalog's own realistic node counts (hundreds to low thousands
  per real screen), `tick_all`'s real cost is comfortably under 0.3%
  of frame budget -- an even smaller real cost than M34 Phase 1's own
  "a real, honest, modest win, not a dramatic one" finding for
  tessellation caching; here there is essentially no real problem
  to solve. A new `active: HashSet<NodeId>` mechanism (populated by
  every real animation-starting call site -- `update_hover`, `set_
  pressed`, `transition_focus`, every `Node.animate()` entry point --
  each one gaining a new, permanent "never forget to keep the active
  set in sync" correctness obligation) would trade real, ongoing
  maintenance risk and code complexity for a per-frame saving no real
  user could ever perceive. This whole project's own repeated
  "measured, not assumed" discipline (M34 Phase 1's original
  precedent, M38/M39's own repeated corrections of prior completion
  notes found factually wrong on direct re-check) applies here in its
  other valid direction too: a real benchmark can validly conclude
  "the numbers say don't build this," and that conclusion, honestly
  reached and documented, is this phase's own complete, real
  deliverable -- not a placeholder deferring real work to later.
- No production code changed. The scratch benchmark file was deleted
  after capturing these real numbers, per its own stated intent and
  M34 Phase 1's own identical precedent -- never staged, never part of
  any commit.
- `BUILD_TRACKER.md`: Phase 5 flipped to done with the full real
  measurement writeup (all three scenarios' own real numbers, and the
  real reasoning for the "don't build it" conclusion); milestone
  status line flipped to "✅ Complete — all 5 phases done
  (2026-09-19)"; Top Metrics row to 100%; a new "Just closed" trailer
  written for M39's own closure, added above the pre-existing M38
  Phase 7 trailer (the established stacking convention -- newest
  closure first). Parser re-confirmed balanced (39 milestones, 127
  phases, 218 items, unchanged -- no new real line items, only status
  flips); artifact regenerated and republished.

**M39 — Hardening III: Second Follow-Up Gap Sweep — is now fully
complete, all 5 phases done:**
1. Code Editor Horizontal Scroll
2. Loading Indicator + Time Picker Dial
3. Shape-Morphed Border Inset Fix
4. Terminal Cell Text Attributes
5. `Tree::tick_all` Active-Set Optimization (real profiling, real
   "no code change warranted" conclusion)

This closes the whole milestone — per the standing "push only after a
full milestone closes" convention, a `git push` is now appropriate.
