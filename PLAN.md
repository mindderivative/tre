# Plan: M4 Phase 6 — `EventKind` + Real `HoverEnter`/`HoverExit` (§7.3, §16.2)

## Context

§7.3: "The engine also fires an optional `HoverEnter`/`HoverExit`
`EventKind` (§16.2) through the ordinary handler path for the rare case
an app wants to react to hovering itself (a delayed tooltip, say) — the
default MD3 visual never depends on anything handling it." No
`Event`/`EventKind` type exists anywhere yet (confirmed via grep before
Phase 4 even started). This phase introduces the minimal real version
and wires hover transitions through it.

## Investigation before writing code

- **`AppHandler` (`engine-core/src/input.rs`) is dead code.** Confirmed
  via grep: declared, documented, re-exported — never `impl`'d anywhere.
  The real mechanism that shipped instead (M4 Phase 1 steps 2-3) is a
  plain `on_input: FnMut(WindowId, InputEvent)` closure on
  `run_windowed_multi` plus `engine-py`'s own `dispatch::run_activation`
  interpreting `DispatchOutcome` directly. Not touched by this phase
  (out of scope — noted honestly in `BUILD_TRACKER.md`, not silently
  left as a misleading claim), but worth knowing before extending the
  same enum it references.
- **The real click-handling call graph, re-traced:** `Tree::dispatch`
  returns a `DispatchOutcome`; exactly one function,
  `dispatch::run_activation`, interprets it into a real Python call;
  it's invoked from exactly three real call sites — `app.rs`'s
  `on_input` closure (the real `winit`-driven path, already receiving
  *every* `InputEvent`, `PointerMoved` included), `Window.click()`/
  `Window.press_key()`, and `View.click()`. Extending `DispatchOutcome`
  with a new variant and generalizing `run_activation` to interpret it
  reaches all three call sites for free — the same "one mechanism, many
  real call sites" shape this project keeps reusing (splitters,
  overlays, `set_on_click` itself).
- **Hover transition tracking already happens unconditionally.**
  `Tree::dispatch`'s `PointerMoved` arm calls `update_hover`, which
  compares the hit-test result against `self.hovered` regardless of
  whether either node ever opted into `InteractionState` — only the
  *animation* is gated on opt-in. This matches §7.3's own text exactly:
  the event should fire independently of whether the default MD3 visual
  is enabled, so `HoverEnter`/`HoverExit` firing needs no new gating
  logic, just surfacing the transition `update_hover` already computes
  but currently discards.

**Real design decision, resolved: generalize the handler storage now,
not duplicate it.** With three real event kinds to support (`Click`,
`HoverEnter`, `HoverExit`), duplicating `click_handlers`'s exact shape
two more times (`hover_enter_handlers`, `hover_exit_handlers`) would add
two new `Rc<RefCell<HashMap<...>>>` fields to `Node`/`PyWindow`/`View`/
`WindowSetup`/`WindowRuntime` apiece. Instead, `click_handlers` becomes
`handlers: Rc<RefCell<HashMap<(NodeId, EventKind), Py<PyAny>>>>` — one
field, re-keyed. This is the Rule of Three, not premature abstraction:
duplicating a two-field shape once (`Node`'s existing precedent, one
field) is fine; duplicating it a second and third time for the same
underlying "which callback fires for this node+event" concept is the
point where the generalization pays for itself.

**Handlers stay zero-argument**, matching Phase 4's already-established,
deliberately narrow convention (`Node.set_on_click`'s existing
`lambda: ...`/`def bump(self): ...` shape) — a real `Event` struct
carrying `source`/`data` (§16.2's own fuller sketch) is deferred until a
real handler needs the extra context; nothing today does.

## Approach

1. **`engine-core`**: `EventKind { Click, HoverEnter, HoverExit }`
   (`Clone, Copy, PartialEq, Eq, Hash`) in `input.rs`. `DispatchOutcome`
   gains `HoverChanged { old: Option<NodeId>, new: Option<NodeId> }`.
   `Tree::dispatch`'s `PointerMoved` arm captures `self.hovered` before
   calling `update_hover`, and returns `HoverChanged` when the result
   actually differs (a genuine transition), `None` otherwise (an
   unchanged hover is not a new fact to report, matching
   `update_hover`'s own "repeated call, same result, is a no-op").
2. **`engine-py`**: rename `click_handlers` → `handlers` (re-keyed by
   `(NodeId, EventKind)`) across `Node`, `PyWindow`, `View`,
   `WindowSetup`, `WindowRuntime`. `Node.set_on_click` inserts at
   `(id, EventKind::Click)` (behavior unchanged). New
   `Node.set_on_hover_enter`/`set_on_hover_exit` insert at the matching
   key. `dispatch::run_activation` → `dispatch::run_dispatch_outcome`,
   extended to also interpret `HoverChanged` (fire the old node's
   `HoverExit` handler if any, the new node's `HoverEnter` handler if
   any). New `Window.hover(node)`/`View.hover(node)` — the same
   no-live-window-needed proof pattern `.click()` established, this
   time dispatching a `PointerMoved` at the node's own center.
   `View::_attach`'s handler loop generalizes its single
   `if event == "on_click"` check into also recognizing
   `"on_hover_enter"`/`"on_hover_exit"`.
3. **Tests**: `engine-core` unit tests for `DispatchOutcome::
   HoverChanged` (mirroring the existing `dispatch_activates_only_...`
   test shape). New `tests/test_hover_events.py`: a real
   `Window.hover(node)`/`View.hover(node)` call actually invokes a
   registered `on_hover_enter`/`on_hover_exit` handler; hovering off a
   node fires its exit handler; a node with no hover handler registered
   is a safe no-op (matching `set_on_click`'s own precedent).

## Files to touch

- `crates/engine-core/src/input.rs` — `EventKind`, `DispatchOutcome::
  HoverChanged`.
- `crates/engine-core/src/tree.rs` — `dispatch`'s `PointerMoved` arm,
  new unit tests.
- `crates/engine-py/src/node.rs`, `window.rs`, `view.rs`, `app.rs`,
  `dispatch.rs` — the `handlers` rename + hover wiring described above.
- `tests/test_hover_events.py` — new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
