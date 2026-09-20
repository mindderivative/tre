# PLAN — M43 Phase 2: Real Removal, with Automatic `Signal` Unsubscription

## Goal
Add `Component.remove()`, tearing an instance's subtree down and
unsubscribing its own bindings' `Signal`s, closing the real panic risk
Phase 1's own investigation found (a removed-but-still-subscribed
component's `BindingCallback` would panic on the next write to a
`Signal` it read from).

## Steps
1. `python/tre/__init__.py`: added `Signal._unsubscribe(callback)` --
   removes `callback` from `self._subscribers` if present, a silent
   no-op otherwise (matching `Tree::remove`'s own convention).
2. `component.rs`: added `subscriptions: Vec<(Py<PyAny>, Py<PyAny>)>` to
   `Component`; `_attach` now keeps the list `attach_bindings_and_
   handlers` returns (View discards it, since a View is never removed).
3. New `Component.remove(&mut self, py)`: unsubscribes every tracked
   `(signal, callback)` pair, *then* `Tree::remove(self.reconciler.
   root())` -- unsubscribing first is real, deliberate ordering, not
   incidental (a `Signal` write racing with removal must never reach a
   `BindingCallback` whose `NodeId` is already gone).
4. `.pyi`/`BUILD_TRACKER.md` updated.
5. New pytest tests (`tests/test_component.py`, +4): a `Signal` write
   after `remove()` doesn't panic (confirmed, by reading `Node::
   set_text`'s own `.expect()`, that this really would have panicked
   before the fix); a real remove/add cycle across 5 iterations stays
   stable, with an untouched sibling's own state proven unaffected; a
   sibling's own click still works after a neighbor is removed.
6. **Real, significant bug found and fixed by actually running the
   extended live example, not by inspection:** widened
   `examples/component_list.py` into a real "Add Card"/per-card
   "Remove" button flow -- running it hit `RuntimeError: Already
   mutably borrowed` inside `view.instantiate(...)`, called from a
   dispatched `on_click` handler. Root cause: `View::click`/`hover`/
   `right_click`/`_attach` all took `&mut self`, even though none of
   their bodies mutate a plain `View` field directly (every real
   mutation goes through an interior-mutable `Rc<RefCell<...>>`/`Cell`)
   -- pyo3 holds an *exclusive* borrow on the whole `View` Python object
   for a `&mut self` method's entire duration, so a handler dispatched
   from inside `click()` calling any other method on that same `view`
   (exactly `view.instantiate(...)` from an "Add" button) panicked.
   Fixed by widening all four to `&self`, matching `Window`'s own
   `click`/`hover`/`right_click` (`window_input.rs`), which already
   used `&self` for the identical real reason. New pytest regression
   test (`test_instantiate_called_from_inside_a_click_handler_does_not_
   panic`) calls `instantiate` from *inside* a dispatched handler -- the
   one scenario that actually exercises this.
7. Extended `examples/component_list.py` + both yaml files: real
   "Add Card" button (wired to an `AppViewModel.add_card` handler on
   the outer `View`) and a per-card "Remove" button (`CardViewModel.
   remove_self`, calling `component.remove()`) -- 3 cards added via
   real dispatched clicks, one clicked, one removed via its own
   dispatched click, one more added, then a genuine 20-frame
   `App.run()`.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt`, `cargo test --workspace --release` (213 unchanged),
`maturin develop --release`, `pytest tests/` (600 passed, +4, 1 skipped
unchanged), all 80 examples, showcase demo. **M43 -- Embeddable
Components: Multi-Instance Views with Independent ViewModels -- is now
fully complete, both phases.** This closes the milestone -- per the
standing "push only after a full milestone closes" convention, a `git
push` is now appropriate.
