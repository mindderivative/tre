# Build Tracker — `tre` 0.4.0

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

**This tracker is the `0.4.0` line** -- the `vello_hybrid` fork for real GPU-level partial redraw ([issue #4](https://github.com/mindderivative/tre/issues/4)) -- and its milestones restart at 1. Earlier history is archived: `v0.1.0`–`v0.3.5.1` (M1–M104) in [`BUILD_TRACKER_ARCHIVE_0.3.md`](BUILD_TRACKER_ARCHIVE_0.3.md), and M1–M49 of that line in [`BUILD_TRACKER_ARCHIVE_M1-M50.md`](BUILD_TRACKER_ARCHIVE_M1-M50.md). Fixes to the released `0.3.5` stay on the 0.3.x line as `0.3.5.x` patch releases, tracked on `main`'s `BUILD_TRACKER.md` (continuing its numbering after M104), as `0.3.5.1` was.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — Scope and Fork Setup | `░░░░░░░░░░` 0% | ⬜ Proposed — the decision gate issue #4 calls for |
| M2 — The `vello_hybrid` Patch: No-Clear and Scissored Render | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M3 — A Persistent Offscreen Target and Blit | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M4 — Dirty-Region Tracking | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M5 — Partial Redraw End to End, Measured | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M6 — Release `0.4.0` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** the `0.4.0` line scaffolded (2026-09-27) -- branch `0.4.0` off `main` at `a67376d` (post-`v0.3.5.1`), the version bumped to `0.4.0` in `Cargo.toml` and `pyproject.toml`, this tracker started with its milestones restarting at 1, and the 0.3.x tracker archived. User: "Scaffold 0.4.0 and start a new build tracker. Annotate as 0.4.0 and restart the Milestones at 1. 0.4.0 will cover vello_hybrid fix. Any current issues for 0.3.5 will stay in as 0.3.5.x."

**Up next:** M1, the scope decisions -- nothing is built before the user confirms them. The milestones below are the proposed shape, from issue #4's own three-part sizing (the patch, the offscreen target, dirty-region tracking), plus measurement and the release.

**Known gaps:**
- Every frame repaints the whole window: `vello_hybrid` 0.2.0's public `Renderer::render` always clears the target and takes no scissor, and `tre` renders straight into the swapchain image, which keeps no previous frame. The idle loop sleeps when nothing changes (0.3.x, M29), so the cost is paid only while something animates -- but then it's the full window, however small the change. This line exists to close it.

---

## Milestone 1 — Scope and Fork Setup

**Status: ⬜ Proposed.** Issue #4 records the plan and its sizing -- three separate pieces of work plus ongoing fork maintenance against an upstream whose "scene scheduling and rendering architecture has been rewritten" since 0.1.0 -- and says a scope-confirmation pause is warranted before building. This milestone is that pause, and then the fork itself.

### Phase 1 — Decisions ⬜
- Step 1: re-verify issue #4's patch points against the current `vello_hybrid` source -- 0.2.0 is still the latest published release (checked 2026-09-27), but upstream `main` may have moved; decide whether to fork the 0.2.0 tag or upstream `main` — ⬜
- Step 2: the user's decisions -- fork mechanics (a `mindderivative/vello` fork consumed through `[patch.crates-io]` at a pinned revision, or a vendored copy in this repo); dirty-region granularity (one bounding rect per frame, or a small set); and whether partial redraw is on by default or opt-in for `0.4.0` — ⬜

### Phase 2 — Fork Setup ⬜
- Step 1: the fork created and pinned, building unchanged -- every existing test and pixel test passing against it before any patch — ⬜

---

## Milestone 2 — The `vello_hybrid` Patch: No-Clear and Scissored Render

**Status: ⬜ Proposed.** Issue #4's first piece, in the fork.

### Phase 1 — Patch ⬜
- Step 1: a public render entry point that can skip the full-target clear -- `render_scene` already takes `clear: bool`, which `render()` hardcodes to `true` — ⬜
- Step 2: a scissored clear of a caller-supplied rect, using the `set_scissor_rect` technique `clear_atlas_region` already uses internally — ⬜
- Step 3: fork-level pixel tests -- untouched pixels outside the rect survive a render; the rect is cleared and redrawn — ⬜

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
