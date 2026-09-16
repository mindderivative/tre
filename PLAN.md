# Plan: M6 Phase 2 — `transform` exposure from Python

## Context

Confirmed directly (`engine-py/src/node.rs`): `Node.animate()`'s real
property list is `opacity`/`corner_radius`/`elevation`/`background`
only — no `transform`. M5 Phase 1 built `PaintProperties.transform:
Animated<kurbo::Affine>` and the whole real composition mechanism, but
nothing in `engine-py` ever reaches it. This is the exact gap M5 Phase
4 hit while trying to write a live-animated Python pan/zoom example.

## Investigation before writing code

- **The Python-facing representation has to match `Interpolate for
  Affine`'s own real, already-stated limitation, not expose more than
  it correctly supports.** M5 Phase 1's own `Interpolate` impl is a
  plain componentwise coefficient lerp — exact only for the convex
  subspace of affines with no rotation/shear (uniform scale + translate,
  `a == d`, `b == c == 0`). A raw 6-coefficient tuple would let a Python
  caller construct a rotated/sheared `Affine`; animating between two
  such values would then visibly "morph" rather than sweep through a
  correct arc — the exact named limitation M5 Phase 1's own doc comment
  already flags, but currently unreachable from Python at all, so never
  actually exercised by a real caller. **Scope narrowed, correctly, to
  match the framework's own real interpolation guarantee:** expose
  `transform` as a `(translate_x, translate_y, scale)` 3-tuple —
  "pan offset × zoom scale," §11.9's own text, verbatim — composed as
  `Affine::translate((tx, ty)) * Affine::scale(scale)`, the exact
  product M5 Phase 1's own pixel test already used. This isn't a
  reduced convenience shape layered over a fuller one; it's the actual
  boundary of what this framework's transform animation is correct for
  today. A rotation-capable API is additive whenever `Interpolate`
  itself gets the real SVD-based decomposition M5 Phase 1's own
  `LOG.md` named as the (currently unbuilt) alternative.
- **`Node.get()` doesn't need a `"transform"` case.** It only ever
  returns `f64` (`background` was already excluded from it for the same
  reason — "isn't a single f64, and nothing yet needs to read it back").
  Consistent, not a new decision.
- **Corrected mid-plan, before writing the wrong test:** this plan
  originally called for a new `engine-render` pixel test proving a live
  Python `animate("transform", ...)` call moves a rendered pixel.
  Investigation found that's not actually this project's established
  pattern for *any* `animate()`-settable property — confirmed via grep,
  no Python-level test anywhere reads back a rendered pixel or even a
  non-`f64` property's applied value (`background`'s own `animate()`
  path has never been pixel- or value-verified from Python either, the
  same precedent `Node.get()`'s own doc comment already states). The
  underlying paint mechanism for `PaintProperties.transform` is already
  exhaustively pixel-proven at the Rust level (M5 Phase 1's
  `transform_composition.rs`) — Phase 2 adds a new *write path* to that
  same field, not new paint/hit-test logic, so there is nothing new for
  a pixel test to prove. Building embedded-Python-interpreter Rust
  tests to force a pixel readback from the FFI layer would be real,
  disproportionate new test infrastructure this project has never
  needed for any other property — not built here either.

## Approach

1. **`engine-py/src/node.rs`**: new `extract_translate_scale(to,
   property) -> Result<(f64, f64, f64), EngineError>`, mirroring
   `extract_f64`/`extract_color`'s exact shape. New `"transform"` arm in
   `animate()`'s match: extracts `(tx, ty, scale)`, builds
   `Affine::translate((tx, ty)) * Affine::scale(scale)`, calls
   `node.paint.transform.animate_to(...)`.
2. **New pytest tests**, matching the same FFI-wiring-only scope every
   other non-`f64` `animate()` property already has: a real call with a
   valid 3-tuple doesn't raise; a bad-shape argument raises a clear
   `TypeError`.
3. **New example** showing a real, live-animated camera pan — the
   concrete thing M5 Phase 4 couldn't build (no Python `transform`
   exposure existed), now buildable; the actual, honest visual proof a
   human can run, matching this project's own "the live example is the
   real proof of visible behavior" pattern used throughout.

## Files to touch

- `crates/engine-py/src/node.rs` — `extract_translate_scale`,
  `"transform"` arm.
- New pixel test + pytest tests + example.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
