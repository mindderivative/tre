# LOG — M42 Phase 2: Swap Which `View` a Live Window Shows

- User's own governing instruction: the approved Tesserae bootstrap plan
  (`/home/phil/.claude/plans/reflective-sleeping-falcon.md`), Part 1,
  Phase 2 -- itself scoped in response to the user's own explicit
  plan-review feedback on the first `ExitPlanMode` attempt: "`app.py`
  should be the entry point for the app... this allows for switching of
  current views without needing to bootstrap each view/viewModel."
- Re-read the plan's own original design text before writing any code:
  "a new shared type wrapping `Rc<RefCell<(Rc<RefCell<Tree>>, NodeId)>>`
  ... `Window.show_view(view: &View)` writes the target View's own
  `(tree.clone(), reconciler.root())` into that shared cell directly."
- **Real, load-bearing correctness finding, made during implementation,
  not covered by that original text:** `engine_core::NodeId`
  (`crates/engine-core/src/node.rs`) is a `slotmap` generational key,
  unique only *within* the `Tree` that allocated it -- two independent
  `View`s' own root nodes can (and, confirmed by how `slotmap` allocates
  keys, routinely do) collide on the identical raw value.
  `HandlerMap`/`context_menus` are keyed by `(NodeId, EventKind)`/
  `NodeId` alone, with no per-`Tree` namespacing at all -- sharing one
  persistent map across a `show_view` switch would silently cross-wire
  a different `View`'s old callback onto a colliding `NodeId` in the new
  one. Widened the real swap bundle to `tree`+`root`+`handlers`+
  `context_menus` together (a new `window::ActiveTree`/
  `SharedActiveTree = Rc<RefCell<ActiveTree>>`, `window.rs`), not just
  the `(Tree, NodeId)` pair the plan's own text named.
  `theme`/`completions` deliberately stay outside this bundle: `ThemeState`
  isn't keyed by `NodeId` at all (a single scheme+dark flag, resolved
  uniformly); `CompletionRegistry` is keyed by its own private
  monotonic `next_id` counter, not `NodeId` -- no cross-`Tree` collision
  risk for either, and Phase 1 already named live theme-switching and
  `on_complete` callbacks on View-sourced nodes as real, separate,
  stated gaps this milestone doesn't fix.
- **Real, scope-narrowing design choice, confirmed by grep before
  implementing anything:** the naive reading of the plan's own text
  ("wrap `tree`/`root` in the shared cell") would mean changing
  `PyWindow`/`WindowSetup`/`WindowRuntime`'s own `tree`/`root`/
  `handlers`/`context_menus` field *types* -- grepped and counted ~230
  pre-existing call sites across `window_factory.rs`/`window_input.rs`/
  `window_docking.rs`/`window_virtual_canvas.rs`/`app.rs` that read
  those fields directly, almost all in `add_*` imperative factory
  methods completely unrelated to View-switching. Rewriting all of them
  through an extra indirection layer would have been a large, invasive,
  error-prone refactor for a narrow, secondary capability. **Real
  resolution:** those plain fields stay exactly as they are, used
  unchanged by every `add_*` factory and by `wrap_node`; a new,
  additional `pub(crate) active: SharedActiveTree` field was added
  instead. `WindowRuntime`'s own per-frame/per-input closures (`app.rs`)
  re-sync their existing plain `tree`/`root`/`handlers`/`context_menus`
  fields from `active` at the top of every real invocation -- 4 real
  call sites needed this: `frame` (re-syncs into `runtime`'s own owned
  fields, tight block, borrow dropped immediately), `input` (identical
  pattern), `access` (read-only, reads `active` locally since it only
  ever holds `&runtime`), `access_action` (same, plus a real bug found
  there -- see below). None of the ~230 `add_*`-factory call sites
  needed to change at all.
- `Window.show_view(view: &View)` (`window.rs`, added right after
  `from_view`): writes a whole new `ActiveTree` into the shared cell in
  one `RefCell` replace, picked up on the very next real frame. Also
  syncs the window's current size into `view.width`/`view.height`
  (mirroring `from_view`'s own real "sync size into the view" step) --
  **real, stated smaller limit, not silently glossed over:** unlike
  `from_view`'s own `Rc`-identity sharing (the *same* `Rc<Cell<u32>>`),
  this only copies the *current* size once, at switch time -- a later
  live resize while a *different* View is showing won't keep this one's
  own size in sync until `show_view` is called on it again. Named as a
  real, separate follow-up if a live resize ever needs to reach every
  registered View at once, not needed for this milestone's real scope
  (only the active View is ever visible/interactive at a time).
- **Real bug #1, caught and fixed before this ever shipped, not found
  later:** the first working draft of `Window.click`/`hover`/`scroll`/
  `right_click` (`window_input.rs`, updated to read through `self.
  active` instead of `self.tree`/`self.root`/`self.handlers` so
  synthetic dispatch stays correct after a `show_view` switch) held a
  live `Ref<ActiveTree>` (from `self.active.borrow()`) across the very
  `run_dispatch_outcome` call that can synchronously invoke a real
  Python handler. A handler calling `window.show_view(...)` from inside
  itself -- the exact, expected real pattern a nav button uses -- would
  then hit `show_view`'s own `self.active.borrow_mut()` while that
  outer `Ref` was still alive on the stack, panicking with "already
  borrowed." Caught by direct reasoning about the borrow lifetime
  before writing the live example (not by a failing test -- the first
  pytest test written, `test_show_view_switches_a_live_windows_
  dispatch_to_a_different_view`, calls `show_view` directly from the
  test body, never touching the buggy code path at all, a real reminder
  that a test proving the *feature* doesn't automatically prove the
  *reentrant* case). Fixed by cloning the needed `Rc`s (`tree`/`root`/
  `handlers`/`context_menus` as needed per method) out of `active` and
  dropping the borrow immediately, *before* any dispatch/callback runs
  -- the identical pattern already correct in `app.rs`'s own `frame`/
  `input` closures (their own refresh block is tightly scoped and
  already dropped before continuing). Re-grepped for every remaining
  `runtime.active.borrow()`/`self.active.borrow()` site after fixing
  `window_input.rs` and found the identical bug in one more spot missed
  on the first pass: `app.rs`'s `access_action` closure (a real
  AccessKit-driven activation calling `run_dispatch_outcome` while still
  holding `active`) -- fixed the same way.
- **Real bug #2, caught by this crate's own pre-existing regression
  test, not by inspection alone:** `PyWindow::__traverse__`'s new pass
  over `self.active.borrow().handlers`, unconditional in the first
  draft, calls `visit.call` a second time on the *same* `Py<PyAny>`
  object whenever `active.handlers` is still the same `Rc` as `self.
  handlers` (the common, never-switched case -- true for every `Window`
  built via plain `Window::new()`, not just `from_view`). Running the
  full verification chain's own `pytest tests/` step surfaced this
  directly: `test_window_participates_in_cyclic_gc_when_a_click_
  handler_captures_it_back` (a real, pre-existing regression test proving
  a window<->handler reference cycle gets collected) started failing --
  the cycle stopped being collected. Root cause, confirmed by reasoning
  through CPython's own cyclic-GC algorithm: `tp_traverse`'s `visit.call`
  is how a container reports one real outgoing reference for the
  "subtract internal refs from total refcount" phase; reporting the
  identical single Rust-level `Py<PyAny>` reference *twice* (once via
  `self.handlers`, once via `self.active.borrow().handlers`, the same
  underlying `Rc<RefCell<HashMap>>`) makes CPython's collector believe
  there are more real edges into that object than actually exist,
  undercounting how many of its references are purely internal to the
  candidate cycle -- so the cycle looks like it still has an external
  referent and survives. Fixed with an `Rc::ptr_eq(&self.handlers,
  &active_handlers)` guard: the second traversal only runs when
  `active`'s handlers are genuinely a *different* map (i.e. after a
  real `show_view` switch to a different `View`). Re-ran the full
  pytest suite after the fix: 589 passed (up from 586, the correct +3
  for this phase's own new tests), confirming the regression test
  passes again, not just that the build succeeds.
- New pytest tests (`tests/test_view_in_window.py`, 3 new):
  `test_show_view_switches_a_live_windows_dispatch_to_a_different_view`
  -- a click through the *window* after `show_view` reaches the
  newly-shown View's own handler and not the old one, proving real
  tree/root/handlers sharing, not a stale copy;
  `test_show_view_called_from_inside_a_click_handler_does_not_panic` --
  the one test that actually exercises real bug #1 above (calls
  `show_view` from *inside* a dispatched handler, not from the test
  body directly);
  `test_show_view_keeps_each_views_bindings_independently_reactive` --
  a `Signal` write on either View's own `ViewModel` after switching away
  from it still reaches its own (merely not-currently-shown) tree, and
  doesn't leak into the other View.
- New live example (`examples/live_view_switch.py` +
  `live_view_switch_a.yaml`/`live_view_switch_b.yaml`): two fully-
  bootstrapped, independent `View`s (`Screen A`/`Screen B`), each with
  its own real `_attach`-wired `on_click` handler that calls `window.
  show_view(...)` from *inside* itself -- three real dispatched clicks
  switch A->B->A->B, asserted via a real `switches` list recording each
  transition, then a genuine 20-frame `App.run()`. Ran clean: "switch
  sequence: ['a->b', 'b->a', 'a->b']" / "live_view_switch.py: exited
  cleanly after a real 20-frame render loop, switching Views three
  times."
- `.pyi` stub (`python/tre/_core.pyi`) updated: `Window.show_view`
  added with a full docstring, including the real, stated size-sync
  limit named above.
- `BUILD_TRACKER.md`: Phase 2 flipped to ✅ with the full real
  investigation/implementation/bug-fix writeup (both real bugs named
  honestly, not glossed over); milestone-level status flipped to
  "Complete, both phases"; Top Metrics row updated to 100%; a new "Just
  closed" pointer added for the whole milestone, above Phase 1's own
  (left unedited, a permanent historical record). Parser confirmed
  balanced (42 milestones, 133 phases, 226 items, unchanged -- pure
  status flips and prose on already-scoped phases); artifact
  regenerated and republished.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (unchanged counts across every crate -- `show_view` itself needs a
  live Python interpreter to call through pyo3's own generated wrapper,
  so no new Rust-level `#[test]` was added for it, matching this
  crate's established GIL-needed/GIL-free test-surface split);
  `maturin develop --release` rebuilt; `pytest tests/` (589 passed, +3,
  1 skipped, unchanged -- and, decisively, the pre-existing GC
  regression test passing again after bug #2's fix); all 79 examples
  (+2, `live_view_switch.py` plus a companion example the prior segment
  already added) and the showcase demo run clean.

**M42 -- Live `View`: Wiring the Declarative Layer into a Real Window --
is now fully complete, both phases.** This closes the milestone -- per
the standing "push only after a full milestone closes" convention, a
`git push` is now appropriate. Part 2 of the approved Tesserae bootstrap
plan (bootstrapping the separate `tesserae` repo itself) is next.
