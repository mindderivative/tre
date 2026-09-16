# Log: M4 Phase 6 — `EventKind` + Real `HoverEnter`/`HoverExit` (§7.3, §16.2)

Corresponds to `BUILD_TRACKER.md` M4 Phase 6. Introduces the minimal
`Event`/`EventKind` vocabulary §16.2 names but that never existed
anywhere in this codebase (confirmed via grep before M4 Phase 4 even
started), and wires `HoverEnter`/`HoverExit` (§7.3) through it.

## Investigation before writing code

- **`AppHandler` (`engine-core/src/input.rs`) is dead code.** Declared,
  documented, re-exported — never `impl`'d anywhere (confirmed via
  grep). The real mechanism that shipped instead (M4 Phase 1) is a
  plain `on_input` closure on `run_windowed_multi` plus `engine-py`'s
  own interpretation of `DispatchOutcome`. Not touched this phase —
  noted here honestly rather than silently left as a misleading claim,
  since this phase extends the very enum that comment references.
- **Traced the real click-handling call graph:** exactly one function
  interprets `DispatchOutcome` into a real Python call
  (`dispatch::run_activation`), called from exactly three real sites —
  `app.rs`'s `on_input` closure (already receiving every `InputEvent`,
  `PointerMoved` included), `Window.click()`/`press_key()`, and
  `View.click()`. Extending the outcome enum and generalizing that one
  interpreter reaches all three for free.
- **Hover transition tracking already happens unconditionally.**
  `Tree::dispatch`'s `PointerMoved` arm calls `update_hover`, which
  compares against `self.hovered` regardless of whether either node
  ever opted into `InteractionState` (M4 Phase 5's `enable_interaction`)
  — only the *visual animation* is gated on opt-in. This matches §7.3's
  own text exactly: the event should fire independent of whether the
  default MD3 visual is enabled, confirmed directly against the real
  code rather than assumed.

**Real design decision, resolved: generalize handler storage now, via
the Rule of Three.** With `Click`/`HoverEnter`/`HoverExit` all needing
the same "look up a registered handler for this node, call it" shape,
duplicating `click_handlers`'s exact shape two more times would have
added two more `Rc<RefCell<HashMap<...>>>` fields apiece to `Node`/
`PyWindow`/`View`. Instead, `click_handlers: HashMap<NodeId, Py<PyAny>>`
became `handlers: HashMap<(NodeId, EventKind), Py<PyAny>>` — one field,
re-keyed, aliased as `dispatch::HandlerMap` once clippy's own
`type_complexity` lint (correctly) flagged the raw type as too complex
to repeat five times across the crate.

## What happened

`engine-core`: new `EventKind { Click, HoverEnter, HoverExit }` and
`DispatchOutcome::HoverChanged { old, new }`. `Tree::dispatch`'s
`PointerMoved` arm now captures `self.hovered` before calling
`update_hover` and reports a real transition when the result actually
differs, `None` otherwise (matching `update_hover`'s own "repeated
call, same result, is a no-op" contract — an unchanged hover isn't a
new fact to report).

`engine-py`: renamed `click_handlers` → `handlers` (re-keyed) across
`Node`, `PyWindow`, `View`, `WindowSetup`, `WindowRuntime`.
`dispatch::run_activation` → `dispatch::run_dispatch_outcome`, now
interpreting both `Activated` (unchanged) and `HoverChanged` (fires the
old node's `HoverExit` handler if any, the new node's `HoverEnter`
handler if any). New `Node.set_on_hover_enter`/`set_on_hover_exit` —
thin inserts into the same map `set_on_click` already uses, kept as
separate named methods rather than one generic `set_handler(kind, cb)`
(no real caller needs the generic form yet). New `Window.hover(node)`/
`View.hover(node)`, mirroring `.click()`'s own no-live-window-needed
proof pattern exactly, dispatching a `PointerMoved` instead of a
press/release pair. `View::_attach`'s single `on_click`-only special
case generalized into a small match recognizing `on_click`/
`on_hover_enter`/`on_hover_exit`, reusing `Node`'s own setters verbatim
(the same construction `apply_binding_value` already established for
`animate`) rather than inserting into the map directly, so
`set_on_click`'s real side effect (adding `Action::Click` to
`access.actions`, §10) isn't silently lost for the `View` path.

Handlers stay zero-argument, matching the established
`Node.set_on_click`/`run_dispatch_outcome` convention — a real `Event`
struct carrying `source`/`data` (§16.2's own fuller sketch) is deferred
until a real handler needs the extra context; nothing today does.

## Real bug found and fixed during the rename

The blind `s/click_handlers/handlers/g` sed pass created a genuine
field-name collision in `View`: the struct already had an unrelated
`handlers: Vec<(String, String, String)>` field (the parsed
`{event, method_name}` declarations from the YAML) before this phase's
rename landed the *second* `handlers` field on top of it. The compiler
caught it immediately (duplicate struct field), not a silent bug — but
worth naming: it wasn't spotted by inspection first. Fixed by renaming
the pre-existing Vec field to `declared_handlers`, which is also a more
accurate name (it's what's declared in YAML, not what's actually
registered).

## Verification

1 new `engine-core` unit test (`dispatch_reports_hover_changed_only_on_a_real_transition`,
mirroring the existing `dispatch_activates_only_...` test's shape):
first entry onto a node reports `HoverChanged`, a repeated move within
the same node reports `None`, moving directly to a sibling reports both
halves in one call, and leaving every node reports the exit half. 5 new
pytest tests in `test_hover_events.py`: `HoverEnter` fires on real
arrival, `HoverExit` fires on a real departure to a sibling, hovering a
node with no handler is a safe no-op, hover fires even without
`enable_interaction()` (proving §7.3's "independent of the visual" text
for real), and `View.hover()` actually invokes a wired
`on_hover_enter` handler. All passed on the first run. Full pre-existing
suite (49 passed, 1 skipped) and all four examples unaffected by the
internal rename.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean (after adding HandlerMap alias)
$ cargo fmt --check                                           # clean
$ cargo test --workspace                                      # all green, 1 new engine-core test included

$ maturin develop
$ python -m pytest tests/ -v
49 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py       # exited cleanly, unaffected
$ python examples/resizable_panes.py    # exited cleanly, unaffected
$ python examples/ripple_button.py      # exited cleanly, unaffected
$ python examples/two_windows.py        # exited cleanly, unaffected
```

## Next

M4 Phase 7: right-click context menus (§11.3) — `PointerButton::
Secondary` exists and is unit-tested to never start a drag, but nothing
acts on it yet. Real, stated-not-silent gaps unchanged: only `Click`/
`HoverEnter`/`HoverExit` are wired (a `Change`/`Focus` `EventKind`
would need the same small-match generalization, added only when a real
bound component needs one); handlers stay zero-argument, no real
`Event` object exists yet; `AppHandler` remains real, confirmed dead
code, untouched.
