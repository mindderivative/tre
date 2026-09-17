# Log: M14 Phase 3 — Real `Change` EventKind + Two-Way Binding Sugar (§5, §7.3, §16.7)

Corresponds to `BUILD_TRACKER.md` M14 Phase 3. Closes M14 entirely (3
of 3 phases). A real `Change` fired two ways (mechanical for a Slider
drag-release, direct for a Checkbox via `Node.set_checked`), plus
§16.7's two-way binding sugar wiring it into `View`/`ViewModel`.

## Investigation before writing code

Confirmed by direct read: `EventKind`/`DispatchOutcome` had no
`Change`/`Changed` at all; a Slider drag-release produced no
distinguishable outcome. Design Principle 6 (engine-core never knows
"meaning") rules out one uniform firing path — a Slider drag genuinely
completes inside `Tree::dispatch` (mechanical), but `checked` is
app-owned data `engine-core` never touches (§5), so its own `Change`
can only originate from `Node.set_checked` in `engine-py`. `call_
handler` was private — needed `pub(crate)` for `set_checked` to reuse
it directly rather than inventing a second dispatch path. Two-way
binding is a `View`/YAML-only concept, and `NodeKindSpec` only had
`Rect`/`Container`/`Text` — testing/demonstrating it required making
`Checkbox`/`Slider` declarable in `view.yaml` for the first time, a
real, additional scope expansion reasoned through explicitly.

## What happened

`EventKind::Change` + `DispatchOutcome::Changed(NodeId)` in
`engine-core`. `Tree::dispatch`'s `PointerReleased` arm checks for a
real ending Slider drag *before* clearing `self.dragging`, taking
priority over `Activated`. `Node.set_checked` now fires `Change`
directly via `call_handler` (widened to `pub(crate)`) after writing
`state.checked`. New `Node.get_checked`/`Node.set_on_change` (the
missing read-back getter and handler registration, mirroring `set_on_
click`/`set_on_hover_*`). `engine-py::dispatch::run_dispatch_outcome`
gained a `Changed` arm.

`WidgetSpec` gained `checked: bool`/`value: f64`/`two_way:
Option<String>` (all `#[serde(default)]`, additive); `NodeKindSpec`
gained `Checkbox`/`Slider`; `build.rs::node_kind_and_paint` gained
matching arms via the existing `required_background` helper —
`Checkbox`/`Slider` are now real, declarable `view.yaml` kinds, not
just imperative-API-only.

`View._attach`: `on_change` handler-name wiring; `apply_binding_value`
gained a `Value::Bool` branch calling `set_checked` (its stale "only
numeric" doc comment corrected — `animate()` already dispatched every
real numeric property with zero change needed); a new `TwoWayCallback`
pyclass registered as the widget's own `Change` handler, reading the
node's current value back (`get_checked()`/`get(property)`) and
writing it into the bound `Signal`.

**Real finding #1, caught only once the round-trip was actually
tested, not merely unit-tested in isolation:** a first draft required
the two-way binding's own expression to be a *bare* `Expression::Ident`
(matching ARCHITECTURE.md §16.7's own inline illustration, `{{
username }}` with no `.get()`). But every binding's forward direction
resolves through `PyViewModelResolver::ident`, which reads the raw
Python attribute unmodified — for a `Signal`, that's the `Signal`
object itself, not its value, so it always resolved to an opaque
`Value::Handle` and `apply_binding_value` rejected it *before* the
two-way registration code even ran. Every real binding elsewhere in
this codebase already requires `.get()` to extract a primitive (see
`test_view_binding.py`) — the fix matches that established convention:
the two-way check now requires `Expression::Call(Expression::Ident
(signal_name), "get")`, which both resolves to the real primitive
forward (identical to every other binding) and still names the exact
`Signal` to write back to.

**Real finding #2, caught only by actually running the new example
end to end, not by any unit test:** with finding #1 fixed, `checkbox.
set_checked(True)` fired `Change` → `TwoWayCallback` wrote `signal.
set(True)` → `Signal.set`'s own unconditional notify re-triggered the
widget's *own forward binding* → which called `set_checked(True)`
again → infinite recursion. Each level was individually caught and
printed by `call_handler`'s own pre-existing "an uncaught exception is
non-fatal" policy (§9), which is exactly why `pytest` never caught
it — the assertion the test cared about (`vm.agreed.get() is True`)
was already satisfied by the outermost call before the recursion
storm even started unwinding. The correct, general fix (not a
two-way-specific special case): `Signal.set`/`Signal.update` now skip
notification entirely when the new value equals the current one —
standard reactive-signal change-detection, and the natural place the
recursion above actually terminates (the *second* `set(True)` in the
loop is a no-op write to a `Signal` already holding `True`). Verified
in isolation with two new pure-Python `Signal` tests (no `View`/`Tree`
involved), a targeted `test_two_way_round_trip_does_not_recurse_
infinitely` asserting stderr stays completely clean (not just "the
final value is right"), and confirmed against every existing `.set()`/
`.update()` caller in the repo — none relied on always-notify
semantics.

New `engine-core` tests (`dispatch_release_ending_a_real_slider_drag_
produces_changed`, `dispatch_release_with_no_slider_drag_in_progress_
never_produces_changed`) proving the mechanical Slider path. New
`engine-spec` tests: `WidgetSpec`/`NodeKindSpec` parse real `Checkbox`/
`Slider` widgets with `checked`/`value`/`two_way` (plus their real
defaults when omitted); `load_view` builds real `NodeKind::Checkbox`/
`Slider` nodes correctly seeded from the spec; a colorless `Checkbox`
fails the same `MissingField` way `Rect`/`Text` already do. New
`tests/test_change_event.py` (5 tests): `get_checked`/`set_on_change`
FFI wiring, a real registered handler firing on `set_checked`, the
same "uncaught exception is caught and logged" policy already proven
for click/hover handlers. New `tests/test_two_way_binding.py` (5
tests): the real round trip both directions, the recursion regression
above, and both load-time errors (`two_way` on a computed expression;
an unmatched `two_way` staying a harmless no-op). New `tests/test_
view_binding.py` additions (2 tests): `Signal` change-detection in
isolation. New `examples/two_way_binding.py` + `two_way_binding.yaml`:
a real `Checkbox`/`Slider` view, both two-way bound, run with no live
window (matching `View`'s own "no render-loop concept" scope) — proves
the initial one-way apply from each `Signal`'s starting value and a
real write-back round trip after a real `Change`.

Full `cargo test --workspace --release` (`engine-core` 94, up from 92;
`engine-spec` 33, up from 29)/`cargo clippy --workspace --all-targets
-- -D warnings`/`cargo fmt --check` all clean — every prior test
(splitter, checkbox, slider, view-binding, cascade) passed unmodified.
`maturin develop --release` + full `pytest tests/` (131 passed, up
from 119, 1 skipped) and all twenty-two examples (twenty-one existing
+ new `two_way_binding.py`) confirmed clean, including a second full
run after the `Signal` fix.
