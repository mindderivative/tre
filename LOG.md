# Log: M4 Phase 4 — Wire `View`'s Declarative Handlers to Real Dispatch (§16.2)

Corresponds to `BUILD_TRACKER.md` M4 Phase 4. Closes the gap `view.rs`'s
own module doc comment had named since before M4 existed: `View::_attach`
validated every declared handler eagerly but never registered it
anywhere real dispatch reached, so a `view.yaml`'s `{on_click: "bump"}`
did nothing when actually clicked.

## Investigation before writing code

Re-read §16.2 in full, then read `view.rs` end to end (not just the
handler loop) before deciding an approach. Two real facts changed the
plan from what `BUILD_TRACKER.md`'s prior scoping assumed:

- `View` owns a completely standalone `Tree` (`View::new` calls
  `Tree::new()` directly) — it has no width/height and is never
  embedded into a `PyWindow`. Giving `View` a real winit-driven render
  loop (parity with `PyWindow`'s whole lifecycle) would be real,
  separate, much larger work than the actual confirmed bug. Scope
  narrowed to NOT build that here — the confirmed gap is "a validated
  handler is never registered," not "a `View` can't run in a live
  window," and only the former needed fixing.
- `Node::set_on_click` was the exact existing mechanism to reuse
  (insert into a `click_handlers` map + add `Action::Click` to
  `access.actions`), the same way `apply_binding_value` already reuses
  `Node::animate` verbatim rather than reimplementing dispatch. It was
  module-private (`fn`, no `pub(crate)`) — widened to `pub(crate)`,
  matching `animate`'s own existing visibility, since it now has a
  second in-crate caller.

## What happened

`View::_attach`'s handler loop, after its existing eager validation
(`getattr` + `is_callable()`, unchanged), now also wires `on_click`
handlers for real: looks up the widget's `NodeId` via the already-real
`Reconciler::id_of`, constructs a temporary `Node` sharing `View`'s own
`tree`/`click_handlers` (the same construction `apply_binding_value`
already uses), and calls `Node::set_on_click` with the validated,
already-`getattr`'d bound method. Other declared event names still
validate (so a typo'd handler still fails at `_attach()` time) but
reach no real mechanism yet — matching §16.2's own "generalizing to
whatever named events a `NodeKind` exposes," not manufactured ahead of
M4 Phase 6's `EventKind` work.

New `View.click(node)` mirrors `Window.click` (M4 Phase 1 step 3)
exactly — the same no-live-window-needed proof pattern — but computes
layout with `AvailableSpace::MaxContent` on both axes instead of a
fixed window size, since `View` has none of its own. Every existing
`view.yaml` in this repo already declares an explicit `style.width`/
`style.height` on its root widget, so `MaxContent` sizes correctly
rather than needing a workaround. Dispatches a real primary
press+release pair at the node's computed center and runs
`dispatch::run_activation` against `View`'s own `click_handlers` for
each — the exact same shared mechanism `Window.click`/`App.run()`'s
`on_input` closure already use.

Handlers are called with zero arguments (`call0`), matching the
established, already-tested `Node.set_on_click`/`run_activation`
convention (`tests/test_click_dispatch.py`) rather than §16.2's own
illustrative `Event`-argument example — a real `Event`/`EventKind`
type doesn't exist anywhere in this codebase yet (M4 Phase 6's own
scope), so this phase reuses what's real today rather than
manufacturing a new argument-passing convention ahead of it.

## Verification

4 new pytest tests in `tests/test_view_handlers.py`: a real dispatched
click actually invokes a wired `bump` method (mutating a `Signal`, read
back to confirm); repeated clicks each invoke it again; a declared
non-`on_click` handler still validates but a click does not call it (no
mechanism exists yet); an uncaught exception from a real handler is
caught, printed, and non-fatal, matching `Window.click`'s own §9
policy. All passed on the first run.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean
$ cargo fmt --check                                           # clean
$ cargo test --workspace                                      # all green, unchanged elsewhere

$ maturin develop
$ python -m pytest tests/ -v
41 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py       # exited cleanly, unaffected
$ python examples/resizable_panes.py    # exited cleanly, unaffected
$ python examples/two_windows.py        # exited cleanly, unaffected
```

## Also fixed this session, before this step

While starting this phase, found and corrected a factual error in the
prior `BUILD_TRACKER.md` update: `Tree::dispatch`'s `PointerPressed` arm
already calls `interaction_mut(node).spawn_ripple(...)` for real — a
previous tracker entry wrongly claimed dispatch itself never spawned a
ripple, based on an incomplete grep read rather than reading the actual
match arm. The real, narrower gap (confirmed via grep): `engine-py`
never calls `interaction_mut` anywhere, so no real Python-built node
ever has `InteractionState` to animate. M4 Phase 5's scope was corrected
in `BUILD_TRACKER.md` accordingly (commit `1ff12dd`) before this step
began.

## Next

M4 Phase 5 (corrected scope): a Python-facing opt-in into
`InteractionState` so a real button can actually ripple/hover — the
`engine-core` mechanism underneath is already real and correct.
Real, stated-not-silent gaps unchanged: `View` still has no live
render loop of its own; only `on_click` reaches real dispatch (M4
Phase 6 generalizes this once a real `EventKind` exists); a handler's
zero-argument calling convention diverges from §16.2's own illustrative
`Event`-argument example, deliberately, until Phase 6 gives it
something real to pass.
