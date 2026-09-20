# LOG — M43 Phase 2: Real Removal, with Automatic `Signal` Unsubscription

- User's own governing instruction: the approved plan
  (`/home/phil/.claude/plans/reflective-sleeping-falcon.md`), Part 2 --
  close the real panic risk Phase 1's own investigation named:
  `python/tre/__init__.py`'s `Signal` had `_subscribe` but no
  `_unsubscribe`, meaning a removed component's own `BindingCallback`
  would stay subscribed forever, panicking the next time that `Signal`
  was written to.
- `python/tre/__init__.py`: added `Signal._unsubscribe(callback)` --
  `self._subscribers.remove(callback)` inside a `try`/`except
  ValueError: pass`, matching `Tree::remove`'s own "not found is a
  no-op, not an error" convention throughout this codebase. Confirmed
  `list.remove()` uses `==`, which falls back to identity (`is`) for
  `BindingCallback`/`TwoWayCallback` pyo3 objects (neither defines
  `__eq__`), so this correctly removes the exact subscribed callback.
- `component.rs`: `Component` gained `subscriptions: Vec<(Py<PyAny>,
  Py<PyAny>)>`; `_attach` now keeps the list `attach_bindings_and_
  handlers` returns (`View::_attach` still discards it -- a `View` is
  never removed, so it has nothing to unsubscribe later). New
  `Component.remove(&mut self, py)`: unsubscribes every tracked
  `(signal, callback)` pair first, *then* `self.tree.borrow_mut()
  .remove(self.reconciler.root())` -- this ordering is real and
  deliberate, not incidental: unsubscribing before removing means a
  `Signal` write that happens to race with this call can never reach a
  `BindingCallback` whose own `node_id` is already gone from the
  `Tree`.
- New pytest tests (`tests/test_component.py`, 4 new, before the
  reentrancy fix below): `test_a_signal_write_after_remove_does_not_
  panic` -- confirmed real, not accidental, by reading `Node::set_text`
  ('s own `tree.get_mut(self.id).expect("Node holds a NodeId missing
  from its own Tree...")`) directly before writing the test: this
  genuinely would have panicked pre-fix, since `BindingCallback::
  __call__` -> `apply_binding_value` -> (`Value::Str` case) `temp_node.
  set_text(...)` hits exactly this `.expect()` on a `NodeId` `Tree::
  remove` already deleted. `test_remove_add_remove_cycle_stays_stable`
  -- 5 real instantiate/click/remove iterations into the same
  container, with a never-removed sibling's own state proven completely
  unaffected throughout. `test_remove_is_safe_to_call_once_and_stops_
  dispatch_reaching_the_handler` -- a sibling's own click still
  dispatches correctly after a neighbor is removed, proving the shared
  `Tree` stays healthy.
- **Real, significant bug caught and fixed before this ever shipped,
  found by actually running the extended live example, not by
  inspection or by any of the pytest tests above (none of them happened
  to exercise the reentrant path):** widened `examples/component_list.
  py` into the plan's own called-for "Add Card"/"Remove Card" real
  button flow -- `component_list.yaml` gained a real `add_button`
  (`on_click: "add_card"`), `component_list_card.yaml` gained a real
  `remove_button` (`on_click: "remove_self"`). Running the script hit
  `RuntimeError: Already mutably borrowed` inside `view.instantiate
  (...)`, called from `AppViewModel.add_card`, itself dispatched via
  `view.click(add_button)`.
  Root-caused by reasoning through pyo3's own borrow model, not
  guessing: `View::click`/`hover`/`right_click`/`_attach` (`view.rs`)
  had always taken `&mut self`, matching `View`'s own original,
  pre-M43 shape -- but re-reading each body confirmed none of them
  actually mutate a plain `View` struct field directly; every real
  mutation goes through an interior-mutable `Rc<RefCell<Tree>>`/
  `HandlerMap`/etc. pyo3 holds an *exclusive* borrow on the whole
  `View` Python object for a `&mut self` method's entire duration --
  since `run_dispatch_outcome` (called from inside `click()`) invokes
  the Python handler *synchronously*, and that handler called `view.
  instantiate(...)` (a *different* method on the *same* `view` object),
  pyo3's own reentrant-borrow check correctly refused it. This had been
  a real, latent bug in `View::click`/etc. since long before M43 --
  M43 is simply the first real feature that ever called back into a
  `View` method from inside a dispatched handler, the exact "essence of
  MVVM and single page applications" pattern the user asked this whole
  milestone to support.
  Fixed by widening `click`/`hover`/`right_click`/`_attach` from `&mut
  self` to `&self` -- confirmed safe by rereading each body (no direct
  field mutation anywhere), and confirmed consistent with precedent:
  `Window`'s own `click`/`hover`/`right_click`/`scroll` (`window_input.
  rs`) already use `&self`, for the identical real reason. `View::
  poll_reload` was deliberately left as `&mut self` -- it genuinely
  calls `self.reconciler.reconcile(&mut tree, ...)`, a real mutation of
  a plain (non-interior-mutable) field, and isn't a real reentrancy
  risk in practice (not called from inside a dispatched handler).
  New regression test, `test_instantiate_called_from_inside_a_click_
  handler_does_not_panic` -- calls `view.instantiate(...)` from *inside*
  a dispatched `on_click` handler, the one scenario that actually
  exercises this; the four pre-existing Phase 2 tests above never
  touched this path (none of them called `instantiate` reentrantly), a
  real reminder that "the feature works" tests don't automatically
  cover "the feature works when triggered reentrantly."
- Re-ran the full pytest suite after the `&self` widening: 600 passed
  (up from 599, the correct +1 for the new regression test), confirming
  the widening is a pure capability-add with zero behavior change to
  any existing caller.
- `examples/component_list.py` (+ both yaml files) now demonstrates the
  full real dynamic-list lifecycle: 3 real dispatched clicks on a real
  "Add Card" button (each instantiating a fresh `Card` component with
  its own `CardViewModel`); a real dispatched click on one card's own
  "+1" button; a real dispatched click on a different card's own
  "Remove" button (calling `component.remove()` from inside its own
  handler, then notifying the script's own `cards` list via a plain
  Python callback); one more real "Add" click; then a genuine 20-frame
  `App.run()`. Ran clean end to end after the reentrancy fix.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (213 unchanged -- `Component.remove()`/the `&self` widening need a
  live Python interpreter, no new Rust-level `#[test]`s needed);
  `maturin develop --release` rebuilt; `pytest tests/` (600 passed, +4,
  1 skipped, unchanged); all 80 examples and the showcase demo run
  clean.

**M43 -- Embeddable Components: Multi-Instance Views with Independent
ViewModels -- is now fully complete, both phases.** This closes the
milestone -- per the standing "push only after a full milestone
closes" convention, a `git push` is now appropriate.
