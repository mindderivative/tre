# PLAN — M42 Phase 2: Swap Which `View` a Live Window Shows

## Goal
Add `Window.show_view(view)`, switching which `View` an already-live
`Window` shows, without closing/reopening it -- the real capability
behind the user's own explicit plan-review feedback: "this allows for
switching of current views without needing to bootstrap each
view/viewModel."

## Steps
1. Re-read the approved plan's own Phase 2 text ("a new shared type
   wrapping `Rc<RefCell<(Rc<RefCell<Tree>>, NodeId)>>`") and re-verified
   it against real source before implementing anything.
2. **Real, load-bearing correctness finding, not covered by that
   original text:** `engine_core::NodeId` is a `slotmap` generational
   key, unique only *within* the `Tree` that allocated it -- two
   independent `View`s' own root nodes can (and do) collide on the
   identical raw value. `HandlerMap`/`context_menus` are keyed by
   `(NodeId, EventKind)`/`NodeId` alone, no per-`Tree` namespacing --
   sharing one persistent map across a switch would silently cross-wire
   a different `View`'s old callback onto a colliding `NodeId` in the
   new one. Widened the swap bundle to `tree`+`root`+`handlers`+
   `context_menus` together (`window::ActiveTree`/`SharedActiveTree`),
   not just `(Tree, NodeId)`. `theme`/`completions` deliberately stay
   outside it (`ThemeState` isn't keyed by `NodeId` at all;
   `CompletionRegistry` is keyed by its own private monotonic counter,
   not `NodeId` -- no collision risk either way, and Phase 1 already
   named live theme-switching/`on_complete` on View-sourced nodes as
   real, separate, stated gaps this milestone doesn't fix).
3. **Real, scope-narrowing design choice, confirmed by grep before
   implementing:** changing `PyWindow`/`WindowSetup`/`WindowRuntime`'s
   own `tree`/`root`/`handlers`/`context_menus` field *types* would have
   needed rewriting ~230 pre-existing call sites across
   `window_factory.rs`/`window_input.rs`/etc. Instead, those plain
   fields stay exactly as they are; a new, additional `active:
   SharedActiveTree` field was added, and `WindowRuntime`'s own
   per-frame/per-input closures (`app.rs`) re-sync their existing plain
   fields from it at the top of every real invocation (4 real call
   sites: `frame`, `input`, `access`, `access_action`).
4. `Window.show_view(view: &View)` (`window.rs`) writes a whole new
   `ActiveTree` into the shared cell in one `RefCell` replace; also
   syncs the window's current size into `view.width`/`height` (a real,
   smaller stated limit than `from_view`'s own `Rc`-identity sharing).
5. **Real bug #1, caught and fixed before shipping:** an early draft of
   `Window.click`/`hover`/`scroll`/`right_click` (`window_input.rs`,
   updated to read through `active` so synthetic dispatch stays correct
   post-switch) held a live `Ref` on `self.active` across the very
   `run_dispatch_outcome` call that can invoke a real Python handler --
   a handler calling `show_view` (the exact real "nav button switches
   screens" pattern) would panic on `show_view`'s own `borrow_mut()`
   while that `Ref` was still alive. Fixed by cloning the needed `Rc`s
   out and dropping the borrow *before* any dispatch/callback runs.
   Found the identical bug in one more spot missed on the first pass
   (`app.rs`'s `access_action` closure) and fixed it the same way.
6. **Real bug #2, caught by this crate's own pre-existing regression
   test, not by inspection:** `PyWindow::__traverse__`'s new pass over
   `active`'s handlers, unconditional in an early draft, called
   `visit.call` a second time on the *same* `Py<PyAny>` object whenever
   `active.handlers` was still the same `Rc` as `self.handlers` (the
   common, never-switched case) -- `test_window_participates_in_
   cyclic_gc_when_a_click_handler_captures_it_back` started failing.
   CPython's cyclic collector counts each `visit.call` as one real
   outgoing edge when subtracting internal refs from an object's total
   refcount; double-reporting the identical single reference made a
   genuine cycle look like it still had an external referent and
   survive collection. Fixed with an `Rc::ptr_eq` guard.
7. New pytest tests (`tests/test_view_in_window.py`, 3 new): a click
   through the window after `show_view` reaches the newly-shown View's
   own handler, not the old one; `show_view` called from *inside* a real
   dispatched click handler doesn't panic (the exact scenario bug #1
   needed); each View's own bindings stay independently reactive across
   a switch.
8. New live example (`examples/live_view_switch.py` + two YAML/
   `ViewModel` pairs): two fully-bootstrapped `View`s, a real nav button
   in each calling `window.show_view(...)` from inside its own
   dispatched handler, switching A->B->A->B three times, then a genuine
   20-frame `App.run()`.
9. `.pyi` stub updated: `Window.show_view` added.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt`, `cargo test --workspace --release` (unchanged counts --
`show_view` itself needs a live Python interpreter to call, matching
this crate's own GIL-needed/GIL-free test-surface split), `maturin
develop --release`, `pytest tests/` (589 passed, +3, 1 skipped
unchanged, confirming the real GC regression above is genuinely fixed,
not just that the code compiles), all 79 examples, showcase demo.
**M42 -- Live View: Wiring the Declarative Layer into a Real Window --
is now fully complete, both phases.** Part 2 of the approved plan
(bootstrapping the separate `tesserae` repo) is next.
