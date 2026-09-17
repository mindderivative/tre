# Log: M24 Phase 1 — Real Slider ArrowLeft/ArrowRight Increment, closing M24 (§10)

Corresponds to `BUILD_TRACKER.md` M24 Phase 1: a focused `NodeKind::
Slider` responds to `Key::ArrowLeft`/`ArrowRight` by nudging
`thumb_position` a real, fixed step.

## Investigation before writing code

ARCHITECTURE.md §10 Accessibility: "component-specific keyboard
semantics — arrow keys moving between options in a radio group, a
slider's arrow-key increments — are deferred to per-component design
in `engine-md3`, exactly when each such component is actually built."
`Slider` landed at M14 (§5, §7.3); this real, explicitly-deferred gap
was never revisited afterward, confirmed via grep — `Tree::dispatch`'s
`KeyPressed` arm had a real, dedicated `TextField`-only first-refusal
block, but `Key::ArrowLeft`/`ArrowRight` otherwise fell straight
through to a `DispatchOutcome::None` catch-all with no `Slider` case.

This M24 scoping itself followed a systematic, section-by-section
ARCHITECTURE.md re-audit run after M23 closed, checking every major
named v1 subsystem directly against the live source — §1 (icon
pipeline, M23), §5 (Image, M22), §7.6 (Container Transform, M7/M9),
§9 (Threading — all already-decided design, no build TODOs), §11.3
(the real generic overlay mechanism confirmed complete since M10;
`NodeKind::MenuBar`/`MenuItem`/`Menu` are illustrative usage examples
of that primitive, not §5-sketched core types), §11.6 ("no new
mechanism," already true), §16.3–16.7 (cascade/hot-reload/composition/
two-way bindings all confirmed built) — all found already complete.
`Slider`'s own arrow-key gap was the one real, narrow item left.

## What happened

New `Tree::dispatch_slider_key` — mirrors `dispatch_text_field_key`'s
own "focused node gets first refusal, `None` means not mine" contract
exactly, so `Tab`/`Enter`/`Escape` still fall through to the generic
handling for a focused slider too. `ArrowLeft`/`ArrowRight` only
(WCAG's own baseline "operate the value via keyboard" pair every
mainstream desktop slider supports) — deliberately not `Home`/`End`,
which §10's own text never names. Reuses `Tree::set_slider_position`
verbatim for the real clamp + immediate `Duration::ZERO` tick a mouse
drag already gets (the same "live-follows" semantics, not a second
mechanism), and returns `DispatchOutcome::Changed`, the identical
outcome a real drag-release already produces — so a two-way
`bindings: {value: ...}` write-back picks up an arrow-key nudge for
free, no new wiring anywhere. A fixed `0.05` step (5% of the track per
press), matching common desktop-slider convention; not made
configurable, since nothing yet demonstrates a need for a different
value.

**Real, necessary connected fix, found only by actually trying the
feature end to end from Python, not assumed:** `dispatch_slider_key`
is only ever reached when `self.focused == Some(slider)` — but
`Tree::collect_interactive`'s own real Tab-order predicate ("any node
with a non-empty `access.actions`") never included a `Slider` at all;
`Node.enable_interaction()` only ever sets ripple/hover tint, never
touches `access.actions`. So before this, a `Slider` could never
actually receive keyboard focus through the ordinary Tab mechanism —
the new arrow-key handling, while correctly implemented and provable
directly via `Tree::set_focus_to` in a Rust test, was unreachable from
a real running Python app. Fixed by giving `Window.add_slider` the
identical real `AccessNodeData::new(Role::Slider).with_action(Action::
Focus)` construction-time opt-in `add_text_field` already has — §10's
own "keyboard operability ships from day one" text, now actually true
for `Slider`, not just `TextField`.

New `engine-core` tests (4): `ArrowRight` on a focused slider increases
`thumb_position` by the real step and returns `Changed`; `ArrowLeft`
decreases it; a press at the real `0.0` minimum clamps rather than
going negative; the same keys with no slider focused remain the real,
already-established no-op. Updated `examples/slider.py`: a real Tab
press focuses the slider (now automatic at construction), two real
`ArrowRight` presses nudge it, the resulting real value read back and
checked stable across a full `App.run` loop.

Full `cargo test --workspace --release` (`engine-core` 144, up from
140), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo
fmt --check` all clean. `maturin develop --release` + `pytest tests/`
(187 passed, unchanged, 1 pre-existing skip — no new Python-visible
property was added) and all thirty-two examples run headlessly — the
same two pre-existing, unrelated `on_complete` failures already
flagged separately (`task_a5249ecc`), no new regressions.

M24 — Slider Keyboard Increments is now fully complete.
