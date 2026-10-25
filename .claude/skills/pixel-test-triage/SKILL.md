---
name: pixel-test-triage
description: Use when a headless GPU pixel test in tre fails or changes result, above all after a bump of the pinned renderer (vello_gpu, vello_common, glifo, wgpu, naga, skrifa), or when a test passes on one GPU and fails on another. Sorts the failure into a real regression, a renderer change, or an adapter difference before anything is changed.
---

# Pixel-test triage

tre's pixel tests (`crates/engine-render/tests/*.rs`, sharing `tests/support/mod.rs`) render a small scene offscreen, read the pixels back and assert on them in the test itself. There are no golden images. A failure therefore says "this pixel was not the colour the test expects", and the cause may be the engine, the renderer underneath it, or the GPU. Settle which before touching the code or the assertion.

Pair this with `systematic-debugging`: no fix, and no loosened assertion, before the cause is known.

## 1. Record what failed, exactly

- The test binary and test name, and the assertion's own message (coordinates, expected, actual).
- The adapter. The harness prints `GPU adapter: <name> | backend | type | driver` once per test process to stderr; run with `-- --nocapture` to see it. A result with no adapter named cannot be compared with another.
- Whether the tree changed since the last green run: `git log` on the test, the code it covers, and `Cargo.toml` / `Cargo.lock`.

## 2. Reproduce on both adapters

```bash
cargo test -p engine-render --release --test <binary> -- --nocapture
# the software adapter CI uses (lavapipe):
TRE_TEST_FORCE_FALLBACK=1 cargo test -p engine-render --release --test <binary> -- --nocapture
# or choose the ICD explicitly:
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json cargo test ...
```

Also run it once with `TRE_DAMAGE_VERIFY=1`, which makes the damage tracker check itself against a full walk and panic on a disagreement. A damage bug shows up as a stale or missing region, not a wrong colour.

| Real GPU | Lavapipe | Reading |
|---|---|---|
| fails | fails | Engine or renderer change. Go to step 3. |
| passes | fails | CI will fail. Software rasteriser difference or a real bug the GPU hides. |
| fails | passes | A driver or hardware difference. Say which GPU, and do not "fix" it by loosening the test. |
| flaky | | Suspect timing: a missing wait for the GPU, a reused target, or a thread race. Re-run 10 times before drawing conclusions. |

## 3. Classify the change

- **Off by a little along an edge** (anti-aliasing, a rounded corner, a gradient step): suspect the renderer. Read the failing assertion to see whether it samples an edge or the middle of a shape. `analytic_fill_quality.rs` exists because fill quality has changed before.
- **Wrong colour or alpha in the middle of a shape:** blend, opacity or colour-space handling. Check `peniko` and `vello_common` for a changed default.
- **Text or emoji differs:** `glifo` and `skrifa` changes, and the font in use. Bitmap glyphs only draw through glifo's atlas (see the `glifo` comment in the root `Cargo.toml`).
- **A shader test fails to compile:** `naga` / `wgpu` version drift. `tools/check_shaders_live.py` and `docs/design/wgsl.md` say how shaders are validated.
- **Whole frame blank or panics:** device creation, limits or format support, not paint code.

## 4. After a renderer bump

The root `Cargo.toml` pins `vello_gpu`, `vello_common` and `glifo` to one upstream commit and requires `wgpu` at the major that commit uses. They move together. To find the culprit:

1. Run the failing test at the old and the new pin (`git stash` or a worktree), on the same adapter.
2. If it differs, read the upstream commit range between the two revisions for the area that failed.
3. Decide with the user whether to accept the new output (update the expected value in the test, with a comment saying which revision changed it and why), report it upstream, or hold the bump.

Re-run the full pixel suite after any bump, not just the test that failed.

## 5. Report

State the adapter(s) tested, the verdict (regression, renderer change, adapter difference, flaky), the evidence, and what was and was not run. Say plainly if only software rendering was available. Hardware that cannot be reached from here (Mac, Windows, other GPU vendors) is an open check: add it to the relevant held issue (#91, #118) instead of calling it verified.

## Do not

- Do not loosen a tolerance or change an expected pixel to turn a test green without knowing why it moved.
- Do not skip or `#[ignore]` a failing test to unblock a bump.
- Do not treat a pass on lavapipe as proof for real GPUs, or the reverse.
