# Plan: M4 Phase 1, Step 1 — Real Input Dispatch Core (§4, §7.3, §9, §10, §11.10)

M3 (all 15 of §14's Suggested Build Order steps) is complete. No M4 is
sequenced anywhere in `ARCHITECTURE.md` — this is the first work on it,
picking up the single most consistently recurring finding from every
interaction-dependent step in M3 (7, 9, 11, 12, 13, 14, 15, each checked
directly): real pointer/keyboard `InputEvent`/`AppHandler` dispatch and
hit-testing don't exist anywhere in this codebase. Every one of those
steps exposed a direct, programmatic `Tree` method instead (`spawn_ripple`,
`open_overlay`, `set_splitter_position`, `apply_active_tab`,
`set_virtual_list_window`) and explicitly deferred "wire this to a real
input source" to whichever step first built dispatch. This is that step.

`interaction.rs`'s and `access.rs`'s own module doc comments both name
this exact gap and its exact blocking reason ("nothing to dispatch to
yet") — both now have a real target: the button built in M3 step 7
(`AccessNodeData::new(Role::Button).with_label(...).with_action(Action::Click)`).

## Scope for this step

Core mechanism only, entirely inside `engine-core` — no `winit`, no
`pyo3` yet. Real winit event translation (`engine-platform`) and a real
`AppHandler` impl enabling `Node.set_on_click` (`engine-py`) are each
their own later step, once this core is proven standalone (Design
Principle 5).

**`engine-core/src/input.rs` (new)**: `InputEvent` (`PointerMoved`/
`PointerPressed`/`PointerReleased` with `position: kurbo::Point` +
`button: PointerButton`; `KeyPressed`/`KeyReleased` with a minimal `Key`
enum — `Tab`/`Enter`/`Space`/`Escape` only, matching §10's own stated
minimal keyboard model exactly, not a full text-input key-code mapping
nothing here needs yet) and `AppHandler` (the generic trait §4 names,
kept to the one meaning-dependent outcome this step produces —
activation — everything mechanical stays inside `Tree` itself per
Design Principle 6, not pushed onto `AppHandler` implementors).

**`Tree` gains, in `tree.rs`**:
- `hit_test(root, point) -> Option<NodeId>` (§11.10): reverse paint-order
  walk (last child first — topmost), rect containment via existing
  `absolute_position`/`layout().size`. **Explicitly narrowed, stated
  here rather than silently skipped:** no transform-aware hit-testing
  (§11.9's inverse-transform step) — `PaintProperties.transform` doesn't
  exist yet (`node.rs`'s own doc comment defers it); no `NodeKind::Canvas`
  custom hit-test override — `Canvas` doesn't exist yet either. Both
  revisit this method once their own prerequisite lands, per Design
  Principle 5.
- `update_hover(root, point, hover_opacity, duration, now) -> Option<NodeId>`:
  the concrete fulfillment of §7.3's own text ("hover needs no new
  dispatch mechanism — it falls out of hit-testing, run every
  pointer-move... entirely inside `engine-core`"). Only animates nodes
  that already opted into `InteractionState` (Design Principle 6's "only
  a node that opts in pays the cost") — never lazily creates it, unlike
  `interaction_mut`. `hover_opacity`/`duration` are caller-supplied, not
  hardcoded: `engine-core` stays MD3-agnostic (§1 Locked Decisions) —
  the actual MD3 hover value is `engine-md3`'s to supply later, the same
  generic/preset split `MotionCurve`/`engine_md3::motion::STANDARD`
  already uses.
- `move_focus(root, direction, focus_ring_opacity, duration, now)`:
  Tab/Shift-Tab in tree order (§10) — "interactive" means
  `access.actions` is non-empty (the real, already-existing signal, no
  new field needed), wrapping at both ends. Animates `focus_ring` the
  same opt-in-only way `update_hover` animates `hover_opacity`.
- `dispatch(root, event, config: &InteractionConfig, now) -> DispatchOutcome`:
  the one real top-level entry point `engine-platform` will call.
  `DispatchOutcome::Activated(NodeId)` is the single meaning-dependent
  outcome (a primary-button click released over the same node it was
  pressed on, or Enter/Space on the focused node) — `AppHandler`'s job,
  later, is entirely "what does activating this node mean" (call a
  registered `on_click` or nothing); everything mechanical (hover,
  focus movement, ripple-spawn-on-press) happens inside `dispatch`
  itself before it ever reaches `AppHandler`.

**Real API-drift finding, verified directly (not assumed) before
writing any code:** §10's own text says Enter/Space "dispatches
`accesskit::Action::Default`" — checked directly against the pinned
`accesskit = "0.25.0"`'s real `Action` enum (`accesskit-0.25.0/src/
lib.rs`): there is no `Action::Default` variant. The real, equivalent
variant is `Action::Click` ("do the equivalent of a single click or
tap") — already the exact action M3 step 7's own button test uses. This
codebase's own `AccessNodeData::with_action(Action::Click)` is what
"interactive" (focusable) actually keys off, so this isn't a new
convention, just making an existing one do real work.

**Ripple stays single-shot for this step**, not upgraded to
`interaction.rs`'s own described two-phase press/hold/release model —
that's a real, separate scope of its own (tracking *which* ripple is
still "held down" across a press/release pair, MD3's actual timing
curves) genuinely bigger than "make real events reach the tree at all,"
which is this step's actual claim. Stated here, not silently dropped.

## Verification

New `engine-core` unit tests, each isolating one claim: `hit_test`
picks the topmost of two overlapping siblings and correctly reaches
into an appended overlay over background content (reusing step 13's
own overlay-proof shape); `update_hover` only animates nodes that
already opted into `InteractionState`, and correctly fades the old node
out while fading the new one in on a real hover-target change;
`move_focus` cycles through exactly the nodes with a non-empty
`access.actions` list, in tree order, wrapping at both ends, leaving
non-interactive nodes untouched; `dispatch` produces `Activated` only
for a same-node press+release pair with the primary button, and `None`
for a press/release over different nodes (a drag-off, not a click) and
for every non-primary button. `cargo test --workspace`, `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean.

## Next (not this step)

`engine-platform`: translate real `winit` `CursorMoved`/`MouseInput`/
`KeyboardInput` events into `InputEvent` and call `Tree::dispatch`,
proven via a real `harness = false` test. `engine-py`: implement
`AppHandler` for real — this is what finally lets `Node.set_on_click`
exist (still just a forward-reference comment in `node.rs` today),
needing the same real `PyObject`-callback-storage + `__traverse__`/
`__clear__` GC-cycle-safety discipline M3 step 15 Stage C's materializer
callback just established for `PyWindow`.
