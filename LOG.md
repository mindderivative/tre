# Log: M11 Phase 2 — Canvas Redraw Triggering: Investigate and Resolve (§11.10, §11.11)

Corresponds to `BUILD_TRACKER.md` M11 Phase 2, closing M11 entirely.
Investigates whether `Window.redraw_canvas`'s explicit, app-triggered-
only design is a real gap or a deliberate, correct choice — the open
question this milestone's own scoping deliberately left unresolved
rather than presupposing an answer.

## Investigation

`crates/engine-py/src/app.rs`'s real winit frame loop was checked
directly: `canvas_draws` (the `HashMap<NodeId, Py<PyAny>>` backing
`redraw_canvas`) never appears anywhere in `app.rs` — confirmed via
grep. It isn't part of `WindowSetup`/`WindowRuntime`, the two structs
carrying everything the per-frame closures can see; the per-frame
closure only calls `Tree::tick_all` and drains completions. This is
structural, not an oversight: `WindowSetup`'s own module doc comment
already states the real reason a Python-callback field gets threaded
into the frame loop at all — `handlers` is invoked only on a real
dispatched activation, `completions` only on a real animation actually
completing. Every existing case is event-driven; none is unconditional
per-frame work. `engine_core::canvas`'s own module doc comment already
correctly documents the current design without caveat ("the actual
Python 'draw callback' is invoked exactly once by ... `redraw_canvas`,
at an app-triggered sync point"). `ARCHITECTURE.md` (§11.10, §11.11,
and a full grep for "redraw"/"Canvas") specifies no requirement for
automatic per-frame redraw anywhere.

Real cost analysis: unconditional per-frame redraw would mean, every
frame, for every registered canvas, a Python call across the GIL, a
fresh `CanvasContext`, the app's own draw closure re-running in full,
and a `Vec<DrawCommand>` rebuild plus `Tree::set_canvas_content` — real
cost for content that, in the common case, hasn't changed since the
last real interaction. This is exactly the class of per-frame overhead
this codebase has consistently designed against elsewhere.

## Resolution

**No change needed.** `redraw_canvas`'s explicit, app-triggered-only
design is confirmed deliberate and correct for this framework's
retained-mode `Tree` model, not a gap: the frame loop's own
architecture keeps Python-callback invocation narrow and event-driven
by design; `ARCHITECTURE.md` names no automatic-redraw requirement;
and unconditional per-frame invocation would impose real, avoidable
cost for the common unchanged-content case. This mirrors M7 Phase 5
Step 1's own honest "real finding: already existed, nothing new needed
building" resolution shape.

`BUILD_TRACKER.md`'s M5 carried-forward "known gap" note, which had
stated the fact without resolving it, is corrected to record this
resolution — closing the question so it doesn't get re-surfaced as
open in a future milestone's own scoping.

No production code change this phase. `cargo test --workspace
--release`/clippy `-D warnings`/fmt all re-confirmed clean as a
formality; `pytest` (96 passed, 1 skipped, unchanged) and all sixteen
examples re-confirmed clean, verifying the zero-code-change claim is
actually true.

M11 (Canvas Authoring Completeness) is now complete: both phases done
— Phase 1 shipped the real capability (curve authoring), Phase 2
investigated and resolved the milestone's own remaining open question
with no code change required.
