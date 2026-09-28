# Build Tracker — `tre` 0.4.0

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

**Artifact:** generate with `python3 tools/generate_tracker_artifact.py --project "tre 0.4.0" --out tools/build-tracker-0.4.0.generated.html` (after committing the tracker, so the stamp names the commit) and publish to [the 0.4.0 tracker page](https://claude.ai/artifact/PTt1sBpABbM2XxTrL7xdFb). The [0.3 line's page](https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty) stays the record of `main`'s tracker and its `0.3.5.x` fixes.

**This tracker is the `0.4.0` line** -- the `vello_hybrid` fork for real GPU-level partial redraw ([issue #4](https://github.com/mindderivative/tre/issues/4)) -- and its milestones restart at 1. Earlier history is archived: `v0.1.0`–`v0.3.5.1` (M1–M104) in [`BUILD_TRACKER_ARCHIVE_0.3.md`](BUILD_TRACKER_ARCHIVE_0.3.md), and M1–M49 of that line in [`BUILD_TRACKER_ARCHIVE_M1-M50.md`](BUILD_TRACKER_ARCHIVE_M1-M50.md). Fixes to the released `0.3.5` stay on the 0.3.x line as `0.3.5.x` patch releases, tracked on `main`'s `BUILD_TRACKER.md` (continuing its numbering after M104), as `0.3.5.1` was.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — Scope and Upstream Pin | `███████░░░` 67% | 🚧 In progress — decisions made: build on upstream `vello_gpu`, which already has issue #4's patch, at a pinned Git commit |
| M2 — Migrate to `vello_gpu` | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M3 — A Persistent Offscreen Target and Blit | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M4 — Dirty-Region Tracking | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M5 — Partial Redraw End to End, Measured | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M6 — Release `0.4.0` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** M1 Phase 1, the decisions (2026-09-28). Checking upstream first showed `linebender/vello` had renamed `vello_hybrid` to `vello_gpu` and added issue #4's patch itself -- a render that keeps the target's contents and clears only listed rectangles -- so `0.4.0` builds on a pinned upstream commit instead of a fork, and M2 becomes a migration rather than a patch.

**Up next:** M1 Phase 2 -- pin an upstream `vello_gpu` commit and size M2's migration from its dependency set. The user's decisions (2026-09-28): the pinned upstream commit as a Git dependency, a small set of dirty rects with a full-redraw fallback, and partial redraw on by default with a switch to turn it off.

**Known gaps:**
- Every frame repaints the whole window: `vello_hybrid` 0.2.0's public `Renderer::render` always clears the target and takes no scissor, and `tre` renders straight into the swapchain image, which keeps no previous frame. The idle loop sleeps when nothing changes (0.3.x, M29), so the cost is paid only while something animates -- but then it's the full window, however small the change. This line exists to close it.

---

## Milestone 1 — Scope and Upstream Pin

**Status: 🚧 In progress.** Issue #4 called for a scope-confirmation pause before building; this milestone is that pause, then the pinned dependency. Checking upstream first changed the plan: `linebender/vello` renamed `vello_hybrid` to `vello_gpu` ([PR #1883](https://github.com/linebender/vello/pull/1883), 2026-09-08) and then added issue #4's patch itself ([PR #1869](https://github.com/linebender/vello/pull/1869), 2026-09-09) -- `render()` takes a `TargetInit`: `SrcOver` draws over the kept contents, and `ClearSettings::Rects` clears only listed rectangles. Neither is published: `vello_hybrid` 0.2.0 is still the latest release, and `vello_gpu` on crates.io is a 0.1.0 name reservation.

### Phase 1 — Decisions ✅
- Step 1: issue #4's patch points re-verified against upstream — ✅ (0.2.0 is still the latest release, checked 2026-09-27; upstream `main` renamed the crate to `vello_gpu` and already has the no-clear and rect-clear render, so no patch is needed; its `render()` also gained `resources`, `depth_view`, and `texture_bindings` parameters; `tre` calls `vello_hybrid` 33 times in 29 files, most in tests)
- Step 2: the user's decisions — ✅ (user, 2026-09-28: "D1 - pinned upstream commit, D2 - pinned Git dependency, D3 - small set with a full-redraw fallback, D4 - as recommended" -- build on upstream `vello_gpu` at a pinned commit rather than forking 0.2.0; depend on it as a Git dependency pinned to that commit, a `mindderivative/vello` fork only if a change upstream lacks is ever needed; track a small set of dirty rects, merged when they overlap, with a full redraw past a size limit; partial redraw on by default with a switch to turn it off, made safe by an M5 test requiring identical pixels from partial and full redraw)

### Phase 2 — Upstream Pin ⬜
- Step 1: choose the upstream commit and check its dependency set against `tre`'s -- `wgpu`, `peniko`, `kurbo`, `parley`, `glifo`, and `vello_common` versions, and the MSRV -- to size M2's migration — ⬜

---

## Milestone 2 — Migrate to `vello_gpu`

**Status: ⬜ Proposed.** Replaces the planned `vello_hybrid` patch: upstream already has it (M1), so this milestone moves `tre` from `vello_hybrid` 0.2.0 to `vello_gpu` at M1's pinned commit, with no behavior change.

### Phase 1 — Migration ⬜
- Step 1: `vello_hybrid` → `vello_gpu` as a pinned Git dependency, with the other Linebender crates moved in step; `render()` called with `TargetInit::Clear(ClearSettings::Viewport)`, today's behavior — ⬜
- Step 2: every test, pixel test, example, and `frame_budget.rs` passing unchanged; the wheel builds with the Git dependency in CI on all three platforms — ⬜

---

## Milestone 3 — A Persistent Offscreen Target and Blit

**Status: ⬜ Proposed.** Issue #4's second piece, in `engine-render`/`engine-py`: a swapchain image keeps no previous frame, so partial redraw needs a texture that does.

### Phase 1 — Offscreen Target ⬜
- Step 1: render into a persistent per-window `wgpu::Texture`, recreated on resize and scale-factor change, and blit it to the acquired swapchain image every frame — ⬜
- Step 2: pixel tests that the blitted output matches the direct render exactly, and no regression in `frame_budget.rs` — ⬜

---

## Milestone 4 — Dirty-Region Tracking

**Status: ⬜ Proposed.** Issue #4's third piece, in `engine-core`: today's `Tree.dirty` is a whole-frame yes/no; partial redraw needs where.

### Phase 1 — Dirty Rects ⬜
- Step 1: accumulate the painted bounds, before and after, of whatever changed each frame -- a ticking animation, a property set, a layout change, a structural change, a canvas redraw, a layer shown or hidden -- including transforms, shadows, and clipping — ⬜
- Step 2: tests that every kind of change reports a rect covering it, and that a change spanning most of the window falls back to a full redraw — ⬜

---

## Milestone 5 — Partial Redraw End to End, Measured

**Status: ⬜ Proposed.**

### Phase 1 — Integration ⬜
- Step 1: the frame loop renders only the dirty region through the patched renderer into the offscreen target, falling back to a full redraw on resize, scale change, or a large region — ⬜
- Step 2: correctness -- pixel tests comparing partial against full redraw across every example's animations — ⬜
- Step 3: measurement against `v0.3.5.1`, the same way M101 of the 0.3 line measured: frame time for a small animation in a large window, and no regression for a full-window change — ⬜

---

## Milestone 6 — Release `0.4.0`

**Status: ⬜ Proposed.**

### Phase 1 — Release ⬜
- Step 1: PR to `main`, CI green on all three platforms, merge, re-verify, release note, annotated tag, as with `v0.3.5` — ⬜
- Step 2: Tesserae moves to `0.4.0` — ⬜

---

## Branch: `0.4.0` — Scaffold

**Status: ✅ Scaffolded (2026-09-27).**

- Branch `0.4.0` created off `main` at `a67376d` — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.0` (from `0.3.5` and `0.3.5.1`); `Cargo.lock` updated via `cargo check` — ✅
- The 0.3.x tracker moved to `BUILD_TRACKER_ARCHIVE_0.3.md`; this tracker started at M1 — ✅
- GitHub: a `0.4.0` milestone holding issue #4, and a `0.3.5.x` milestone for fixes to the released `0.3.5` — ✅
