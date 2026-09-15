# Log: Phase 5, Step 5.1.1 -- Canvas Drawing-Context State Stack

## Real bug found and fixed during implementation

**`draw_rounded_rect`'s first `scale_alpha` draft only scaled the vertex
color's alpha byte, not its RGB channels -- which rendered every
`Canvas::set_alpha()` call invisible, not just imprecise.** `UiVertex::
color` must be *premultiplied* alpha before it reaches
`sdf_rounded_rect.frag`: that shader's own output is `vec4(frag_color.rgb
* coverage, frag_color.a * coverage)` -- `frag_color.rgb` is multiplied
only by the SDF's own anti-aliasing coverage term, never by
`frag_color.a`. Scaling only the alpha byte therefore produced a
genuinely over-bright premultiplied color (`rgb / a > 1.0` in straight-alpha
terms) that the GPU's blend hardware silently clamped back to fully
opaque -- confirmed directly by running `canvas_state_stack_demo`, whose
first draft reported Rect B's 50%-alpha square as pixel-identical to a
fully opaque one:

```
thread 'main' panicked at .../canvas_state_stack_demo.rs:151:5:
assertion `left == right` failed: Rect A's raw local position must be untouched background, got [255, 255, 255, 255]
```

(That specific panic was actually a second, independent test-design bug
-- see below -- but investigating it is what surfaced this real one: Rect
B's blended square at [192, 192, 194, 255] would have been indistinguishable
from `[255, 255, 255, 255]` had the alpha-only scaling bug not already
been fixed first.)

**Fix:** `scale_alpha` (renamed `premultiply_alpha` to match what it now
actually does) scales all four channels -- R, G, B, and A -- by the
effective alpha factor, not just A. Documented directly on
`UiVertex`'s own canonical definition in ARCHITECTURE.md Section 3.1,
since this is a real, previously-undocumented convention any future
primitive writing `UiVertex::color` (`draw_text`, `draw_path`,
`draw_svg` -- Step 5.1.2 onward) needs to follow, not something specific
to `draw_rounded_rect` alone.

## A second, independent bug -- in the demo's own test design, not the Canvas

`canvas_state_stack_demo`'s first draft checked point `(25, 25)` to prove
Rect A's transform had genuinely moved it away from its raw local
position. `(25, 25)` turned out to fall inside Rect B's own footprint
`(10,10)-(60,60)` -- a real, separate rect drawn elsewhere in the same
scene -- so the check was tautologically doomed to see *something*
there regardless of whether Rect A's transform worked correctly. Fixed
by moving the check point to `(5, 5)`, confirmed clear of every other
rect in the scene.

## What else was verified, not just unit-tested in isolation

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo test --workspace`: all clean, zero warnings,
  zero failures across every crate. `tre-engine` now has 20 unit tests
  (up from 12), covering `save`/`restore`/`transform`/`set_alpha`
  composition and `push_clip`/`pop_clip` intersection, all passing on
  the first run -- only the real GPU render caught the premultiplied-
  alpha gap, since the unit tests check IR data (vertex colors, clip
  bounds), not rasterized pixels.
- `sdf_rounded_rect_demo` (the pre-existing, only other real caller of
  `draw_rounded_rect`, which never touches `save`/`transform`/
  `push_clip`) re-run manually: unchanged output, confirming the
  rewired vertex/alpha/clip logic is a genuine no-op at the default
  identity/full-alpha/no-clip state.
- New capstone example `canvas_state_stack_demo` (once both bugs above
  were fixed) passed all its own assertions: a real transformed
  world-space placement, a real visible partial-alpha blend, and an
  IR-level clip-bounds check.
- **All 17 examples** (the 16 pre-existing plus the new
  `canvas_state_stack_demo`) re-run manually end to end against real
  Vulkan hardware: zero validation errors, zero regressions.
- `tre-rhi-vulkan` gained a new dev-dependency on `tre-math` (needed for
  `Affine2` in the new demo) -- confirmed this doesn't create a cycle or
  otherwise disturb the crate's existing dependency graph.
