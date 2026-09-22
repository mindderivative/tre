# LOG — M55: Real `FocusEnter`/`FocusExit` Events

- User: "What's next?" -> recommended `Focus` `EventKind` (M54's own
  explicitly-deferred candidate) or an `Event.source`-to-`Node`
  upgrade, or a fresh "Known gaps" audit. User: "Scope Focus
  EventKind."
- Dispatched a dedicated Explore agent to trace every real place
  `Tree.focused` changes before designing anything.
- **Critical finding:** a real focus-changing `Tree::dispatch()` call
  (`PointerPressed`'s click-to-focus, `KeyPressed`'s Tab navigation)
  and a real `Activated`/`SecondaryActivated`/`Changed`-producing call
  never overlap in the same single `dispatch()` invocation -- both
  focus-mutating arms always returned `DispatchOutcome::None` today, so
  a new `DispatchOutcome::FocusChanged` variant slots in with zero
  restructuring. `self.focused` can never go stale (`Tree::remove`/
  `detach` both proactively clear it). AccessKit's `Action::Focus`
  (`app.rs:1146`) calls `Tree::set_focus_to` directly, bypassing
  `dispatch()` entirely -- the one real non-dispatch mutation path. Two
  real `right_click` call sites (`window_input.rs`, `view.rs`)
  currently discard their own `PointerPressed` dispatch's outcome with
  no variable at all -- a real, pre-existing gap this investigation
  found, not introduced by this milestone.
- Five real design questions resolved: four via `AskUserQuestion`
  (`FocusEnter`/`FocusExit` pair, not a single `Focus` kind -- mirrors
  `HoverEnter`/`HoverExit`, the one real precedent for this identical
  transition shape; a new `DispatchOutcome` variant, not a parallel
  return; AccessKit's `Action::Focus` fires the same event, parity with
  `Action::Click`; add `Node.focus()`/`Window.focus(node)`, mirroring
  `click()`/`hover()`'s own synthetic-entry-point precedent), one
  decided directly (`Tree::remove`/`detach`'s own silent focus-clearing
  stays out of scope -- no real `InputEvent`/user action drives either,
  and `Hover` has no analogous case).
- Entered Plan Mode with this real, grounded scope before implementing.

## Phase 1 — `engine-core`: `FocusChanged` Outcome

- New `EventKind::FocusEnter`/`FocusExit` (`input.rs`) -- a pair, not a
  single kind, since `HandlerMap`'s own per-node key can never give one
  event two real sources.
- New `DispatchOutcome::FocusChanged { old: Option<NodeId>, new:
  Option<NodeId> }`, mirroring `HoverChanged`'s exact real shape.
- Widened `transition_focus` (the real shared chokepoint `move_focus`/
  `set_focus_to` both already funnel through) to return `Option<
  (Option<NodeId>, Option<NodeId>)>` -- `None` on its own existing
  `old == new` early return, `Some((old, new))` otherwise. Propagated
  through `move_focus`/`set_focus_to`'s own now-widened return types
  (both were `()`, neither had any real caller reading a return value,
  so a low-risk signature change).
- `Tree::dispatch`'s `PointerPressed` (click-to-focus) and `KeyPressed`
  `Key::Tab` (`move_focus`) arms now return `DispatchOutcome::
  FocusChanged{old,new}` when a real transition happened, replacing
  their prior unconditional `DispatchOutcome::None`.
- 3 new Rust unit tests: a real click-to-focus on a previously-
  unfocused `TextField` reports the transition, and pressing the
  already-focused field again reports `None` (no stale repeat);
  real Tab navigation onto the one interactive node reports the
  transition; clicking a plain, non-`TextField`/`Terminal` node
  reports `None` and never fabricates a transition.
- Full `engine-core` chain green: `cargo check`/`clippy -D warnings`/
  `fmt --check` clean, `cargo test -p engine-core --release` (227
  passed, up from 224, +3). `cargo check --workspace --all-targets`
  confirms the only remaining breakage is in `engine-py` (`dispatch.rs`
  /`event.rs`'s own exhaustive matches on the now-widened
  `DispatchOutcome`/`EventKind`) -- exactly Phase 2's own scope.

## Phase 2 — `engine-py`: `Event` Wiring + `Window.focus`/`View.focus`

- `Event::focus_transition(kind, node)` (`event.rs`): mirrors `Event::
  hover` exactly, but `position` stays `None` -- a focus transition,
  unlike hover, never carries a real pointer position regardless of
  which of its several real sources (click, Tab, explicit `focus()`,
  AccessKit) caused it. `kind_name` widened for the two new variants.
- `run_dispatch_outcome`'s new `FocusChanged` arm delegates to a new
  `pub(crate) fn fire_focus_transition(handlers, old, new, py)`
  (`dispatch.rs`) -- factored out specifically because it has a
  *second* real caller: AccessKit's own `Action::Focus` handling
  (`app.rs`), which calls `Tree::set_focus_to` directly and never
  reaches `Tree::dispatch`/`run_dispatch_outcome` at all. One real
  implementation, two real callers, mirroring `HoverChanged`'s own
  two-single-source-`call_handler`-calls shape.
- `app.rs`'s `Action::Focus` handling captures `tree.set_focus_to`'s
  now-widened return and calls `fire_focus_transition` -- parity with
  `Action::Click`'s own existing treatment immediately above it.
- `Node.set_on_focus_enter`/`set_on_focus_exit` (`node.rs`), mirroring
  `set_on_hover_enter`/`exit` exactly, through the same `dispatch::
  register_handler` arity-sniff. `view.rs`'s declarative handler-name
  mapping widened with `on_focus_enter:`/`on_focus_exit:`, the same
  extension `on_change` already established.
- **Real, found-while-implementing correction to the approved plan's
  own original phrasing:** the plan said "`Node.focus()`/`Window.
  focus(node)`" -- implementing it found that `Node` has zero existing
  self-dispatching methods anywhere (confirmed via grep); `click`/
  `hover`/`right_click` all live exclusively on `Window`/`View`, taking
  a `node: PyRef<'_, Node>` argument. Landed as `Window.focus(node)`/
  `View.focus(node)` instead, matching that real, established
  convention rather than breaking it. Both call `Tree::set_focus_to`
  directly (no real `InputEvent` represents "focus this specific
  node," the identical real reason AccessKit's own path does the same)
  and fire the transition via the shared `fire_focus_transition`.
- Fixed a real, pre-existing gap Phase 1's own investigation found:
  `Window.right_click`/`View.right_click`'s own `PointerPressed`
  dispatch used to discard its outcome with no variable binding at all
  -- captured and forwarded now, through `run_dispatch_outcome` like
  every other real dispatch call site already does. A real right-
  click-to-focus (M53) on `TextField`/`Terminal` is now genuinely
  observable from both entry points for the first time.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean across the whole workspace, first attempt; `cargo test
  --workspace --release` (unchanged). `maturin develop --release`; a
  standalone smoke script confirmed every real path end to end before
  writing formal tests: click-to-focus, `Window.focus()`, backward-
  compat zero-arg handlers, and the newly-fixed right-click-to-focus
  observability. `pytest tests/` (767 passed, 2 skipped -- unchanged
  from before this phase).

## Phase 3 — Python-Facing API, Tests, Example, Docs

- `python/tre/_core.pyi`: new `set_on_focus_enter`/`set_on_focus_exit`
  stubs on `Node`, mirroring `set_on_hover_enter`/`exit`; new `focus
  (node)` stubs on `Window`/`View`, mirroring `hover(node)`; `Event`'s
  own class doc and `kind` field widened to name `"focus_enter"`/
  `"focus_exit"`.
- 9 new pytest tests (`tests/test_focus_events.py`): `Window.focus()`
  gives a one-arg handler a real `Event` with every other field
  `None`; focus-exit fires when focus moves to a sibling; focusing an
  unregistered/already-focused node is a safe no-op with no stale
  repeat; real click-to-focus *and* the newly-fixed real right-click-
  to-focus on a `TextField` both fire `FocusEnter`; real Tab
  navigation fires it; `View.focus()`'s own declarative `on_focus_
  enter:` wiring actually invokes the bound `ViewModel` method; an
  uncaught exception in a focus handler is caught, logged, and
  non-fatal.
- AccessKit's own `Action::Focus` wiring verified by code review and
  parity with `Action::Click`'s identical, equally untestable-in-this-
  environment pattern -- no live AT-SPI/UIA/NSAccessibility client
  exists in this dev/CI environment, and no existing pytest harness
  exercises `Action::Click` either, confirmed via grep before
  concluding this rather than assumed.
- New `examples/focus_events.py`: a real, live window demonstrating
  `FocusEnter`/`FocusExit` on two `TextField`s via explicit `Window.
  focus()`, real click-to-focus, the newly-fixed real right-click-to-
  focus, and real Tab navigation, side by side with both zero-argument
  and one-argument handlers.
- `BUILD_TRACKER.md`: Phase 2 and Phase 3 sections added, milestone
  marked ✅ complete, Top Metrics updated to 100%. Regenerated: 55
  milestones/165 phases/306 items/1 known gap/20 fixed gaps. Artifact
  republished.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean (no Rust changes this phase); `cargo test --workspace
  --release` (unchanged). `maturin develop --release`; `pytest tests/`
  (776 passed, up from 767, +9, 2 skipped unchanged); all 87 examples
  (+1, zero failures); `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M55 is complete -- all 3 phases.** The real capability gap this
milestone exists to close -- keyboard focus genuinely changing with
no way for a registered handler to know -- is closed, with zero
breaking changes to any of the 767 pre-existing tests or 86 pre-
existing examples. Two real, found-while-implementing corrections to
the approved plan (documented honestly, not glossed over): the new
synthetic focus entry point landed as `Window.focus`/`View.focus`, not
`Node.focus()`, once the established convention was confirmed; and a
real, pre-existing gap in both `right_click` implementations (a
discarded dispatch outcome) was found and fixed along the way, not
just the new `Focus` mechanism added around it. Committing locally
now; push deferred pending explicit user confirmation, per this
session's own established convention.
