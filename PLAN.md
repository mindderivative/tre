# Plan: M11 Phase 2 — Canvas Redraw Triggering: Investigate and Resolve (§11.10, §11.11)

Corresponds to `BUILD_TRACKER.md` M11 Phase 2's own scoping: determine
whether `Window.redraw_canvas`'s explicit, app-triggered-only design
is a real gap or a deliberate, correct choice for this framework's
retained-mode `Tree` model, closing the milestone either way.

## Investigation

- `crates/engine-py/src/app.rs` (`App::run`'s own real winit frame
  loop): confirmed via grep, `canvas_draws` (the `HashMap<NodeId,
  Py<PyAny>>` `redraw_canvas` reads from, defined on `PyWindow` in
  `window.rs`) never appears anywhere in `app.rs` — it is not part of
  `WindowSetup`/`WindowRuntime`, the two structs that carry everything
  the real per-frame closures can see. The per-frame closure only ever
  calls `Tree::tick_all` (animation ticking) and drains completions —
  it has no way to reach a registered draw callback today, structurally,
  not by oversight.
- `WindowSetup`'s own module doc comment (`app.rs:47-56`, confirmed by
  direct read) already states the real, deliberate reason: "`handlers`
  is the one exception: `Node.set_on_click`'s own real `Py<PyAny>`
  callbacks... have to be looked up by the `on_input` closure below on
  a real activation, so this is the one Python-object-bearing field
  extracted here rather than converted to plain data." Every
  Python-callback-invoking field this codebase threads into the frame
  loop (`handlers`, `completions`) is invoked only in response to a
  real, specific event (a dispatched activation, an animation actually
  completing) — never unconditionally, every frame, regardless of
  whether anything changed. Adding `canvas_draws` to that set and
  calling every registered canvas's own draw callback unconditionally
  every frame would be the *first* case of blanket per-frame Python
  invocation anywhere in this codebase — a real, structural precedent
  break, not a small addition.
- `engine_core::canvas`'s own module doc comment (confirmed accurate by
  direct read, `crates/engine-core/src/canvas.rs:1-8`) already
  documents the current design correctly and without caveat: "the
  actual Python 'draw callback' is invoked exactly once by `engine-py::
  Window.redraw_canvas`, at an app-triggered sync point outside both
  paint and hit-testing, and only its *result* -- this module's types
  -- ever reaches `Tree`." This is a description of the real,
  intentional architecture, not a stale claim needing correction.
- `ARCHITECTURE.md` was checked directly (§11.10, §11.11, and a full
  grep for "redraw"/"Canvas") — nothing there specifies or requires
  automatic per-frame canvas redraw. The historical "known gap" note
  this milestone's own scoping paragraph inherited (`BUILD_TRACKER.md`
  M5's own carried-forward gap list) stated the fact ("`Window.
  redraw_canvas` is explicitly app-triggered, not automatic every
  frame") without asserting it was wrong — this phase is the first time
  that fact has actually been investigated for whether it's a defect.
- Real cost analysis of the alternative: unconditional per-frame redraw
  would mean, every single frame, for every registered `Canvas` node:
  a Python call across the GIL, a fresh `CanvasContext` allocation, the
  app's own draw closure re-running its full drawing logic, and a
  `Vec<DrawCommand>` rebuild plus `Tree::set_canvas_content` — real,
  non-trivial per-frame cost for canvas content that, in the common
  case (a static chart, a node-graph that hasn't changed since the last
  interaction), hasn't changed at all. This is exactly the class of
  "hot per-frame path stays lean" cost this codebase has consistently
  avoided elsewhere (`engine-render` never depending on `engine-md3`;
  `paint_node`'s own child-recursion; the M8 culling milestone's entire
  premise). The current "invalidate, then the app explicitly redraws
  when it knows something changed" model is the standard, correct
  pattern for a retained-mode scene graph (the same reason a game
  engine's own "mark dirty" flag exists) — not a gap unique to this
  framework.

## Resolution

**No change needed.** `Window.redraw_canvas`'s explicit, app-triggered-
only design is confirmed to be the deliberate, correct choice for this
framework's retained-mode `Tree` model, not a real gap:

1. The frame loop's own architecture (`WindowSetup`/`WindowRuntime`)
   deliberately keeps Python-callback invocation narrow and
   event-driven (`handlers` on real activation, `completions` on real
   animation completion) — never unconditional per-frame work.
2. `ARCHITECTURE.md` names no requirement for automatic redraw.
3. Unconditional per-frame invocation would impose real, avoidable cost
   for the common case (unchanged canvas content), the exact class of
   per-frame overhead this codebase has consistently designed against
   elsewhere.

This closes M11 Phase 2 and the milestone with a stated, justified "no
change needed" rather than manufacturing a fix for something that
isn't broken — matching the same honest resolution shape M7 Phase 5
Step 1 used ("real finding: this already existed... nothing new needed
building").

The stale "known gap" framing this milestone's own scoping paragraph
and `BUILD_TRACKER.md`'s M5 carried-forward gap note both inherited is
corrected to state the resolution, so this doesn't get re-surfaced as
an open question in a future milestone's own scoping.

## Verification plan

No production code change this phase — a pure investigation-and-
documentation phase. Re-run `cargo test --workspace --release`/clippy/
fmt and full `pytest tests/` + all sixteen examples as a formality,
confirming the zero-code-change claim is actually true (nothing
regressed because nothing changed).
