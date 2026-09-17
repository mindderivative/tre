# Plan: M24 Phase 1 — Real Slider ArrowLeft/ArrowRight Increment (§10), closing M24

Corresponds to `BUILD_TRACKER.md` M24 Phase 1: a focused `NodeKind::
Slider` responds to `Key::ArrowLeft`/`ArrowRight` by nudging
`thumb_position` a real, fixed step.

## Investigation before writing code

- ARCHITECTURE.md §10 Accessibility: "component-specific keyboard
  semantics — arrow keys moving between options in a radio group, a
  slider's arrow-key increments — are deferred to per-component
  design in `engine-md3`, exactly when each such component is
  actually built." `Slider` landed at M14 (§5, §7.3); this real gap
  was never revisited, confirmed via grep — `Tree::dispatch`'s own
  `KeyPressed` handling has no `Slider` case at all; `Key::ArrowLeft`/
  `ArrowRight` fall straight through to a `DispatchOutcome::None`
  catch-all whenever no `TextField` is focused.
- **Real, confirmed reachability:** a `Slider` can already become
  genuinely focusable via the existing, generic `Node.
  enable_interaction()` opt-in — `collect_interactive`'s own real
  predicate (`Tree::collect_interactive`, confirmed via direct read)
  is "any node with a non-empty `access.actions`," not a `TextField`-
  only special case. So this is a real, live, currently-reachable gap
  for any app that already calls `enable_interaction()` on a slider,
  not a hypothetical one.
- **Real precedent to mirror:** `TextField`'s own "focused node gets
  first refusal on most keys" block, right at the top of the
  `KeyPressed` arm (`if let Some(field) = self.focused && matches!(...
  TextField...) && let Some(outcome) = self.dispatch_text_field_key(...)
  { return outcome; }`) — `dispatch_text_field_key` returns `None` for
  any key it doesn't own, meaning "not mine, let the generic `match
  key` below still run" (so `Tab`/`Escape` keep working for a focused
  field). A new `dispatch_slider_key` follows the identical contract
  for `Slider`.
- **Real mechanism reuse, not a new one:** `Tree::set_slider_position`
  already exists (used by a real mouse drag) and already does exactly
  what an arrow-key nudge needs — clamp to `0.0..=1.0`, then an
  immediate `Duration::ZERO` `animate_to` + manual `tick` (the same
  "live-follows" semantics a drag has, confirmed via direct read).
  Reused verbatim rather than duplicating the clamp/tick logic.
- **Real, deliberate scope boundary:** `ArrowLeft`/`ArrowRight` only —
  WCAG's own baseline "operate the value via keyboard" requirement,
  and the identical increment/decrement pair every mainstream desktop
  slider widget supports. Not `Home`/`End`-to-extremes, which §10's
  own text never names and no current example/component needs
  (matches this codebase's own "don't build ahead of need" discipline
  throughout). A fixed `0.05` step (5% of the track per press) — a
  real, reasonable default matching common GUI convention; not made
  configurable, since nothing yet demonstrates a real need for a
  different value.
- **Real outcome contract:** returns `DispatchOutcome::Changed(id)` —
  the identical outcome a real drag-release on a slider already
  produces (M14 Phase 3, §16.7), so any caller already wired to react
  to a slider's value settling (e.g. a two-way `bindings: {value:
  ...}` write-back) picks up an arrow-key nudge for free, no new
  wiring needed anywhere.

## What will change

- `crates/engine-core/src/tree.rs`: new `dispatch_slider_key(&mut
  self, id: NodeId, key: Key, now: Instant) -> Option<DispatchOutcome>`
  (mirrors `dispatch_text_field_key`'s own contract); a new focused-
  Slider first-refusal block in `dispatch`'s `KeyPressed` arm, right
  after the existing `TextField` one; the final catch-all match arm's
  own doc comment updated to note `Slider` is now handled above it,
  not silently still a no-op for every focused node.
- New `engine-core` tests: `ArrowRight` on a focused slider increases
  `thumb_position` by the real step and returns `Changed`; `ArrowLeft`
  decreases it; clamping at `0.0`/`1.0` (an already-at-the-limit press
  doesn't go out of range, and — real, worth checking — still returns
  `Changed` or correctly becomes a no-op, whichever `set_slider_
  position`'s own real clamp-then-set behavior actually produces);
  the same key on an unfocused/non-Slider-focused tree is a true
  no-op, matching the existing catch-all's own established coverage.
- Updated `examples/slider.py`: a real `enable_interaction()` +
  focus + arrow-key nudge, proving the real, live, end-to-end path
  (matching this project's own "every mechanism gets a real running
  proof" discipline).

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run every example script.
