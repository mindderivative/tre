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

## Status

**M55 Phase 1 of 3 is complete.** The real mechanical half -- a click-
to-focus or Tab-navigation transition genuinely producing a real,
observable outcome instead of being silently discarded -- is proven.
Committing locally now. Up next: Phase 2, the `engine-py` side --
`Event::focus_transition`, `FocusEnter`/`FocusExit` registration
methods, the AccessKit `Action::Focus` wiring, `Node.focus()`/
`Window.focus()`, and fixing the two right-click call sites that
currently discard their own dispatch outcome.
