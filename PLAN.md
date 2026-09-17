# Plan: M14 Phase 1 — Real Checkbox (§5, §7.3)

Corresponds to `BUILD_TRACKER.md` M14 Phase 1's own scoping:
`NodeKind::Checkbox(CheckboxState)` with real paint and real
click-to-toggle interaction, reusing already-real `Click`
dispatch/ripple.

## Investigation before writing code

- ARCHITECTURE.md §5's own sketch shows only `check_progress:
  Animated<f64>` for `CheckboxState`, but §7.3's own text separately
  names `CheckboxState.checked` as a real field ("a well-known field
  name on a `NodeKind` payload (a `TabState.selected`, a `CheckboxState
  .checked`) derives its corresponding `AccessStates` flag") — the
  struct needs both: `checked: bool` (plain app-owned state, per
  Design Principle 6 — "selection/checked-state... depend on what the
  app's data means") and `check_progress: Animated<f64>` (the
  engine-driven visual consequence).
- **Design Principle 6 confirms the engine must not auto-toggle
  `checked` on click** — that's meaning-dependent, app-owned. The
  "click-to-toggle interaction" this phase's own scoping names is
  therefore: the app's own `on_click` handler flips `checked` and
  animates `check_progress`; the *mechanism* this phase reuses
  unchanged is the already-fully-generic `Click` dispatch + `Node.
  enable_interaction()`'s ripple/state-layer (works for any `NodeKind`
  already, confirmed via direct read — zero new dispatch code needed).
- `Tree::tick_all` (`tree.rs:909-923`, confirmed by direct read) only
  ticks `node.paint`/`node.interaction` today — no kind-specific
  payload is ticked centrally (`SplitterState.position`/`VirtualList
  State.scroll_offset` are deliberately driven directly, never eased).
  `check_progress` *is* meant to animate-to-target (ARCHITECTURE.md's
  own "animated through the same `Animated<T>` mechanism once set"),
  so `tick_all` needs one new arm for `NodeKind::Checkbox`.
- `AccessStates` (`access.rs`, confirmed by direct read) only carries
  `disabled` today — that module's own doc comment already states why:
  "no states derived automatically from a `NodeKind` payload field
  yet... §7.3's interaction components land at later build-order
  steps." This is that step. `accesskit` 0.25.0 (pinned, confirmed via
  direct read of its own source) has a real `Toggled { False, True,
  Mixed }` + `Node::set_toggled`, with `From<bool> for Toggled` — the
  real mechanism `build_access_update` needs, not a new engine
  invention.
- `Node.animate()`/`Node.get()` (`node.rs`, confirmed by direct read)
  only dispatch against `PaintProperties`' own universal fields today
  — the "two-level dispatch" ARCHITECTURE.md §8 describes (`property`
  against `PaintProperties` first, then the node's own `NodeKind`
  payload) isn't built yet. This phase adds the first real kind-payload
  arm (`"check_progress"`), confirmed as genuinely new work.
- `paint_node` (`engine-render/lib.rs:362-441`, confirmed by direct
  read) already has the exact per-`NodeKind` match shape a `Checkbox`
  arm slots into; `Canvas`'s own `DrawCommand::StrokePath` arm is the
  direct template for stroking a real checkmark path.

## Design

`crates/engine-core/src/access.rs`: `AccessStates` gains `checked:
bool`.

`crates/engine-core/src/node.rs`: `NodeKind` gains `Checkbox
(CheckboxState)`; new `CheckboxState { checked: bool, check_progress:
Animated<f64> }` with a `new(checked: bool) -> Self` constructor
(`check_progress` starts at `1.0`/`0.0` matching `checked`, so a
checkbox created already-checked doesn't need a real animation just to
show its own initial state).

`crates/engine-core/src/tree.rs`:
- `tick_all` gains one new arm: `NodeKind::Checkbox(state) => state.
  check_progress.tick(now, &mut completed)`, folded into the existing
  `any_active` accumulation.
- `build_access_update` gains: if `node.kind` is `Checkbox(state)`,
  `access_node.set_toggled(state.checked.into())` — the real, automatic
  derivation ARCHITECTURE.md §7.3 promises.

`crates/engine-render/src/lib.rs`: `paint_node` gains a `NodeKind::
Checkbox(state)` arm — the box itself via the same rounded-rect fill
`Rect`/`Splitter` already use, then a real checkmark `BezPath` (a
fixed, node-local tick shape) stroked with opacity `state.check_
progress.current` (a plain white mark — this codebase's own established
"real but not yet theme-aware, wire theme later when a real need
arises" precedent, the same shape ripple's own hardcoded tint had
before M7 Phase 3).

`crates/engine-py/src/node.rs`: `Node.animate("check_progress", ...)`/
`Node.get("check_progress")` reach `CheckboxState.check_progress` when
`node.kind` is `Checkbox`; new `Node.set_checked(checked: bool)` — a
plain, non-animated write to `CheckboxState.checked` (mirroring the
"engine owns the mechanism, app owns the meaning" split this phase's
own investigation confirmed).

`crates/engine-py/src/window.rs`: new `Window.add_checkbox(checked=
False, width=.., height=.., background=.., x=None, y=None) -> Node`,
mirroring `add_rect`'s own real shape exactly.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New `engine-core`
  tests: `tick_all` genuinely animates `check_progress` toward a real
  target over real ticks; `build_access_update` reports the real
  `Toggled` state matching `checked`. New `engine-render` pixel test
  (mirroring `canvas_paint.rs`'s own established split): a real,
  fully-checked checkbox paints a real, visible checkmark; an unchecked
  one (`check_progress` at `0.0`) paints none.
- `maturin develop --release` + `pytest tests/`. New `test_checkbox.py`:
  `add_checkbox` returns a real `Node`; `set_checked`/`animate(
  "check_progress", ...)`/`get("check_progress")` all reach the real
  state (checked via a real functional proof where one exists, `get`
  otherwise); the existing generic `Click`/ripple mechanism already
  works unmodified on a `Checkbox` (a real click fires a registered
  handler, the same "clicking it proves it" discipline this suite
  already uses).
- Run all examples; a new `examples/checkbox.py` demonstrating a real,
  live, click-toggled checkbox with its own real animated checkmark.
