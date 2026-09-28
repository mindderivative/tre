# Build Tracker — `tre` 0.4.0

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

**Artifact:** generate with `python3 tools/generate_tracker_artifact.py --project "tre 0.4.0" --out tools/build-tracker-0.4.0.generated.html` (after committing the tracker, so the stamp names the commit) and publish to [the 0.4.0 tracker page](https://claude.ai/artifact/PTt1sBpABbM2XxTrL7xdFb). The [0.3 line's page](https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty) stays the record of `main`'s tracker and its `0.3.5.x` fixes.

**This tracker is the `0.4.0` line** -- the `vello_hybrid` fork for real GPU-level partial redraw ([issue #4](https://github.com/mindderivative/tre/issues/4)) -- and its milestones restart at 1. Earlier history is archived: `v0.1.0`–`v0.3.5.1` (M1–M104) in [`BUILD_TRACKER_ARCHIVE_0.3.md`](BUILD_TRACKER_ARCHIVE_0.3.md), and M1–M49 of that line in [`BUILD_TRACKER_ARCHIVE_M1-M50.md`](BUILD_TRACKER_ARCHIVE_M1-M50.md). Fixes to the released `0.3.5` stay on the 0.3.x line as `0.3.5.x` patch releases, tracked on `main`'s `BUILD_TRACKER.md` (continuing its numbering after M104), as `0.3.5.1` was.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — Scope and Upstream Pin | `██████████` 100% | ✅ Complete (2026-09-28) — build on upstream `vello_gpu` at `b408cd00`, which already has issue #4's patch; M2's main cost is `wgpu` 29 → 30 |
| M2 — Migrate to `vello_gpu` | `██████████` 100% | ✅ Complete (2026-09-28) — `tre` on upstream `vello_gpu` and `wgpu` 30, every test unchanged, CI green on Linux, macOS, and Windows |
| M3 — A Persistent Offscreen Target and Blit | `██████████` 100% | ✅ Complete (2026-09-28) — each window renders into a texture that keeps its frame, copied to the screen byte-exact |
| M4 — Dirty-Region Tracking | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M5 — Partial Redraw End to End, Measured | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M6 — Release `0.4.0` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** M3 (2026-09-28) -- each window now renders into a texture that keeps its frame, copied to the swapchain image every frame, byte-exact against a direct render in a new pixel test. Surfaces that can't be copied into keep rendering directly. No visible change; it's what partial redraw draws over.

**Up next:** M4, dirty-region tracking -- which parts of the window changed each frame, as a small set of rects with a full-redraw fallback (the user's D3).

**Known gaps:**
- Every frame repaints the whole window: `vello_hybrid` 0.2.0's public `Renderer::render` always clears the target and takes no scissor, and `tre` renders straight into the swapchain image, which keeps no previous frame. The idle loop sleeps when nothing changes (0.3.x, M29), so the cost is paid only while something animates -- but then it's the full window, however small the change. This line exists to close it.

---

## Milestone 1 — Scope and Upstream Pin

**Status: ✅ Complete (2026-09-28).** Issue #4 called for a scope-confirmation pause before building; this milestone is that pause, then the pinned dependency. Checking upstream first changed the plan: `linebender/vello` renamed `vello_hybrid` to `vello_gpu` ([PR #1883](https://github.com/linebender/vello/pull/1883), 2026-09-08) and then added issue #4's patch itself ([PR #1869](https://github.com/linebender/vello/pull/1869), 2026-09-09) -- `render()` takes a `TargetInit`: `SrcOver` draws over the kept contents, and `ClearSettings::Rects` clears only listed rectangles. Neither is published: `vello_hybrid` 0.2.0 is still the latest release, and `vello_gpu` on crates.io is a 0.1.0 name reservation.

### Phase 1 — Decisions ✅
- Step 1: issue #4's patch points re-verified against upstream — ✅ (0.2.0 is still the latest release, checked 2026-09-27; upstream `main` renamed the crate to `vello_gpu` and already has the no-clear and rect-clear render, so no patch is needed; its `render()` also gained `resources`, `depth_view`, and `texture_bindings` parameters; `tre` calls `vello_hybrid` 33 times in 29 files, most in tests)
- Step 2: the user's decisions — ✅ (user, 2026-09-28: "D1 - pinned upstream commit, D2 - pinned Git dependency, D3 - small set with a full-redraw fallback, D4 - as recommended" -- build on upstream `vello_gpu` at a pinned commit rather than forking 0.2.0; depend on it as a Git dependency pinned to that commit, a `mindderivative/vello` fork only if a change upstream lacks is ever needed; track a small set of dirty rects, merged when they overlap, with a full redraw past a size limit; partial redraw on by default with a switch to turn it off, made safe by an M5 test requiring identical pixels from partial and full redraw)

### Phase 2 — Upstream Pin ✅
- Step 1: choose the upstream commit and check its dependency set against `tre`'s -- `wgpu`, `peniko`, `kurbo`, `parley`, `glifo`, and `vello_common` versions, and the MSRV -- to size M2's migration — ✅ (pinned `linebender/vello` `b408cd003d5e936c9cc456d18d41467bb6d0e12f`, upstream `main` on 2026-09-28 with green CI, 24 checks passed; `peniko` 0.6.1, `kurbo` 0.13.1, `skrifa` 0.44.0 match `tre`'s, and `parley` 0.11.1 builds on the same two; `vello_common` 0.2.0 and `glifo` 0.3.0 keep their version numbers but carry unreleased code, so both come from the same Git commit to keep types identical; `wgpu` goes 29.0.4 → 30.0.x, the one major bump, touching 68 direct uses in `tre`'s source (`engine-py` `app.rs`, `engine-render` `lib.rs`, `image_cache.rs`, `text.rs`) and 487 in pixel tests, most through shared setup; MSRV 1.89, this machine 1.98.1; a throwaway crate on that commit with `wgpu` 30 built in 36 s and printed both `TargetInit::SrcOver` and `TargetInit::Clear(ClearSettings::Rects { .. })`, confirming the no-clear and rect-clear render is public)

---

## Milestone 2 — Migrate to `vello_gpu`

**Status: ✅ Complete (2026-09-28).** User: "push and lets get started on M2". Replaces the planned `vello_hybrid` patch: upstream already has it (M1), so this milestone moves `tre` from `vello_hybrid` 0.2.0 to `vello_gpu` at M1's pinned commit, with no behavior change.

### Phase 1 — Migration ✅
- Step 1: `vello_hybrid` → `vello_gpu`, with `vello_common` and `glifo`, as Git dependencies pinned to `b408cd00`, and `wgpu` 29 → 30 in `tre`'s own GPU code; `render()` called with `TargetInit::Clear(ClearSettings::Viewport)`, today's behavior — ✅ (one `[workspace.dependencies]` table in the root `Cargo.toml` pins `vello_gpu`, `vello_common`, and `glifo` to the commit and `wgpu` to 30.0.1, and every crate refers to it -- the lock resolves one `wgpu`; API changes: `glyph_run` now needs `vello_gpu`'s `text` feature; `Scene::draw_texture_rects` and `SampleRect` are gone, so an image is an `ImageSource::ExternalTexture` paint over a filled rect, its sample transform as the paint transform; `render()` takes a `depth_view` (`None`) and a `TargetInit` (a full transparent clear, as 0.2.0 always did); `fill_glyphs` returns a `Result` -- the rest of a run still draws -- now warned once per process through `tracing`; `wgpu` 30 moved `present()` to the queue, made `get_mapped_range()` return a `Result`, added `RequestAdapterOptions::apply_limit_buckets` (set `false`, the adapter's real limits, as before), and `RenderSize`/`RenderTargetConfig` sizes are `u16`, clamped by a new `render_extent`; the pixel tests' readback and setup code updated the same way; `vello_gpu` still updates its size-dependent state from each `render()`'s size, checked in its source, so resizing needs no rebuild; comments describing the old image path and resize reasoning rewritten, and `ARCHITECTURE.md`, the README, and the docs name `vello_gpu`; cargo 327 passed, every pixel test included -- the same as on 0.2.0 -- `frame_budget.rs` median 0.628 ms, pytest 430 passed, 1 skipped, 21 examples, clippy, fmt, mypy, docs strict, API audit 43 names; a release wheel and an sdist build with the Git dependency, the wheel 10.43 MB against 10.32 MB, `glifo` now compiled in)
- Step 2: every test, pixel test, example, and `frame_budget.rs` passing unchanged; the wheel builds with the Git dependency in CI on all three platforms — ✅ (user: "yes" to pushing and a draft PR, since `ci.yml` runs only on pull requests to `main`; [draft PR #17](https://github.com/mindderivative/tre/pull/17), which stays a draft until M6, ran CI on `1cbda98`, green on all three jobs -- Linux: cargo 328 passed, the frame budget included, pytest 417 passed, 14 skipped with no display, every example; macOS and Windows: cargo 327 passed, 0 failed, so the pixel tests pass on each platform's GPU backend, not only Vulkan -- all fetching `vello_gpu` from the Git commit)

---

## Milestone 3 — A Persistent Offscreen Target and Blit

**Status: ✅ Complete (2026-09-28).** User: "push and start M3". Issue #4's second piece, in `engine-render`/`engine-py`: a swapchain image keeps no previous frame, so partial redraw needs a texture that does.

### Phase 1 — Offscreen Target ✅
- Step 1: render into a persistent per-window `wgpu::Texture`, recreated on resize and scale-factor change, and blit it to the acquired swapchain image every frame — ✅ (new `engine_render::PersistentTarget`: a texture in the surface format, `RENDER_ATTACHMENT | COPY_SRC`, `ensure_size` recreating it only when the size changes and reporting that its contents are then undefined, and `copy_to` a plain texture-to-texture copy -- byte-exact, no shader -- limited to the area both textures cover, so a resize race can't panic; `engine-py`'s `GpuState` asks for `COPY_DST` on the surface where `get_capabilities` offers it, and keeps a target per window, resized with the surface, which a scale-factor change reaches as a size change in physical pixels; the frame renders into the target and copies it into the acquired image; a surface without `COPY_DST` renders straight into the swapchain image as before, with an info log; a debug log names the route, and on this machine, AMD with Vulkan, both of `two_windows.py`'s windows take the persistent target; `ARCHITECTURE.md` §6 updated)
- Step 2: pixel tests that the blitted output matches the direct render exactly, and no regression in `frame_budget.rs` — ✅ (new `tests/persistent_target.rs`, 3 tests: a shadowed rounded card under a translucent box, rendered into a target and copied into a stand-in swapchain image, matches a direct render with 0 differing bytes; a second copy with no render in between gives the identical frame, the target keeping its contents; `ensure_size` recreates only on a size change, and raises 0 to 1; `frame_budget.rs` now renders into a `PersistentTarget` and copies it, the live frame: medians 0.73, 0.73, and 1.05 ms over three runs against 0.63 ms rendering direct -- within the 0.31–1.10 ms noise band M101 measured on this machine, the copy being GPU-side work the CPU timer barely sees; cargo 330 passed, pytest 430 passed, 1 skipped, 21 examples, clippy, fmt, mypy, docs strict)

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
