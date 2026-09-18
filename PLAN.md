# Plan: M27 Phase 3 — Motion & Custom Drawing Screen

Corresponds to `BUILD_TRACKER.md` M27 Phase 3. Written retroactively
alongside implementation — see `LOG.md` and `BUILD_TRACKER.md`'s own
Phase 3 entry for the complete real investigation, findings, and
verification record.

## What changed

- `demo/showcase.py`'s "motion" placeholder replaced with
  `build_motion_screen`: four real animation triggers (opacity/
  corner_radius/elevation/shape) on a shared card, each fired by a
  real click; a live `Canvas` node graph (reusing `examples/
  node_graph.py`'s own real drawing/hit-test pattern) with its own
  real `transform` pan/zoom (reusing `examples/pan_zoom.py`'s own
  mechanism, applied to a `Canvas` for the first time).
- Real, connected fix: `materializers`/`canvas_draws`
  (`crates/engine-py/src/window.rs`) wrapped in their own `RefCell`,
  letting `add_virtual_list`/`add_canvas` become `&self` like every
  other `add_*` method — closes the exact residual re-entrancy
  limitation Phase 1's own log predicted, hit for real this phase (a
  nav-click handler building a `Canvas` panicked with `RuntimeError:
  Already borrowed` before this fix).
- Two more real bugs fixed after actually running it: a Tab-order
  count assumption broken by reordering the nav-button check earlier,
  and a wrong assumption about `set_hit_test_circle` replacing (not
  narrowing) the canvas's default hit test.

See `LOG.md` for the full narrative and verification results.
