# Plan: M3 Phase 4, Step 6 — Wire `engine-py` Minimal (§14 step 6)

Corresponds to `BUILD_TRACKER.md` M3 Phase 4, step 6 of 2 (steps 6-7).

## Goal

Per §14 step 6: "Wire `engine-py`: expose node creation + one property
setter to Python; drive step 2's animation from a `.py` script."

## Scope

In scope:
- `engine-py::App`: `new(width, height)`, `add_rect(background, width,
  height) -> Node` (node creation), `run(max_frames=None)` (the one
  blocking call, Design Principle 1 -- opens a real window, ticks/
  lays-out/renders every frame via the exact same `engine-render`
  pipeline every prior step's Rust demo already used).
- `engine-py::Node`: `animate(property, to, duration_ms=0)` -- the one
  property setter, two-level dispatch per §8's review note
  (`PaintProperties` fields first: `opacity`/`corner_radius`/
  `elevation`/`background`; `NodeKind` payload fields second, currently
  always empty since `TextState` has no `Animated` fields yet).
- `engine-py::EngineError` (§8's exact design, minus `CycleRejected` --
  no `add_child` yet) + `From<EngineError> for PyErr`.
- `pyproject.toml` + `python/tre/__init__.py`, matching §12's exact
  sketch. A real `examples/animate_rect.py` driving step 2's animation
  from Python, and `tests/test_engine_py.py` (pytest) covering node
  creation, all four animatable properties, and both `EngineError` paths.
- CI: per §13's "Decision recorded" (primary-OS CI starts at step 6,
  not deferred) -- `maturin develop` + `import tre` + the pytest suite +
  the real demo script, added to the existing CI job.

Out of scope (each additive at its own later build-order step): `PyWindow`
(multi-window is step 14), `set_on_click`/`#[pyclass(gc)]` (nothing
stores a Python callback yet), `add_child` (no tree mutation from Python
beyond `add_rect` yet), `Python::detach` around the render loop (real
finding: requires `Send`, `Rc<RefCell<Tree>>` is deliberately `!Send`,
§9 -- revisit only if a second thread ever contends for the GIL),
`BindingResolver`/MVVM (step 12), `accesskit` (step 7, next).

## Verification

`cargo test --workspace`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo fmt --check` all clean. `maturin develop` +
`python -c "import tre"` + `pytest tests/` + `python
examples/animate_rect.py` all succeed for real, locally, against the
compiled extension -- not just "compiles."
