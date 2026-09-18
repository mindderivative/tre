# Log: M30 Phase 1 Step 1 — `Button`

Scoped as the first real component of M30's 10-phase catalog: MD3's
five real button variants (Elevated/Filled/Filled Tonal/Outlined/Text),
the one component confirmed hand-composed from `Rect`+`Text`+ripple in
every existing example.

## Real anatomy chosen

A `Rect` container (corner radius `height / 2.0` — MD3's own "Full"
shape family) with one centered `Text` label child, matching the
existing hand-composed pattern but built once, not per call site.
Returns the container `Node` so `set_on_click`/`enable_interaction`/
`animate` all work on it exactly like any other node — no new API
surface needed for those.

## Two real engine gaps, found only by actually building this

1. **No border capability at all.** `engine-render` had no way to
   paint anything but a solid fill — the Outlined variant's real 1dp
   stroke was impossible without new machinery. Added universally:
   `PaintProperties.border_color`/`border_width` (true-no-op defaults,
   confirmed no other file does direct `PaintProperties { ... }`
   construction, so this is safe for all ~60 existing call sites),
   painted in `paint_node` inset by half the stroke width so it never
   expands the node's own layout box. Proven with a real headless
   pixel-readback test (`border_paint.rs`): the border color inside
   the stroke band, the fill still intact at the center, and
   `border_width: 0.0` a genuine no-op across the whole surface.

2. **No text-alignment capability at all.** `text.rs` hardcoded
   `parley::Alignment::Start` unconditionally — every button label
   would have rendered flush-left, not centered. Added a universal
   `TextState.align: TextAlign` (Start/Center/End), threaded through
   `LayoutCacheKey` so the shaping cache correctly invalidates on
   alignment change. Required touching every existing `TextState {
   ... }` literal across the workspace (9 sites, no `::new()`
   constructor exists) — mechanical, compiler-enforced, all kept at
   `TextAlign::Start` to preserve exact prior behavior. Proven with a
   real pixel-readback test (`text_align.rs`): `Center` genuinely
   moves ink off the left edge toward the middle; `Start` still hugs
   the left edge exactly as before.

## A real, confirmed bug — not a design choice

`tests/test_button.py`'s own click-dispatch test failed on first run:
a real click on the button never reached its registered handler.
Traced to `Tree::hit_test_at`: it recurses into children first with no
ancestor bubbling anywhere in `dispatch`, so the button's own centered
`Text` child — sized to fill the container's inner content width —
silently claimed the hit and the container's handler was unreachable.
Confirmed via grep first that no existing example or test anywhere
relies on a standalone `add_text` node being independently clickable,
then fixed at the root: a bare `NodeKind::Text` label never
independently claims a hit any more, always deferring to whatever's
behind it. A future standalone clickable label (MD3's own `Link`,
Phase 8) gets its own dedicated `NodeKind` when that phase starts, the
same "each interactive component is its own real `NodeKind`" precedent
`Checkbox`/`Slider`/`TextField` already establish.

## Un-themed default

Real Material 3 baseline hex tokens (`ButtonBaseline` in
`window_factory.rs`), the same "real historical default, not black"
contract `Checkbox`'s white mark / `Slider`'s gray track already
establish for a `Window` that never calls `set_theme`. A themed
`Window` resolves through the new general `ThemeState::role(name)`
instead, widened beyond the old single-field `on_surface()`.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 39 binaries, all green (`border_paint.rs`
and `text_align.rs` included). `maturin develop --release` rebuilt.
`pytest tests/`: 197 passed, 1 skipped (10 new in `test_button.py`,
zero regressions from the hit-test change). All 31 examples and the
showcase demo re-run clean. `mypy --strict` clean against
`examples/button.py`, plus a separate deliberate-error probe (a wrong
argument type and a missing required argument, both suppressed with
`# type: ignore`) confirming `--warn-unused-ignores` stayed silent —
proof the new `.pyi` stub entry carries real type information, not
just a permissive stand-in.
