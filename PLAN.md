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

**Phase 1 of 3 complete.** New `EventKind::FocusEnter`/`FocusExit`;
new `DispatchOutcome::FocusChanged { old, new }`; `transition_focus`/
`move_focus`/`set_focus_to` widened to return the real transition;
`Tree::dispatch`'s `PointerPressed` (click-to-focus) and `KeyPressed`
`Key::Tab` arms now produce `FocusChanged` instead of their prior
unconditional `None`. 3 new Rust unit tests (real click-to-focus
transition + no stale repeat, real Tab navigation, a non-focusable
click correctly stays `None`). `cargo check -p engine-core`/`clippy -D
warnings`/`fmt --check` clean; `cargo test -p engine-core --release`
(227 passed, up from 224, +3). `cargo check --workspace --all-targets`
confirms the only remaining breakage is in `engine-py`, exactly
Phase 2's own scope. **Up next: Phase 2, the `engine-py` side.**
