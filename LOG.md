# LOG — M42 Phase 1: Show a Single `View` Live

- User's own governing instruction: "I want Tesserae UI Framework to be
  a project built on top of TRE with its own git and GitHub and
  project," resolved through two rounds of `AskUserQuestion` into a
  formal, approved plan (`EnterPlanMode`/`ExitPlanMode`,
  `/home/phil/.claude/plans/reflective-sleeping-falcon.md`) naming this
  real TRE-side gap -- `View` has never been shown in a live window --
  as Part 1, ahead of bootstrapping the separate `tesserae` repo itself
  (Part 2).
- Real investigation before writing any code: read `view.rs`'s own
  module doc comment and `node()`/`_attach()`'s construction sites
  directly, confirming `theme`/`completions` were fresh, private,
  never-shared instances built and discarded per call, and `click()`/
  `hover()`/`right_click()` always laid out with a hardcoded
  `AvailableSpace::MaxContent`.
- **Real, scope-narrowing finding, not assumed from the plan's own
  original guess:** read `App::run`'s own setup step (`app.rs:
  383-402`) directly before designing the new entry point --
  `WindowSetup` is already built generically from any `PyWindow`
  instance's `pub(crate)` fields, via `window.borrow(py)`, with zero
  assumption about how that `PyWindow` was constructed. This meant a
  new `PyWindow`-constructing entry point alone is sufficient; the
  plan's own original text guessed real `app.rs`/`WindowRuntime`
  changes might be needed -- they weren't, confirmed by reading the
  real code rather than trusting the earlier guess.
- `View` (`crates/engine-py/src/view.rs`) gained persistent `pub(crate)
  theme: SharedTheme`/`completions: SharedCompletions`/`width`/`height:
  SharedSize` fields, matching `PyWindow`'s own shape exactly (types
  confirmed via direct grep of `window.rs`/`dispatch.rs` before writing
  any code: `SharedTheme = Rc<RefCell<ThemeState>>`, `SharedSize =
  Rc<Cell<u32>>`, `SharedCompletions = Rc<RefCell<CompletionRegistry>>`).
  `node()`/`_attach()` now clone these shared instances instead of
  building fresh, orphaned ones per call.
- New `available_space()` helper, deliberately placed in a *separate*,
  plain (non-`#[pymethods]`) `impl View` block -- mirrors `PyWindow::
  wrap_node`'s own identical split (`window.rs:259-270`), needed because
  `pyo3`'s `#[pymethods]` macro exposes every fn in that block as a
  Python method, and this one is a genuinely internal helper. Returns
  `MaxContent` on both axes while `width`/`height` are `0` (the real
  default, meaning "never shown live") -- byte-for-byte this struct's
  own pre-M42 behavior -- and `Definite` once a real nonzero size has
  been set. `click()`/`hover()`/`right_click()` now call it instead of
  three separate hardcoded `Size { width: AvailableSpace::MaxContent,
  ... }` literals.
- `PyWindow::from_view(view: &View, width=480, height=200, title="tre
  v2") -> PyWindow`, a new `#[staticmethod]` on `Window` (`window.rs`,
  placed right after `new`): sets `view.width`/`view.height`, then
  clones `view.tree`, `view.reconciler.root()`, `view.handlers`,
  `view.context_menus`, `view.theme`, `view.completions`, and the
  just-set `width`/`height` `Rc<Cell<u32>>` cells directly into the new
  `Window`'s own identically-shaped fields -- the same `Rc`-clone
  pattern `PyWindow::wrap_node` already uses for every `Node` it hands
  out, not a second, parallel tree. `dock`/`materializers`/
  `canvas_draws`/`terminals` start fresh-empty, confirmed safe by
  direct read of `engine-spec::build.rs`: the YAML builder has no
  `Terminal`/`VirtualList`/`Canvas` case, so a View-built tree can never
  need any of them populated. **Real, load-bearing consequence:**
  `view.width`/`height` become the exact same shared `Rc<Cell<u32>>`
  the new `Window`'s own fields hold -- a real live resize writes
  through this one shared cell (mirroring `SharedSize`'s own
  established M33 Phase 2 pattern), so `view.click()`/`hover()`, called
  again after the window is shown, see the window's true current size
  immediately, not a value captured once at `from_view` time.
- New Rust-level tests, directly in `view.rs` (`#[cfg(test)] mod
  tests`): confirmed first that `View::new`'s own body touches no
  `Python<'_>`/`Py<PyAny>` (reads a file, builds a `Tree`/`Reconciler`,
  plain Rust bookkeeping only) -- callable directly with no GIL, no
  `pyo3::prepare_freethreaded_python()`, the same real "plain-Rust logic
  gets a Rust test" split this crate has implicitly followed the whole
  project (zero prior `Python::with_gil` usage found anywhere in
  `engine-py`'s own existing test surface, confirmed via grep before
  choosing this approach over a pytest-only one). Three tests: a
  never-shown `View` still lays out `MaxContent`/`MaxContent`; setting a
  real size switches to `Definite`/`Definite`; only one axis set still
  falls back to `MaxContent` on *both* (no mixed-mode edge case).
- New pytest tests (`tests/test_view_in_window.py`, 5 new): the real,
  decisive proof of shared state is `window.click(node)` -- a
  `Window`-side method, never `view.click(node)` -- reaching the
  handler `View._attach` wired onto `view.handlers`, only possible if
  `Window.from_view` genuinely shares the live tree/root/handlers, not
  fresh copies; `view.click()` still works standalone after `from_view`
  switched it to `Definite` layout (regression safety against the
  original headless-click proof in `test_view_handlers.py`); a `Signal`
  write still reaches the live tree post-`from_view`; two independent
  `View`s get independent, non-cross-wired size cells. Deliberately no
  `App.run()` call in this file -- a second real `App().run()` inside
  the same pytest process has broken an unrelated, earlier-passing
  real render-loop test before (the reason `test_tracing.py` already
  subprocesses its own real run instead).
- New example `examples/live_view.py` + `live_view.yaml`: a real
  `Signal`-bound counter (`CounterVM`), five real `Tree::dispatch`
  press+release pairs against the live-shown `View`'s own `button`
  node, an asserted real text-binding repaint on `label` (`Count: 0` ->
  `Count: 5`), then a genuine 30-frame `App.run()` -- the one real,
  full end-to-end proof this phase set out to build, run and passing:
  "live_view.py: exited cleanly after a real 30-frame render loop with
  a live View."
- `.pyi` stub (`python/tre/_core.pyi`) updated in the same step:
  `Window.from_view` added (full docstring, matching the real
  implementation's own stated consequences); `View`'s own class
  docstring mentions the new live-window path.
- `BUILD_TRACKER.md`: Phase 1 flipped to ✅ with the full real
  investigation/implementation/verification writeup; Top Metrics row
  updated to 50%/"Phase 1 done, Phase 2 in progress"; a new "Just
  closed" pointer added above M41's own. Parser confirmed balanced (42
  milestones, 133 phases, 226 items, unchanged -- a status flip on an
  already-scoped phase, no new phase/item added); artifact regenerated
  and republished.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (engine-py's own lib suite +3, every other crate's count unchanged);
  `maturin develop --release` rebuilt; `pytest tests/` (586 passed, +5,
  1 skipped, unchanged); all 78 examples (+1, `live_view.py`) and the
  showcase demo run clean.

**M42 Phase 1 -- Show a Single `View` Live -- is complete.** Phase 2 (a
shared, swappable `(Tree, root)` cell letting a live window switch
which `View` it shows without closing/reopening, per the user's own
explicit plan-review feedback about `app.py`/`App.show(name)`) remains
open -- the milestone as a whole is not yet closed, so no push yet per
the standing "push only after a full milestone closes" convention.
