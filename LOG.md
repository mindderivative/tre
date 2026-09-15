# Log: M3 Phase 4, Step 6 — Wire `engine-py` Minimal (§14 step 6)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 4, step 6 of 2 (steps 6-7).

## What happened

**Verified pyo3's real current API before writing anything** (§15 Risk
Register's own general caution, already borne out twice for `accesskit`
and `parley`-family crates): `cargo add pyo3 --dry-run` resolved
`0.29.2`, not whatever an older training-data assumption might expect.
Checked its source directly and found a real, load-bearing rename:
`Python::with_gil`/`allow_threads` (ARCHITECTURE.md §8/§9's own sketch
and prose) are now `Python::attach`/`Python::detach` -- confirmed in
`marker.rs`, no deprecated alias left. Every `#[pyclass]`/`#[pymethods]`/
`#[pymodule]` example in this step uses the real 0.29.2 shapes
(`Bound<'_, PyAny>` extraction, `#[pymodule] fn _core(m: &Bound<'_,
PyModule>)`), checked against pyo3's own bundled doc examples, not
assumed.

**Second real finding: `Python::detach` requires `Ungil`, which requires
`Send` on stable Rust** (`unsafe impl<T: Send> Ungil for T {}`, checked
directly in `marker.rs`). §9's own text calls wrapping frame work in
`allow_threads`/`detach` "a no-op-cost safety habit... not a live
requirement" given `Tree`'s `Rc<RefCell<>>` design -- but `Rc<RefCell<
Tree>>` is `!Send`, so that habit isn't actually free to apply here: it
would need either an unsafe manual GIL release bypassing `Ungil`'s
safety guarantee, or migrating `Tree` off `Rc<RefCell<>>` (which §9
explicitly chose not to pay for speculatively). Skipped `detach()`
entirely for this step, documented why in `app.rs`'s own module doc
comment, matching §9's own stated reasoning (no second thread actually
contends for the GIL in this step's scope) rather than contradicting it.

**`App`/`Node` are a deliberately narrower slice than §8's full `PyApp`/
`PyWindow`/`PyNode` split:** one implicit window inside `App` itself
(the `PyWindow` split exists for step 14's multi-window model, not this
one), no `add_child`/`set_on_click`/`#[pyclass(gc)]` (nothing needs tree
mutation or a stored callback yet). `Node::animate` implements the real
two-level dispatch §8's review note describes (`PaintProperties` fields,
then `NodeKind` payload fields) even though the second level is
currently always empty (`TextState` has no `Animated` fields, §14 step
4) -- the shape is right, not padded ahead of need.

**Built and ran it for real, not just `cargo build`:** `maturin develop`
installed the compiled extension into a local `.venv`; `python -c
"import tre"` succeeded; the real pytest suite (`tests/test_engine_py.py`,
6 tests: node creation, all four animatable properties, the instant-snap
default, both `EngineError` paths with their exact messages) passed
against the actual extension. `examples/animate_rect.py` -- two rects
created from Python, three `animate()` calls (opacity, corner_radius,
background) -- ran a real 60-frame windowed render loop end to end and
exited cleanly.

**Third real finding, caught by testing the graceful-exit path
specifically, not just the happy path:** `App.run()` only handled "no
GPU adapter" gracefully (exit the process, TRE v1 finding #261's
convention); "no display reachable" (`EventLoopError` from
`run_windowed`) fell through to `result.map_err(...)`, raising a Python
exception instead of exiting cleanly -- inconsistent with every other
entry point in this workspace. Fixed: any `run_windowed` failure now
prints a message and returns `Ok(())`, matching the convention exactly
(this is also what makes the CI Python smoke test below safe to run on
a genuinely headless runner without asserting anything about real
rendering).

**CI, per ARCHITECTURE.md §13's explicit "Decision recorded": primary-OS
CI starts as soon as `engine-py` exists, this step, not deferred.**
Added to the existing job (not a second Rust build): `actions/
setup-python@v5`, `pip install maturin pytest`, `maturin develop`,
`import tre`, the pytest suite, and the real demo script.

## Verification

```
$ cargo build -p engine-py   # clean, second real attempt (first was
                              # missing direct deps on peniko/taffy/wgpu/
                              # vello_hybrid/winit/pollster -- Rust
                              # doesn't re-export a dependency's own
                              # dependencies)
$ cargo test --workspace     # all green
$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check          # clean

$ source .venv/bin/activate && maturin develop
🛠 Installed tre-0.1.0
$ python -c "import tre; print(tre.App, tre.Node)"
<class 'builtins.App'> <class 'builtins.Node'>
$ python -m pytest tests/test_engine_py.py -v
6 passed in 0.02s
$ python examples/animate_rect.py
animate_rect.py: exited cleanly after 60 frames
```

## Next

`BUILD_TRACKER.md` updated: Phase 4 step 6 of 2 done, M3 to ~46%. Phase 4
itself stays 🚧 (matching Phase 2's own step-by-step commit pattern) until
step 7 closes it. Next: step 7 -- wire `accesskit`, confirm one button is
correctly exposed to a screen reader. ARCHITECTURE.md's own review note
warns this crate has had real breaking API changes across the version
range covered during TRE v1's own maintenance -- pin an exact version and
re-verify its real current API directly, the same discipline just applied
to pyo3 above.
