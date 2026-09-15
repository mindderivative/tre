# Plan: M3 Phase 5, Step 8 — MD3 Shadow Spike (§14 step 8)

Corresponds to `BUILD_TRACKER.md` M3 Phase 5, step 8 of 4 (steps 8-11) -- opens Phase 5.

## Goal

Per §14 step 8: "Spike: MD3 shadow via `fill_blurred_rounded_rect`,
standalone, against the exact pinned Vello version (§7.2 risk)."

§7.2 and §15's Risk Register both flag this by name: `fill_blurred_
rounded_rect`/`DropShadowOnly` are early-stage per Vello's own release
notes, with no API stability guarantee and uneven parity across the
`vello`/`vello_cpu`/`vello_hybrid` variants. Mitigation named in the
Risk Register: "Standalone spike (§14 step 8) before MD3 components
depend on it."

## Scope

In scope:
- Verify `vello_hybrid` 0.2.0's real current `fill_blurred_rounded_rect`
  signature directly in its vendored source before writing any code
  (matching this project's established discipline for every young
  Linebender-family dependency).
- `engine-render::build_shadow_scene`: one standalone blurred rounded
  rect, no `Tree`, no layout, no MD3 elevation-level tokens -- those are
  deliberately later steps (9 ripple/state-layer, 11 dynamic color) that
  would otherwise sit on an unverified foundation.
- `crates/engine-render/tests/shadow_spike.rs`: the actual proof --
  headless render-to-texture-then-readback, sampling four points along
  one line straight out from a non-corner edge, asserting a real
  Gaussian falloff (opaque deep inside, ~half-coverage at the raw edge,
  partial-but-decreasing just outside, transparent once past the kernel
  spread) rather than "it compiled and didn't panic."

Out of scope: `engine-md3` itself. Re-reading §4's crate-boundary rule
and the Risk Register's own mitigation text ("confine all Vello calls to
`engine-render`") corrects an earlier assumption (recorded in memory
after step 7) that this step would be "the first step to touch
`engine-md3` for real" -- `engine-md3` depends only on `engine-core`
(§1 Locked Decisions), never on `vello_hybrid`/`wgpu`, so a spike of a
`vello_hybrid` API cannot live there. `engine-md3` starts accumulating
real content once a later step needs to expose MD3-named presets (e.g.
elevation-level -> (radius, std_dev, opacity) tables) on top of this
now-verified primitive -- not this step.

## Verification

`cargo test -p engine-render --test shadow_spike` passes with a real
measured Gaussian falloff, not an approximation asserted without
checking. `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` all clean.
