# PLAN — M55: Real `FocusEnter`/`FocusExit` Events

## Goal
M54 closed every real, buildable "Known gaps" bullet except one it
explicitly deferred: `EventKind` has no `Focus` variant despite
ARCHITECTURE.md §16.2's own original sketch naming one. User: "Scope
Focus EventKind."

## Real investigation
A real focus-changing `Tree::dispatch()` call and a real `Activated`/
`SecondaryActivated`/`Changed`-producing call never overlap in the same
single `dispatch()` invocation -- both focus-mutating arms always
returned `DispatchOutcome::None`, so a new `FocusChanged` variant slots
in with zero restructuring. AccessKit's `Action::Focus` bypasses
`dispatch()` entirely (the one non-dispatch mutation path). Two real
`right_click` call sites currently discard their own dispatch outcome
with no variable at all. No Python-facing explicit "focus this node"
API exists. M54 already built every mechanism this milestone reuses:
`Event`, arity-sniffing, `HandlerMap`, and `HoverChanged`'s own exact
old/new-transition pattern to copy almost verbatim.

## Design (3 phases)
1. `engine-core`: `EventKind::FocusEnter`/`FocusExit`; `DispatchOutcome
   ::FocusChanged{old,new}`; widen `transition_focus`/`move_focus`/
   `set_focus_to` to return the real transition; wire `PointerPressed`/
   `KeyPressed(Tab)` to produce it.
2. `engine-py`: `Event::focus_transition`; `run_dispatch_outcome`'s new
   arm + a shared `fire_focus_transition` helper (also used by
   AccessKit's `Action::Focus`); `Node.set_on_focus_enter`/
   `set_on_focus_exit`; `Node.focus()`/`Window.focus(node)`; fix the
   two right-click call sites that discard their outcome.
3. Python-facing API, tests, example, docs.

## Five real design questions, resolved
1. `FocusEnter`/`FocusExit` pair, not a single kind -- mirrors `Hover`.
2. New `DispatchOutcome` variant, not a parallel return.
3. AccessKit's `Action::Focus` fires the same event -- parity with
   `Action::Click`.
4. Add `Node.focus()`/`Window.focus(node)` -- mirrors `click()`/
   `hover()`.
5. `Tree::remove`/`detach`'s own silent focus-clearing stays out of
   scope (decided directly, no real tradeoff) -- no `InputEvent`/user
   action drives either, and `Hover` has no analogous case.

## Explicitly out of scope, named not silent
`Tree::remove`/`detach` firing a "focus lost" event. Any change to
`move_focus`'s own Tab-order computation. `Event.source`-to-`Node`
(still the same deferred M54 decision).

## Status

**All 3 phases complete. Milestone closed.**

Phase 1: new `EventKind::FocusEnter`/`FocusExit`; new `DispatchOutcome
::FocusChanged { old, new }`; `transition_focus`/`move_focus`/`set_
focus_to` widened to return the real transition; `Tree::dispatch`'s
`PointerPressed` (click-to-focus) and `KeyPressed`'s `Key::Tab` arms
now produce `FocusChanged` instead of their prior unconditional
`None`. 3 new Rust unit tests.

Phase 2: `Event::focus_transition` (mirrors `Event::hover`, `position`
stays `None`); `run_dispatch_outcome`'s new arm delegates to a new
shared `fire_focus_transition(handlers, old, new, py)`, reused directly
by AccessKit's own `Action::Focus` handling in `app.rs` (which calls
`Tree::set_focus_to` directly, never through `dispatch()`, so it can't
reach `run_dispatch_outcome` at all); `Node.set_on_focus_enter`/
`set_on_focus_exit`; declarative `on_focus_enter:`/`on_focus_exit:`
wiring in `view.rs`. **Real, found-while-implementing correction to
this plan's own original phrasing:** the new synthetic "focus this
node" entry point landed as `Window.focus(node)`/`View.focus(node)`,
not `Node.focus()` -- confirmed via grep that `Node` itself has zero
existing self-dispatching methods (`click`/`hover`/`right_click` all
live exclusively on `Window`/`View`, taking a `node` argument), so
`Node.focus()` would have broken an established convention rather
than followed one. Also fixed a real, pre-existing gap Phase 1's own
investigation found: `Window.right_click`/`View.right_click`'s own
`PointerPressed` dispatch used to discard its outcome with no variable
binding at all -- captured and forwarded now, making a real right-
click-to-focus (M53) on `TextField`/`Terminal` observable for the
first time.

Phase 3: `_core.pyi` gets new stubs; 9 new pytest tests in `tests/
test_focus_events.py` (`Window.focus()`, focus-exit-on-sibling-move,
safe no-ops, real click-to-focus, the newly-fixed real right-click-to-
focus, real Tab navigation, `View.focus()`'s own declarative wiring,
exception handling); new `examples/focus_events.py`. AccessKit's own
`Action::Focus` wiring verified by code review/parity with `Action::
Click` (no live accessibility client in this dev/CI environment, and
no existing pytest harness exercises `Action::Click` either, confirmed
via grep before concluding rather than assumed).

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean, `cargo test --workspace --release` (unchanged), `maturin
develop --release`, `pytest tests/` (776 passed, up from 767, +9, 2
skipped unchanged), all 87 examples (+1), showcase demo. Tracker
generator: 55 milestones/165 phases/306 items/1 known gap/20 fixed
gaps.
