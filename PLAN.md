# PLAN — M42 Phase 1: Show a Single `View` Live

## Goal
Wire `View` (the declarative YAML + `ViewModel` layer, §16.2) into a
real, on-screen `Window` for the first time -- every existing
`View`-using example is headless. Part 1 of the user's approved
Tesserae bootstrap plan (`/home/phil/.claude/plans/reflective-sleeping-falcon.md`).

## Steps
1. Investigated `view.rs`'s own current shape before touching anything:
   `node()`/`_attach()`'s handler-wiring each built a fresh, private
   `ThemeState::default()`/`CompletionRegistry::new()` per call
   (`view.rs:438-442, 554-555` before this phase); `click()`/`hover()`/
   `right_click()` laid out with a hardcoded `AvailableSpace::
   MaxContent` on both axes.
2. Read `App::run`'s own setup step (`app.rs:383-402`) directly before
   designing the new entry point -- **real, scope-narrowing finding:**
   `WindowSetup` is already built generically from any `PyWindow`
   instance's `pub(crate)` fields, with zero assumption about how that
   `PyWindow` was constructed. This means a `PyWindow`-constructing
   entry point needs no changes anywhere in `app.rs`/`WindowRuntime` to
   work with the existing `App.add_window()`/`App.run()` path.
3. Gave `View` persistent `pub(crate) theme: SharedTheme`/
   `completions: SharedCompletions`/`width`/`height: SharedSize` fields
   (matching `PyWindow`'s own shape exactly), initialized once in
   `View::new`. `node()`/`_attach()` now clone these shared instances
   instead of building fresh ones.
4. Added a new, non-`#[pymethods]` `available_space()` helper (a
   separate plain `impl View` block, mirroring `PyWindow::wrap_node`'s
   own identical split so it isn't accidentally exposed as a Python
   method) -- `MaxContent` on both axes while `width`/`height` are `0`
   (the real default, "never shown live"), `Definite` once a real size
   has been set. `click()`/`hover()`/`right_click()` now call it instead
   of a hardcoded literal.
5. Added `PyWindow::from_view(view: &View, width=480, height=200,
   title="tre v2") -> PyWindow`, a `#[staticmethod]` on `Window`
   (`window.rs`) -- sets `view.width`/`height`, then clones `view.tree`,
   `view.reconciler.root()`, `handlers`, `context_menus`, `theme`,
   `completions`, and the just-set `width`/`height` cells directly into
   the new `Window`'s own fields. `dock`/`materializers`/`canvas_draws`/
   `terminals` start fresh-empty -- confirmed safe: `engine-spec`'s YAML
   builder has no `Terminal`/`VirtualList`/`Canvas` case.
6. New Rust unit tests directly in `view.rs` (`#[cfg(test)] mod tests`)
   proving `available_space()`'s three real branches -- constructed via
   `View::new()` called directly with no GIL/interpreter, since its body
   never touches `Python<'_>`/`Py<PyAny>` (confirmed by reading it before
   writing the tests) -- the same real "plain-Rust logic gets a Rust
   test" split this crate has always implicitly followed (zero prior
   `Python::with_gil` usage anywhere in `engine-py`'s own test surface).
7. New pytest tests (`tests/test_view_in_window.py`): a click dispatched
   through the *Window* (not the View) reaches a handler `View._attach`
   wired -- the real, decisive proof of shared tree/root/handlers;
   `view.click()` still works standalone post-`from_view`; a `Signal`
   write still reaches the live tree; two independent `View`s get
   independent, non-cross-wired size cells. No test calls `App.run()`
   -- a second real `App().run()` in the same pytest process has broken
   an unrelated test before (`test_tracing.py`'s own subprocess
   convention is the established workaround, not needed for the
   run-loop-free coverage this phase needed).
8. New example `examples/live_view.py` + `live_view.yaml`: a real
   `Signal`-bound counter, five real dispatched clicks against the
   live-shown `View`, an asserted real text-binding repaint, then a
   genuine 30-frame `App.run()` -- the one real, full end-to-end proof.
9. `.pyi` stub updated: `Window.from_view` added, `View`'s own class
   docstring updated to mention it.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt`, `cargo test --workspace --release` (+3, all else
unchanged), `maturin develop --release`, `pytest tests/` (586 passed,
+5, 1 skipped unchanged), all 78 examples (+1, `live_view.py`), showcase
demo. **M42 Phase 1 -- Show a Single `View` Live -- is complete.** Phase
2 (a shared, swappable `(Tree, root)` cell letting a live window switch
which `View` it shows without closing/reopening) remains open.
