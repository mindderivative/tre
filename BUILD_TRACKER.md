# Build Tracker — `tre` 0.4

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

**Artifact:** generate with `python3 tools/generate_tracker_artifact.py --project "tre 0.4" --out tools/build-tracker-0.4.0.generated.html` (after committing the tracker, so the stamp names the commit) and publish to [the 0.4.0 tracker page](https://claude.ai/artifact/PTt1sBpABbM2XxTrL7xdFb). The [0.3 line's page](https://claude.ai/artifact/CaPkWjpd91oR7YFbcqC9ty) stays the record of `main`'s tracker and its `0.3.5.x` fixes.

**This tracker is the `0.4.0` line** -- the `vello_hybrid` fork for real GPU-level partial redraw ([issue #4](https://github.com/mindderivative/tre/issues/4)) -- and its milestones restart at 1. Earlier history is archived: `v0.1.0`–`v0.3.5.1` (M1–M104) in [`BUILD_TRACKER_ARCHIVE_0.3.md`](BUILD_TRACKER_ARCHIVE_0.3.md), and M1–M49 of that line in [`BUILD_TRACKER_ARCHIVE_M1-M50.md`](BUILD_TRACKER_ARCHIVE_M1-M50.md). Fixes to the released `0.3.5` stay on the 0.3.x line as `0.3.5.x` patch releases, tracked on `main`'s `BUILD_TRACKER.md` (continuing its numbering after M104), as `0.3.5.1` was.

**0.4.x patches** (0.4.1 onward) continue here, their milestones numbered on from M6; `v0.4.0` itself is M1–M6.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — Scope and Upstream Pin | `██████████` 100% | ✅ Complete (2026-09-28) — build on upstream `vello_gpu` at `b408cd00`, which already has issue #4's patch; M2's main cost is `wgpu` 29 → 30 |
| M2 — Migrate to `vello_gpu` | `██████████` 100% | ✅ Complete (2026-09-28) — `tre` on upstream `vello_gpu` and `wgpu` 30, every test unchanged, CI green on Linux, macOS, and Windows |
| M3 — A Persistent Offscreen Target and Blit | `██████████` 100% | ✅ Complete (2026-09-28) — each window renders into a texture that keeps its frame, copied to the screen byte-exact |
| M4 — Dirty-Region Tracking | `██████████` 100% | ✅ Complete (2026-09-28) — `DamageTracker` reports what changed each frame as at most 4 rects, or a full redraw |
| M5 — Partial Redraw End to End, Measured | `██████████` 100% | ✅ Complete (2026-09-28) — every window redraws only what changed, byte-identical to a full redraw; a small animation in a 1920x1080 window costs 3.7–4.9x less GPU+CPU time a frame than `v0.3.5.1`, a whole-window change the same |
| M6 — Release `0.4.0` | `██████████` 100% | ✅ Complete (2026-09-28) — `v0.4.0` released on GitHub and PyPI as `tesserae-engine` 0.4.0, closing issue #4; Tesserae moved onto it with nothing broken |
| M7 — Partial Redraw's Fixed Costs | `██████████` 100% | ✅ Complete (2026-09-28) — measured, not cut: the whole-window overhead is the 0.1 ms damage walk (the rest of `v0.4.0`'s reported gap was noise), and the 0.25 ms copy can't be narrowed safely without swapchain buffer age |
| M8 — Show What's Redrawn | `██████████` 100% | ✅ Complete (2026-09-28) — `window.set(show_damage=True)` tints each presented frame's redrawn areas, never the kept frame |
| M9 — Housekeeping and Release `0.4.1` | `██████████` 100% | ✅ Complete (2026-09-29) — `v0.4.1` released on GitHub and PyPI, closing issue #21; Tesserae moved onto it, its side buttons driving back/forward |
| M10 — The Mouse's Back and Forward Buttons ([issue #21](https://github.com/mindderivative/tre/issues/21)) | `██████████` 100% | ✅ Complete (2026-09-29) — `Event.button` reports `"back"` and `"forward"`, heard on the root, with no `click`; ships in `0.4.1` |
| M11 — CSS Grid Layout ([issue #23](https://github.com/mindderivative/tre/issues/23)) | `██████████` 100% | ✅ Complete (2026-09-29) — `display="grid"`, track lists, placements, auto tracks and flow, row/column gaps, and item alignment, read back as set |
| M12 — Scroll Views: Keys, `scroll_into_view`, Focus, and a `scroll` Event ([issue #24](https://github.com/mindderivative/tre/issues/24)) | `██████████` 100% | ✅ Complete (2026-09-29) — Page Up/Down keys, keyboard scrolling, `scroll_into_view` with focus reveal, and the `scroll` event |
| M13 — Release `0.4.2` | `██████████` 100% | ✅ Complete (2026-09-30) — `v0.4.2` released on GitHub and PyPI, closing issues #23 and #24; Tesserae moved onto it, its scroll view now following tre's keys, reveal, and `scroll` event |
| M14 — Keyboard Scrolling Leaves Shortcuts Alone | `██████████` 100% | ✅ Complete (2026-09-30) — keys with Ctrl, Alt, or Meta held are shortcuts and no longer scroll; Shift still does |
| M15 — `scroll_offset` Clamped When Set | `██████████` 100% | ✅ Complete (2026-09-30) — `set` and `animate` clamp `scroll_offset` to the view's range at once, with one `scroll` event; a created offset is the event's baseline |
| M16 — Grid: A Bare Number as a One-Track List ([issue #27](https://github.com/mindderivative/tre/issues/27)) | `██████████` 100% | ✅ Complete (2026-09-30) — `grid_auto_rows=96` means `[96]`, reading back as `"96"` |
| M17 — Shift+Wheel Scrolls Horizontal Views | `██████████` 100% | ✅ Complete (2026-09-30) — Shift+wheel scrolls horizontal views, and a wheel passes views it can't move to the next one out |
| M18 — Unit Tests for the Scroll Core | `██████████` 100% | ✅ Complete (2026-09-30) — 9 `engine-core` tests cover the scroll API and M17's wheel rule without Python |
| M19 — Stub Drift Checked in CI | `██████████` 100% | ✅ Complete (2026-09-30) — the stub matches what PyO3 builds, and CI runs `mypy --strict` and `stubtest` |
| M20 — Release `0.4.3` | `██████████` 100% | ✅ Complete (2026-09-30) — `v0.4.3` released on GitHub and PyPI, closing issue #27; Tesserae moved onto it with nothing broken |
| M21 — Scroll Chaining | `███░░░░░░░` 33% | 🚧 In Progress — a wheel passes a view that can't move its way to the next one out |
| M22 — A Windows CI Cache That Saves | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M23 — Release `0.4.4` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** M12 (2026-09-29) -- issue [#24](https://github.com/mindderivative/tre/issues/24): scroll views beyond the wheel -- Page Up/Down keys; keys scroll the nearest scroll view around the focused node unless that node uses the key; `node.scroll_into_view()`, which focus and assistive technology also trigger; and a `scroll` event with `old_value`/`new_value` on any change of offset.

**Up next:** M21 Step 2 -- keys chain the same way.

**Known gaps:**
- None open on this line.

**Fixed gaps:**
- ~~Every frame repaints the whole window: `vello_hybrid` 0.2.0's public `Renderer::render` always clears the target and takes no scissor, and `tre` renders straight into the swapchain image, which keeps no previous frame. The idle loop sleeps when nothing changes (0.3.x, M29), so the cost is paid only while something animates -- but then it's the full window, however small the change. This line exists to close it.~~ Fixed in M5 (2026-09-28): each window renders only inside its damage rects, into a target that keeps its frame, and nothing when nothing changed.
- ~~CI's cargo-cache upload can hang with no timeout: on 0.4.2's release PR the Windows job's tests had passed, but `actions/cache`'s post-job save hung for over 8 minutes and the run had to be cancelled and re-run -- a step's `timeout-minutes` doesn't reach an action's post-job hook.~~ Fixed (2026-09-30, user: "yes, add the timeout to the cache step"), merged to `main` as PR #26 (`f92fb05`, 2026-09-30, user: "yes, merge PR #26"), its CI green on all four jobs -- though every save step was skipped on an exact cache hit, so the timed save first runs when `Cargo.lock` next changes: in `ci.yml`'s Linux, Windows, and macOS jobs the cache is restored with `actions/cache/restore` and saved at the end by its own `actions/cache/save` step, `timeout-minutes: 5` and `continue-on-error: true`, so a stuck upload skips the save instead of holding the job; it still saves only on an exact-key miss and a successful job.

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

**Status: ✅ Complete (2026-09-28).** User: "push and start M4". Issue #4's third piece, in `engine-core`: today's `Tree.dirty` is a whole-frame yes/no; partial redraw needs where.

### Phase 1 — Dirty Rects ✅
- Step 1: accumulate the painted bounds, before and after, of whatever changed each frame -- a ticking animation, a property set, a layout change, a structural change, a canvas redraw, a layer shown or hidden -- including transforms, shadows, and clipping — ✅ (new `engine_render::DamageTracker` returning `Damage::{None, Full, Rects}`; it compares rather than instrumenting -- mutations reach nodes through `get_mut`, direct writes inside `Tree`, ticks, layout, and tree-level focus, and a missed hook would be a stale-pixel bug under on-by-default partial redraw -- walking the tree with the paint walk's rules through helpers pulled out of `paint_node` (`composed_transform`, `transformed_bounds`, `clips_children`, behavior unchanged, all 111 `engine-render` tests passing) and recording per node its painted rect -- the box grown by shadows, a path's centered stroke, canvas commands past the box, and text that overflows it, measured by a new `TextRenderer::text_extent` that asks the paint path's own shaping cache with `draw`'s exact inputs, so it never reshapes; a single-line text input takes its row across its clip, since its text scrolls sideways unclipped -- transformed, clipped, rounded out with a 2 px margin; and a fingerprint of its paint properties, kind state, composed transform, effective group opacity, clip, place in its parent's paint order, and focus for text inputs and terminals, images by blob id rather than bytes; every state struct is destructured without `..`, so a new field won't compile until someone decides whether it paints; changed, new, and removed nodes give their old and new rects, overlapping rects merge, then the cheapest pairs until at most 4, and past half the window, or on a first frame, a resize, or `reset`, it's `Full`; `ARCHITECTURE.md` §6 describes it)
- Step 2: tests that every kind of change reports a rect covering it, and that a change spanning most of the window falls back to a full redraw — ✅ (new `tests/damage.rs`, 18 CPU-only tests, all passing on their first run: an unchanged frame gives `None` and a first frame `Full`; a paint change damages only near its node; a layout move damages both places; an animation damages each frame until it arrives, then nothing; a shadow widens the damage past the box; a transform damages where the node is drawn; removal and hiding damage where it was; a z-order swap of overlapping siblings; an ancestor's opacity damages its subtree; wrapped text overflowing its box; focusing a text input damages its row; a new image frame with identical bytes; a canvas drawing past its box; scrolling stays inside the viewport; a clipping parent bounds its child's damage; ten scattered changes merge into at most 4 non-overlapping rects covering all ten; a change to the whole window is `Full`; `reset` and a resize are `Full`; cargo 348 passed, pytest 430 passed, 1 skipped, 21 examples, clippy, fmt)

---

## Milestone 5 — Partial Redraw End to End, Measured

**Status: ✅ Complete (2026-09-28).** User: "yes" (push and start M5), then "continue".

### Phase 1 — Integration ✅
- Step 1: the frame loop renders only the dirty region through the patched renderer into the offscreen target, falling back to a full redraw on resize, scale change, or a large region — ✅ (2026-09-28: each window's `DamageTracker` runs after layout and before the scene is built; `None` renders nothing and re-presents the kept frame, `Rects` builds the scene with the new `build_tree_scene_in` -- culled to the rects' bounds, inside one clip layer of them -- and renders with `ClearSettings::Rects` through the new `FrameRenderer::render_into`, and `Full` renders as before; the tracker resets on a target recreate, a font registration, and a frame whose surface texture couldn't be acquired; `window.set(partial_redraw=False)` / `get("partial_redraw")` switch it off, on by default; 7 new GPU tests in `tests/partial_redraw.rs` find partial redraw byte-identical to full for a colour change under a translucent overlap, a move that uncovers, a shadow, text, group opacity, a removal, and no change; a live 800x600 window animating one card renders 1 full frame then 59 partial ones, and 60 full with the switch off)
- Step 2: correctness -- pixel tests comparing partial against full redraw across every example's animations — ✅ (2026-09-28: 9 new GPU tests in `tests/partial_redraw.rs` tick a real animation through 12 frames -- 10 in flight, then settling -- rendering each frame partially into one window and in full into another, and require identical bytes every frame plus at least one partial frame; they cover every property the examples animate -- opacity with corner radius, an eased fill retargeted halfway, scale with its shadow, shadows, translation over other nodes, rotation, a group's border, a path morph with its stroke trim, and a scroll view's offset; a mutation check shrinking every damage rect by 3 px fails 9 of the 16 tests, the 7 survivors being changes whose rects are deliberately generous; cargo 364)
- Step 3: measurement against `v0.3.5.1`, the same way M101 of the 0.3 line measured: frame time for a small animation in a large window, and no regression for a full-window change — ✅ (2026-09-28: new `#[ignore]`d `tests/partial_redraw_bench.rs` times tick, layout, damage, scene, render, copy, submit, and a wait for the GPU, over a 1920x1080 window of 576 cards on this machine's integrated Radeon 890M (Vulkan); `v0.3.5.1` measured with a port of it in a detached worktree, removed afterward, 5 alternating runs each; one 48x48 card animating: medians 0.56-1.00 ms against 2.16-4.78 ms, 3.7-4.9x less in every paired run; every card animating: 2.30-5.34 ms against 2.18-5.19 ms, paired runs 6% lower to 10% higher, within this GPU's clock noise; the first run found a real regression there -- 117-180 ms a frame, the damage merge's pair search being cubic in 577 rects -- fixed by merging past 64 rects straight into their bounding box, with 2 new `tests/damage.rs` tests (600 changes across the window give `Full` in under 500 ms even in a debug build; 100 in a corner stay one partial rect); `v0.3.5.2` changed nothing in `engine-render` or `engine-core`, so these figures hold for it too; cargo 366)

---

## Milestone 6 — Release `0.4.0`

**Status: ✅ Complete (2026-09-28).** `v0.4.0` released, and Tesserae on it.

### Phase 1 — Pre-release Review ✅
- Step 1: a full `/review-project` of the branch -- performance, architecture, security, modernization, each finding adversarially verified — ✅ (2026-09-28, user: "Before we move on to M6, lets do a full /review-project of tre"; [report](https://claude.ai/artifact/X4Xo6Zwk4spCRxnDTWQMk3); 33 findings, 32 confirmed, 20 fixed: a real partial-redraw bug -- a node's shadow or a child overflowing a non-clipping parent kept stale pixels when something else changed under it, since paint culled by layout box and skipped whole subtrees -- fixed by one shared painted-extent function (`damage::painted_rect`) both walks use, partial frames culling as full ones do and drawing a node only where that extent reaches a rect, 2 GPU reproducers; windows past the GPU's 8192 texture limit clamped instead of panicking, and a simulated resize validated; images past `MAX_IMAGE_DIMENSION` rejected from Python and otherwise skipped with a warning; an `Outdated` surface reconfigured and retried, `Suboptimal` reconfigured after presenting, and a skipped frame re-marks the tree dirty (new `Tree::mark_dirty`); the damage walk skipped when its answer is unused; list offsets, path geometry, and duplicate in-place rects out of the per-frame walk; a terminal damaged across its whole grid; same-size image frames written into their texture; clear and clip rects both rounded out; the release workflow's write token limited to `publish`; stale `vello_hybrid` comments, a hand-rolled bounds fold, stub typing, a duplicate unpin, over-wide API, and the 0.4.0 tests' duplicated GPU setup cleaned up; left for the user: one shared walker, a per-window renderer type, the exit-0 on no adapter, lost-surface recreation, presenting on unchanged frames, terminal/canvas hashing, MSRV and action SHA pins, and smaller structural items; cargo 370, pytest 433 + 1 skipped, 20 examples)

### Phase 2 — The Review's Decisions ✅
User, 2026-09-28: "1. Unify them 2. Move into a per-window 3. raise an exception 4. Yes 5. Fix them all".
- Step 1: MSRV from the real dependency floor with an `msrv` CI job; third-party release actions pinned to commit SHAs — ✅ (2026-09-28: `rust-version = "1.90"` for every crate -- the review expected 1.89 from `vello_gpu`, but `cargo +1.89 check` failed on `ordered-float` 5.5.0, which declares 1.90, the highest in the lock file, and `cargo +1.90 check --workspace --all-targets` passes; CI's new `msrv` job runs that check; `PyO3/maturin-action` pinned to `e83996d1` (v1) and `softprops/action-gh-release` to `3bb12739` (v2))
- Step 2: no GPU adapter raises a Python exception from `App.run()`; a zero-sized window is rejected — ✅ (2026-09-28: `GpuState::new` returns a `Result` -- no adapter, no device, a failed surface, or one the adapter can't drive, each formerly `exit(0)` or a panic -- and `engine-platform`'s window-created callback now returns whether setup worked, `false` ending the loop at once; `App.run()` raises the reason as `RuntimeError`; no display still returns `None`, as documented; `Window(0, h)` / `Window(w, 0)` raise `ValueError`; checked by hiding the Vulkan and GL drivers: `RuntimeError: couldn't create a GPU surface...`, with `atexit` still running; docs, stub, and 16 example docstrings updated; pytest 434 + 1 skipped)
- Step 3: one traversal shared by the paint walk and the damage walk — ✅ (2026-09-28: new `walk.rs` holds the rules once -- hidden, culled, and fully transparent nodes skipped with their subtrees, composed transforms, clip narrowing, paint-order children -- calling a `Visitor`'s `enter`/`leave`; the recursive `paint_node` became the `Painter` visitor (opacity and clip layers opened in `enter`, closed with the scroll thumbs in `leave`) around an unchanged `draw_own`, and the damage walk became the `Recorder` visitor; every pixel test unchanged, cargo 370)
- Step 4: a per-window renderer in `engine-render`, used by the app, the pixel tests, and the benchmark — ✅ (2026-09-28: new `engine_render::WindowRenderer` owns the frame renderer, text and geometry caches, persistent target, and damage tracker, with `prepare` (images, cache eviction, damage) and `draw` (render and copy to the surface image), `reset` and `resize`; `GpuState` keeps only the device, queue, and surface around it, and `engine-py` no longer depends on `vello_gpu` directly; `tests/partial_redraw.rs` and the benchmark drive the same type and read the stand-in swapchain image, so they check the app's own sequence, the copy included; 19/19 partial-redraw tests, live probe 1 full then 59 partial; the benchmark's whole-window case ran 0.2-1.8 ms slower with partial redraw on in 3 of 3 runs -- partly run order on this GPU, partly the damage walk fingerprinting all 577 nodes before settling on a full redraw, which Step 5's content generations reduce)
- Step 5: terminal and canvas content generations instead of hashing every cell and command — ✅ (2026-09-28, done the review's other way: a content counter bumped where cells and commands are written would be instrumentation, which `damage.rs` is built to avoid -- these states' fields are written directly, even by tests -- so the fingerprints stay complete and use a fast fixed-seed hash, `foldhash` 0.2.0, already in the lock file; a new ignored measurement, a 200x60 terminal and 2,000 canvas commands beside one changing node: 0.32-0.77 ms a frame with SipHash, 0.07-0.10 ms now; MSRV check still passes)
- Step 6: a lost surface recreated; no present for a tree-requested frame with no damage — ✅ (2026-09-28: `GpuState` keeps the wgpu instance and window, and a `Lost` surface is recreated, configured as before, and the frame retried; `engine-platform` records which redraws it requested itself and tells `on_frame` whether the OS asked instead; a loop-requested frame with no damage skips acquire, copy, and present -- unless something is animating, where the present's wait for the display is what keeps the loop from spinning through unchanged frames; an OS-requested frame with nothing changed now re-presents the kept frame, where before 0.4.0 an expose with nothing dirty presented nothing)
- Step 7: the window's shared fields in one struct; `get("partial_redraw_active")` — ✅ (2026-09-28: a `#[derive(Clone)] WindowHandles` holds the 11 handles a window shares with `App.run()` -- tree, root, size, handlers, dock, completions, terminals, listeners, OS window, the partial-redraw switch, and a new `surface_partial` -- defined once; `WindowState` is `{ handles, title }`, and the run's setup and runtime structs each hold one `handles` clone instead of 11 copied fields, so a new shared field is one line plus its initial value; `window.get("partial_redraw_active")` is `None` until the window opens, then whether partial redraw is really in effect (the setting and a surface that allows it) -- a live run read `None`, `True`, `None` before, during, and after)
- Step 8: the older GPU tests on `tests/support`'s shared setup and readback — ✅ (2026-09-28: 21 test files' own adapter-and-device setup replaced by `support::device`, and 20 files' buffer, copy, map, and poll readback by `support::read_texture` -- 1,142 lines out, 103 in; only `rect_window.rs`, which opens a real window's surface, keeps its own; a wgpu bump now edits `tests/support/mod.rs` alone; every pixel assertion unchanged, cargo 370)
- Step 9: images from `vello_gpu`'s image atlas, or why not — ✅ (2026-09-28, not moved, and why recorded in `image_cache.rs` and ARCHITECTURE.md: at the pinned commit, writing into an existing atlas allocation (`write_to_atlas`) is `pub(crate)`, so every video frame would be a `destroy_image` plus a new `upload_image` where tre's own texture is written in place; `upload_image` unwraps its allocation, so an image larger than a page panics; and the default page is 4096x4096, below the 8192 tre accepts)
- Step 10: `engine-core`'s `tree.rs` split by concern — ✅ (2026-09-28: the 9,207-line `tree.rs` became `tree/`: `mod.rs` (745 lines -- the struct and its core operations: insert, remove, attach, detach, tick, the dirty flag), `layout.rs`, `scroll.rs`, `layers.rs`, `focus.rs`, `text_editing.rs`, `dispatch.rs`, `access.rs`, and `tests.rs` (5,625); 77 methods moved whole by name with their docs, private ones made `pub(super)` so only `tree`'s own modules see them; no behaviour change -- cargo 370, pytest 434 + 1 skipped, MSRV check passes)

### Phase 3 — Release ✅
User: "Start phase 3".
- Step 1: PR to `main`, CI green on all three platforms, merge, re-verify, release note, annotated tag, as with `v0.3.5` — ✅ (2026-09-28: pushed; PR #17 conflicted with `main`, which had gained `v0.3.5.2` -- M105 `undock_panel`, M106 `get("dark")`, M107 PyPI as `tesserae-engine` -- so `main` was merged into `0.4.0` (`4001e66`): this tracker, LOG, and PLAN kept, `main`'s new tracker records applied to `BUILD_TRACKER_ARCHIVE_0.3.md`, `pyproject.toml` taking the name `tesserae-engine` with version 0.4.0 and the local `0.3.5.3` branch's `license-files` sdist fix, the new code on `WindowHandles`, `window.get` listing `dark` too, and the `pypi` job's publish action pinned; cargo 372, pytest 450 + 1 skipped, MSRV clean; CI green on Linux, macOS, Windows, and the new `msrv` job's first run; PR out of draft and merged (`dec815c`), its tree identical to the verified `4001e66`; annotated tag `v0.4.0` pushed; the wheels run passed every job, the GitHub release carries 24 assets and the release note, and PyPI has `tesserae-engine` 0.4.0 -- 22 wheels and, unlike 0.3.5.2, the sdist; a fresh venv installs it from PyPI and runs; issue #4 closed)
- Step 2: Tesserae moves to `0.4.0` — ✅ (2026-09-28: told -- the install, partial redraw and its switch, the four behaviour changes that could touch its code or tests, and Rust 1.90 for source builds; reported by its session: `tesserae-engine==0.4.0` from PyPI, its floor now `>=0.4.0` and its CI's font checkout at `v0.4.0` (local commit `f9f1147`), 2360 passed, 0 skipped, all five examples clean -- the +2 over 0.3.5.2 its own docs tests; each behaviour change checked -- `App.run()`'s `RuntimeError` reaches its callers and is documented, no zero-size windows, no oversized images, no test on the old error text; no `tre` issues found)

---

## Milestone 7 — Partial Redraw's Fixed Costs

**Status: ✅ Complete (2026-09-28).** User: "Do your recommendation" -- close M7 on its measurement. `v0.4.0`'s one worse number: a change covering most of the window costs 0.2-1.8 ms more a frame with partial redraw on, the tracker fingerprinting every node before settling on a full redraw.

### Phase 1 — Measure and Cut ✅
- Step 1: measure where a frame's time goes -- damage walk, scene, render, copy to the surface -- for the small and whole-window workloads — ✅ (2026-09-28: `partial_redraw_bench` now splits each frame into `prepare` (the damage walk), `draw` (scene and encoding) and the GPU wait, and adds an idle workload; three runs, 1920x1080, 576 cards: whole-window with partial redraw on vs off, `prepare` 0.10-0.12 ms vs 0.005 ms, with `draw` and GPU the same -- so the overhead is the walk, about 0.1 ms or 4-5% of the frame, and `v0.4.0`'s reported 0.2-1.8 ms gap was mostly this GPU's clocks and run order; idle frames with partial redraw on cost 0.33-0.38 ms, 0.25 ms of it the GPU copying the kept frame to the surface, which is also most of a small partial frame's 0.31 ms GPU time)
- Step 2: stop the damage walk once a frame is known to be a full redraw, and restart the comparison next frame — ✅ (2026-09-28, decided against, with the user: the walk is 0.10-0.12 ms of a 2.3 ms whole-window frame, and stopping early would save about half of it, since during a continuing whole-window change every other frame must still compare in full to have a baseline)
- Step 3: the copy to the surface: measure its share of a small partial frame, and cut it if it's worth it — ✅ (2026-09-28, measured at 0.25 ms at 1080p and kept: narrowing it to the damage needs to know how many frames old each swapchain image is -- they rotate, 2-3 of them -- and wgpu 30 doesn't expose buffer age, so a guess would show stale pixels; frames with nothing changed and nothing animating already skip it, M6 Phase 2 Step 6)

---

## Milestone 8 — Show What's Redrawn

**Status: ✅ Complete (2026-09-28).** User: "Do your recommendation" (M7 closed, on to M8).

### Phase 1 — The Overlay ✅
- Step 1: `window.set(show_damage=True)` tints each frame's redrawn areas, so partial redraw can be seen working and wasted redraws found — ✅ (2026-09-28: `WindowRenderer::draw_damage_overlay` draws each damage rect filled translucent magenta and outlined, or a full redraw's window edge in orange, over the surface image with the new `FrameRenderer::render_over` (`TargetInit::SrcOver`), in its own submit after the frame's -- two renders in one encoder would share the renderer's buffers, written as each is encoded -- and never into the kept frame; a `show_damage` switch in `WindowHandles`, `set`/`get`, stub, docs; a GPU test finds the damage tinted, the rest untouched, and the next frame without the overlay byte-identical to a full render, and a pytest covers the switch; the test's stand-in swapchain image gained `RENDER_ATTACHMENT`, as a real one has; cargo 373, pytest 451 + 1 skipped)

---

## Milestone 9 — Housekeeping and Release `0.4.1`

**Status: ✅ Complete (2026-09-29).** `v0.4.1` released, and Tesserae on it.

### Phase 1 — Housekeeping ✅
- Step 1: retire the local `0.3.5.3` branch and its `tre-0.3.5.2` worktree (its sdist fix shipped in `0.4.0`), prune merged local release branches, and move `rect_window.rs` onto the shared test helpers where it can — ✅ (2026-09-28: the clean `tre-0.3.5.2` worktree removed and branch `0.3.5.3` deleted -- never pushed, but both its commits' changes are on `main`, the `license-files` fix since `0.4.0` and its tracker line in the 0.3 archive; local branches `0.3.1`-`0.3.5.2` and `0.4.0`, all merged and on `origin`, and a merged M56-era `worktree-agent-...` branch deleted -- local branches are now `main` and `0.4.1`; `rect_window.rs` keeps its own setup, since it picks its adapter for a real window's surface, which the shared helper deliberately doesn't take; a new `examples/show_damage.py` for the manual check -- one pulsing card with the overlay on, printing `partial_redraw_active`, `--watch` to keep the window open -- listed in `docs/examples.md`)

### Phase 2 — Release ✅
- Step 1: the user's manual check of partial redraw on real hardware -- X11, Wayland, macOS, Windows: `partial_redraw_active`, and repaint on uncover, resize, and minimize — ✅ (2026-09-29, with `examples/show_damage.py --watch`: KDE Wayland on this machine ✅ -- `partial_redraw_active = True`, only the pulsing card tinted, the orange edge on resize, and correct after uncover and after minimize/restore (user: "all four looked good on Wayland"); X11 through XWayland on the same machine (`env -u WAYLAND_DISPLAY`) ✅ -- `partial_redraw_active = True` in both of its runs, the four checks good (user: "X11 looked good too"); macOS and Windows not checked by hand -- the user has neither ("I can not test on windows and macos") -- so there only CI's headless build and tests cover them, and the release note says so)
- Step 2: PR to `main`, CI green, merge, tag `v0.4.1`, release, PyPI — ✅ (2026-09-29, user: "works wonderfully start the release": `main` hadn't moved, so no merge into `0.4.1`; PR #22, CI green on Linux, macOS, Windows, and `msrv`; merged as `97d0b4a`, its tree identical to the tested `771fb2d`; annotated tag `v0.4.1`; the wheels run passed every job and the user approved the `pypi` deployment; the GitHub release carries 24 assets and the release note -- which says macOS and Windows are covered by CI only -- and PyPI has `tesserae-engine` 0.4.1, 22 wheels and the sdist; a fresh venv installs it from PyPI once the index caught up, `show_damage` and a simulated back button working; issue #21 closed by the merge)
- Step 3: Tesserae moves to `0.4.1` — ✅ (2026-09-29: reported by its session: `tesserae-engine==0.4.1` from PyPI, its floor `>=0.4.1` and its CI's checkout at `v0.4.1` (committed locally there); 2502 passed on 0.4.1 before its side-button work, as on 0.4.0, and 2503 after, 0 skipped, five examples clean; its `App` hears `pointer_down` on the root and maps `"back"`/`"forward"` to `back()`/`forward()` -- both reach the root over a node and over empty space, with no click and no focus change; no `tre` bugs found)

---

## Milestone 10 — The Mouse's Back and Forward Buttons ([issue #21](https://github.com/mindderivative/tre/issues/21))

**Status: ✅ Complete (2026-09-29).** User, before the `0.4.1` release: "before that check your issues" -- the one open issue, #21 from Tesserae's routing work (its M66, `app.back()`/`app.forward()`), folded into `0.4.1` ahead of M9's release phase.

### Phase 1 — The Side Buttons ✅
- Step 1: `PointerButton::Back`/`Forward`, translated from winit's `MouseButton::Back`/`Forward` (only `Other(n)` still goes unreported); `Event.button` names them `"back"` and `"forward"`, `simulate` accepts them, and like the middle button they focus nothing and make no `click` — ✅ (2026-09-29: `engine-core` dispatch treats them as it treats `Middle`, `pointer_down`/`pointer_up` bubbling to the root as the issue relies on; a Rust test that a side-button press/release activates nothing, the `engine-platform` translation test flipped, a pytest that both reach a root listener as `pointer_down`/`pointer_up` with no `click`, and the unknown-button error listing all five; stub and events docs; cargo 373, pytest 452 + 1 skipped; checked by the user with a real mouse's side buttons: "works wonderfully")

---

## Milestone 11 — CSS Grid Layout ([issue #23](https://github.com/mindderivative/tre/issues/23))

**Status: ✅ Complete (2026-09-29).** User: "yes, start M11". Taffy 0.14 already implements CSS Grid (its default `grid` feature); tre's layout properties, all parsed in `engine-py`'s `node_layout.rs`, are flexbox only. Values follow tre's conventions: plain numbers are pixels.

### Phase 1 — Grid ✅
- Step 1: `display` (`"flex"`, `"grid"`); `grid_template_columns`/`grid_template_rows` as a string (`"200 1fr auto"`, `"repeat(3, 1fr)"`, `"minmax(120, 1fr)"`, percentages, `auto`, `min_content`/`max_content`) or a list; `grid_column`/`grid_row` on a child (a line, `"span 2"`, `"1 / 3"`, `"auto"`) — ✅ (2026-09-29: a new `engine-py/src/grid.rs` parses and formats track lists and placements in plain Rust -- CSS's syntax in `tre`'s units, a plain number being pixels, CSS's hyphenated spellings accepted, errors saying what's wrong -- with 4 unit tests; Taffy's own CSS parser (its `parse` feature) was passed over, since it needs `px` units and would add `cssparser`; `node_layout.rs` takes a string or a list, whose numbers are pixels; named lines and template areas left for later)
- Step 2: `grid_auto_rows`/`grid_auto_columns`, `grid_auto_flow` (`row`/`column`, with `dense`), `row_gap`/`column_gap` (`gap` still sets both), `justify_items`, `justify_self`, `align_content`; `get()` reads each back as set, and the unknown-property error lists them — ✅ (2026-09-29, done with Step 1 in the same parser: 13 layout properties in all, `grid_auto_flow`'s `"dense"` reading back as `"row dense"`; `row_gap` is Taffy's `gap.height` and `column_gap` its `gap.width`)
- Step 3: headless layout tests, a grid section in the layout guide, an `examples/grid.py`, and the stub — ✅ (2026-09-29: `tests/test_grid.py`, 15 tests -- round trips, a list form, computed widths for `100 1fr` and `repeat(3, 1fr)` with gaps, flow into a second row, spans, a negative line, and 8 bad values' errors; a Grid layout section in the guide and in the properties reference; `examples/grid.py`, a form whose fields line up and a gallery of `repeat(auto_fill, minmax(96, 1fr))` with a spanning tile, checking its own geometry; the stub lists no layout properties, so needed nothing; cargo 377, pytest 467 + 1 skipped)

---

## Milestone 12 — Scroll Views: Keys, `scroll_into_view`, Focus, and a `scroll` Event ([issue #24](https://github.com/mindderivative/tre/issues/24))

**Status: ✅ Complete (2026-09-29).** User: "yes, start M12". User's decisions (2026-09-29): keys scroll the nearest scroll view around the focused node when that node doesn't use the key itself; `scroll` fires on any change of offset, found by comparison; keyboard and `scroll_into_view` scrolls jump rather than ease.

### Phase 1 — Scrolling Beyond the Wheel ✅
- Step 1: `Key::PageUp`/`PageDown` -- platform translation, `simulate` names, `key_down` — ✅ (2026-09-29: the two `Key` variants, translated from winit's `NamedKey::PageUp`/`PageDown` (a test added); `simulate` takes `page_up`/`page_down`; `key_down` already named them, from `engine-platform`'s `key_name`; a terminal sends xterm's `\x1b[5~`/`\x1b[6~`; a text field doesn't take them, so they're left for the scroll view around it; until Step 2 they have no default action)
- Step 2: keyboard scrolling -- arrows by 40 px, Page Up/Down by the viewport, Home/End to the ends -- of the nearest scroll view around the focused node, when the focused node doesn't use the key (a text field keeps its arrows, Home, and End), or of a focused scroll view itself — ✅ (2026-09-29: `Tree::scroll_view_for_key` picks the nearest scroll view, from the focused node up, along the key's axis -- Up/Down and Page Up/Down vertical, Left/Right horizontal, Home/End either -- and `Tree::scroll_by_key` jumps by `KEY_SCROLL_LINE` (40 px), a viewport, or to an end; `engine-py`'s `keyboard_scroll` runs after the key's listeners and applies the rule: no scroll when a node from the focused one up to the scroll view has its own `key_down` listener (a box-built slider; Tesserae's own keyboard scrolling) or is a text input and the key isn't Page Up/Down -- core can't know about listeners, and tre has no `preventDefault`; nothing focused, nothing scrolls; 6 pytest cases: each key and clamping, a text input, listeners on the item and on the view, a carousel inside a page, a focused scroll view, no focus; cargo 377, pytest 473 + 1 skipped)
- Step 3: `node.scroll_into_view()` and the accessibility action of the same name: every enclosing scroll view scrolls just enough to show the node; a focus change (Tab, `node.focus()`) reveals the newly focused node the same way — ✅ (2026-09-29: `Tree::scroll_into_view` walks the node's scroll views innermost first, scrolling each by the least that brings the node's box inside its viewport (to the box's start when it's longer), then shifting the box by that much for the next view out; it reads the last layout, which `simulate`, every frame, `node.focus()`, and `node.scroll_into_view()` keep current; `transition_focus` calls it for every newly focused node -- Tab, click, `focus()`, assistive technology; `deliver_a11y_action` calls it after the listeners for `scroll_into_view`, which can't double-scroll since revealing a visible node changes nothing; stub; 5 pytest cases -- just enough and nothing when visible, the accessibility action, `focus()` both ways, Tab, and nested views; cargo 377, pytest 478 + 1 skipped)
- Step 4: a `scroll` event on the scroll view, `old_value`/`new_value`, whenever its offset changes from any cause -- wheel, keys, `scroll_into_view`, focus, `set`, or an animation (each frame) -- found by comparing offsets after each dispatched input and each tick — ✅ (2026-09-29: `ScrollViewState` gained `reported`, the offset last announced, and `Tree::take_scroll_changes` returns each view whose offset moved since, a no-op walk-free return when the tree has no scroll views; `engine-py` fires `scroll`, which doesn't bubble, after every dispatched input, each frame's animation tick, `simulate`'s layout pass, an assistive technology's `scroll_into_view`, and `Node.focus`/`scroll_into_view`/`set`; the damage fingerprint ignores `reported`, which paints nothing; the stub and events reference list the event; 3 new pytest cases, 481 passed and 1 skipped, 377 cargo tests)
- Step 5: a docs note that one content box with `flex_shrink=0` is what scrolls; tests and example updates — ✅ (2026-09-29: checking the note first showed it was wrong -- a content box keeps its length in either orientation, with or without `flex_shrink`, so the guide doesn't ask for it and the test helper's docstring no longer claims it; the guide's Scrolling section gained keyboard scrolling, revealing a node, and the `scroll` event; ARCHITECTURE.md §11.7a covers the key rule's split between `engine-core` and `engine-py`, reveal, and the compared event; new `examples/scroll_keys.py` -- Tab reveals, each key, `scroll_into_view`, and a status line fed by `scroll`, all checked before rendering; cargo 377, pytest 481 + 1 skipped, every example, mkdocs strict)

---

## Milestone 13 — Release `0.4.2`

**Status: ✅ Complete (2026-09-30).** `v0.4.2` released, and Tesserae on it. User, before the release: "I want a complete audit on the MkDocs and the standard documentation. We need to make sure the docs are up to date, have no errors, have no old entries, and explains everything."

### Phase 1 — Release ✅
- Step 1: documentation audit -- every MkDocs page, the README, ARCHITECTURE.md, and the stub, checked against the code with every snippet run — ✅ (2026-09-29: five read-only audit agents over the intro pages and README, the two halves of the guide, the Python API reference with the stub, and ARCHITECTURE with the design and migration pages, each claim they reported re-checked before editing; about 70 fixes -- wrong: `justify_content` has no `"baseline"`, grid line `-1` is past the last column, the stub's `Node.get(property=)` keyword (the runtime's is `name`), `simulate`'s modifiers only on pointer, wheel, click, and key events, `animate`'s `TypeError`, icons as never-hit targets, engine-core's never-built `AppHandler`, `accesskit` "pinned exactly", Painter's unbuilt `fill_path`/`draw_text`, the root as a `box`, and the grid form snippet whose `1fr` column came out 0 wide; stale: 0.3.x versions in the install page, the tracker described as all history, the target-API page still "being implemented", the 0.3.3 migration page and its removed tool, a resolved partial-redraw risk; missing: grid and partial redraw in the overview pages, a Redrawing section in the painting guide, keyboard scrolling and focus reveal in the accessibility, events, and widget guides, `scroll_into_view` in the Node reference, the side buttons and `wheel` in the events guide, Linux build packages and the MSRV, text-input style limits, a terminal's read-only `text`, the `scroll`/`dismiss` events in the stub, and a new `docs/migrating-0.4.md` listing 0.4.x's behavior changes; mkdocs strict, rustdoc `-D warnings`, mypy strict, and pytest 481 + 1 skipped clean; stubtest left with only a hand-written PyO3 stub's structural differences)
- Step 2: the project named Tesserae Engine throughout the documentation — ✅ (2026-09-29, user: "Change the project name in the docs to Tesserae Engine"; the MkDocs site name and page titles, and every prose mention of the project in the README, ARCHITECTURE.md, and the docs site, now say Tesserae Engine, with "imported as `tre`" at the first mention on the entry pages; what names code keeps `tre` -- `import tre`, the `tre` package and `tre._core`, the repository and its URLs, the crates, PyPI's unrelated `tre` project, and the default window title `"tre v2"`; the `docs/design/` pages, dated records of the 0.3.x design, and the type stub are unchanged; mkdocs strict clean)
- Step 3: PR to `main`, CI green, merge, tag `v0.4.2`, release, PyPI — ✅ (2026-09-30, user: "yes, push and start the release": `main` hadn't moved, so no merge into `0.4.2`; PR #25, CI green on Linux, macOS, Windows, and `msrv` -- the Windows job's tests passed but its post-job cargo-cache upload hung for over 8 minutes, so the run was cancelled and the Windows job re-run, which passed; merged as `2fcc381`, its tree identical to the tested `1180f06`; annotated tag `v0.4.2` ("Tesserae Engine 0.4.2"); the wheels run passed every job and the user approved the `pypi` deployment; `tesserae-engine==0.4.2` installed from PyPI in a fresh venv reports 0.4.2 and has grid layout and `scroll_into_view`; issues #23 and #24 closed by the merge; the docs site redeployed under the new name)
- Step 4: Tesserae moves to `0.4.2` — ✅ (2026-09-30: reported by its session: the release's 24 assets and PyPI's 22 wheels plus sdist checked; its floor now `>=0.4.2` and its CI's and release workflow's checkout at `v0.4.2` (committed locally there); 2563 passed and 1 failed straight after the move, all five examples clean, then 2565 passed and 0 failed -- the failure was behaviour change 2: its own M71 scroll view had supplied keys, focus reveal, and `scroll_into_view` itself, and tre's reveal now ran before its focus listener, so its two-way `scroll_offset` binding missed the change; probing also showed its `key_down` listener on the scroll view kept every key from tre, per the key rule; fixed on its side by dropping its own keys and reveals and following the `scroll` event, with a new test pinning tre's key rule; no tre bugs; its observations, not bugs: the keys scroll with modifiers held too (Ctrl+Page Down included), and a `scroll_offset` set past the end reads back as set until layout clamps it, then fires a second `scroll` with the clamped value; grid isn't used in Tesserae yet, its M74)

---

## Milestone 14 — Keyboard Scrolling Leaves Shortcuts Alone

**Status: ✅ Complete (2026-09-30).** User: "yes, start M14, we will hold off on the release until we have some other fixes". Scoped (2026-09-30): "yes, push it and scope both as 0.4.3" -- the first of Tesserae's two 0.4.2 observations: arrows, Page Up/Down, and Home/End scroll the nearest scroll view even with Ctrl, Alt, or Meta held, so Ctrl+Page Down (a tab switch in browsers) or Alt+Left (back) also scrolls. Plan: with Ctrl, Alt, or Meta held, `engine-py`'s `keyboard_scroll` leaves the key alone; Shift still scrolls, as it does in a browser. The rule lives in `engine-py`, which already tracks every modifier (`listeners::modifiers()`); core's `KeyPressed` carries only Shift.

### Phase 1 — Modifiers ✅
- Step 1: no keyboard scroll while Ctrl, Alt, or Meta is held; Shift unchanged; pytest cases for each modifier, plain keys, and Shift — ✅ (2026-09-30: `keyboard_scroll` returns early when `listeners::modifiers()` has Ctrl, Alt, or Meta; `simulate` holds its modifiers for the whole dispatch through `with_modifiers`, so the tests take the live path; 10 new pytest cases -- each of the three modifiers with the arrow, Page Down, and End keys scrolling nothing, then the same key unmodified scrolling, and Shift+Page Down scrolling a viewport; cargo 377, pytest 491 + 1 skipped)
- Step 2: the guide's Keyboard scrolling section, the events guide, and a 0.4.3 section in `docs/migrating-0.4.md` — ✅ (2026-09-30: the rule in the layout guide's Keyboard scrolling section and the events guide's focus section; `docs/migrating-0.4.md` gains a 0.4.3 section and its intro now covers all of 0.4.x; ARCHITECTURE.md §11.7a names the rule and where the modifiers come from; mkdocs strict clean)

---

## Milestone 15 — `scroll_offset` Clamped When Set

**Status: ✅ Complete (2026-09-30).** User: "yes, merge PR #26 and start M15". Scoped (2026-09-30): "scope both as 0.4.3" -- Tesserae's second observation: `set(scroll_offset=...)` past the end reads back as set until the next layout clamps it, which then fires a second `scroll` event with the clamped value. Found while scoping (checked 2026-09-30): an offset given to `create` never becomes the `scroll` event's baseline -- `reported` starts at 0 -- so the first event's `old_value` is 0 instead of that offset. Plan: `Node.set`'s and `animate`'s `scroll_offset` run layout, then clamp to the view's range, so the read-back is right at once and one `scroll` event fires with the clamped value, and an animation eases to the real end rather than stalling there; `create` keeps today's rule (clamped at the first layout), so an offset given before the content is attached still survives adding it; a view's initial offset is its reported baseline.

### Phase 1 — Clamping ✅
- Step 1: `set` and `animate` clamp `scroll_offset` against a fresh layout; one `scroll` event; pytest for past the end, below the content, a shrinking content box, and an animation's target — ✅ (2026-09-30: new `Tree::max_scroll` gives a view's range from the last layout, and layout's own clamp now uses it; `Node.set` with `scroll_offset` runs layout before reporting, so the value reads back clamped at once and one `scroll` fires; `animate` lays out and clamps its target, so an animation past the end eases to the real end instead of stalling there; `create` unchanged; 4 new pytest cases -- 5000 in a 900-pixel range reads back 900 with the one event (0, 900), a value inside the range kept, shrinking content clamping 800 to 400 with one event, and a linear animation to 5000 at 450 halfway and 900 at the end; cargo 377, pytest 498 + 1 skipped)
- Step 2: the create-time offset is the `scroll` event's baseline; a pytest that the first event's `old_value` is that offset — ✅ (2026-09-30: `Window.create` sets a new scroll view's `reported` to its offset once its props are applied, so a view created at 500 fires nothing on its first frame, where it used to fire (0, 500), and its first move reports (500, 600), not (0, 600); 1 new pytest case; cargo 377, pytest 499 + 1 skipped)
- Step 3: the property reference, the guide, and the 0.4.3 migration section — ✅ (2026-09-30: the property reference's `scroll_offset` row says `set` and `animate` clamp at once and `create` at the first layout; the layout guide's Scrolling section says an offset past the end is the end; the 0.4.3 migration section lists the clamp, the one event, the eased animation, and the created baseline; ARCHITECTURE.md §11.7a names `Tree::max_scroll` and where each path clamps; mkdocs strict clean)

---

## Milestone 16 — Grid: A Bare Number as a One-Track List ([issue #27](https://github.com/mindderivative/tre/issues/27))

**Status: ✅ Complete (2026-09-30).** User: "Check your issues" -- the one open issue, #27, filed from Tesserae's M74 (the grid properties in its view styles): the four track-list properties took a string or a list, but a bare number raised `ValueError`, although `grid_column=2` takes one; in YAML the common case is written `grid_auto_rows: 96`.

### Phase 1 — The Fix ✅
- Step 1: a number (int or float, not a bool) is a one-track list, as if `[n]`, reading back as its text; tests; the property reference, the grid guide, and the 0.4.3 migration section — ✅ (2026-09-30: `node_layout.rs`'s `track_list` takes a bare number through the same `non_negative` check list items use, so a bool or a negative number is still refused, and the error names the new form; 3 new pytest cases -- a bare number read back and laid out as a 100px column and 30px auto rows, a float, and a bool and a negative number refused; cargo 377, pytest 494 + 1 skipped)

---

## Milestone 17 — Shift+Wheel Scrolls Horizontal Views

**Status: ✅ Complete (2026-09-30).** User: "yes, start  M17". Scoped (2026-09-30): "Scope 1, 2, and 3 for 0.4.3. Then we will look at releasing if there is nothing else after those." Found while recommending (checked 2026-09-30): a horizontal scroll view scrolls only on a wheel's horizontal part, which most mice don't have -- a plain wheel and Shift+wheel both left it at 0 -- where browsers, GTK, and Qt turn Shift+wheel horizontal. Core's `InputEvent::Scroll` carries no modifiers; `engine-py` tracks them (M14's rule lives there too). Plan: with Shift held, a wheel with no horizontal part is delivered to core's dispatch as horizontal, so the nearest horizontal scroll view scrolls; a wheel that already has a horizontal part (macOS turns Shift+wheel horizontal itself) is left alone; a terminal's scrollback still reads the vertical part; `wheel` listeners get the delta as delivered.

### Phase 1 — Shift+Wheel ✅
- Step 1: the mapping in `engine-py`'s input path; pytest for Shift+wheel on a horizontal view, a vertical view under Shift, an already-horizontal wheel, and a terminal — ✅ (2026-09-30: `engine-py`'s new `shift_wheel` turns a wheel with Shift held and no horizontal part horizontal before core's dispatch and the listeners, leaving a wheel with a horizontal part alone, and the terminal's scrollback path reads the wheel as it came; found while building it (probed 2026-09-30): core's wheel walk stopped at the first scroll view under the pointer even when the wheel had no part along its axis, so a plain wheel over a horizontal carousel inside a vertical page scrolled neither, and Shift+wheel would have failed the same way nested -- `Tree::dispatch` now passes a scroll view or virtual list the wheel can't move on to the next one out, as `scroll_view_for_key` does for keys; 5 new pytest cases -- Shift+wheel on a horizontal view, a plain wheel over a carousel scrolling its page, Shift+wheel over the carousel scrolling it and below it scrolling nothing, a wheel already horizontal left alone, and a `wheel` listener hearing (60, 0) with Shift -- and the terminal test's scrollback wheel now holds Shift; cargo 377, pytest 504 + 1 skipped)
- Step 2: the guide's Scrolling section, the events reference, and the 0.4.3 migration section — ✅ (2026-09-30: the layout guide says the wheel scrolls the nearest view along its direction, with the carousel case and Shift+wheel; the events guide and the events reference's `delta_x` row say Shift+wheel arrives as `delta_x`; the 0.4.3 migration section lists Shift+wheel and the wheel passing views it can't move; ARCHITECTURE.md §11.7a gains the wheel's rule and where Shift is applied; mkdocs strict clean)

---

## Milestone 18 — Unit Tests for the Scroll Core

**Status: ✅ Complete (2026-09-30).** User: "yes, start M18". Scoped (2026-09-30): "Scope 1, 2, and 3". The scroll API added in M12 and M15 -- `scroll_view_for_key`, `scroll_by_key`, `scroll_into_view`, `take_scroll_changes`, `max_scroll` -- is tested only through Python; `cargo test` has stayed at 377 since. Plan: `engine-core` unit tests in `tree/tests.rs`, beside the existing scroll view tests, with no Python or GPU.

### Phase 1 — Tests ✅
- Step 1: `scroll_view_for_key` (each key's axis, nearest first, Home/End either axis, none found), `scroll_by_key` (40 px, a viewport, the ends, clamping, the dirty flag), `scroll_into_view` (least movement, a box longer than its view, nested views, nothing when visible), `take_scroll_changes` (reported once, then quiet), and `max_scroll` (content that fits, no content, horizontal) — ✅ (2026-09-30: 9 tests in `tree/tests.rs` with two small helpers, `offset_of` and `view_over_boxes`: `max_scroll` along each axis, for content that fits, and `None` without content or for a plain node; `scroll_view_for_key` for every key on each axis, on a focused view itself, and past a carousel for the page around it; `scroll_by_key`'s line, page, and ends, clamping, and the dirty flag set only when something moved; `scroll_into_view`'s least movement, nothing when visible, and a long box aligned to its start; `take_scroll_changes` once per move and quiet without scroll views; and M17's rule, a vertical wheel over a carousel scrolling its page and a horizontal one the carousel, checked to fail with M17's core change reversed; cargo 386, pytest 504 + 1 skipped)

---

## Milestone 19 — Stub Drift Checked in CI

**Status: ✅ Complete (2026-09-30).** User: "yes, start M19". Scoped (2026-09-30): "Scope 1, 2, and 3". The docs audit (M13) found the stub's `Node.get(property=)` against the runtime's `name` -- `mypy` accepted a call that raised. Found while scoping: CI runs neither `mypy --strict` nor `stubtest`, though the README and the contributing guide list `mypy --strict` as a check. `stubtest` reports 21 differences today, all structural. Plan: make the stub match what PyO3 builds, then check both in CI.

### Phase 1 — Stub and CI ✅
- Step 1: the stub declares `Window`'s constructor as `__new__`, marks the PyO3 classes `@final`, makes `Node.__eq__`'s argument positional-only, and declares `__all__`; an allowlist names only what exists in the stub alone (the `Color` alias); `stubtest` clean locally — ✅ (2026-09-30: `_core.pyi` declares `__all__` as the runtime's seven names, marks `App`, `Window`, `Node`, `Painter`, `Event`, and `LoopHandle` `@final` (which `stubtest` counts as PEP 800's `@disjoint_base` too, and asks for alone), declares `Window`'s constructor as `__new__` as PyO3 builds it, and makes `Node.__eq__`'s argument positional-only; new `tools/stubtest_allowlist.txt` names only `tre._core.Color`, a type alias for checkers; `stubtest` 21 differences to 0, `mypy --strict` clean on the package and an example, pytest 504 + 1 skipped)
- Step 2: CI's Linux job installs `mypy` and runs `mypy --strict python/tre` and `stubtest` against the built extension — ✅ (2026-09-30: `ci.yml`'s Linux job installs `mypy>=2.3,<3` beside maturin and pytest, and after the import smoke test runs `mypy --strict python/tre` and `stubtest` with `tools/stubtest_allowlist.txt`; the contributing guide and the README list the `stubtest` command, run after `maturin develop`; both pass locally; their first CI run is 0.4.3's release PR)

---

## Milestone 20 — Release `0.4.3`

**Status: ✅ Complete (2026-09-30).** `v0.4.3` released, and Tesserae on it. User: "ok release it then". Held until M17–M19 joined -- user (2026-09-30): "we will hold off on the release until we have some other fixes".

### Phase 1 — Release ✅
- Step 1: PR to `main`, CI green, merge, tag `v0.4.3`, release, PyPI — ✅ (2026-09-30: `main` was already in `0.4.3` (PR #26 merged earlier), so no merge into it; PR #29, CI green on Linux, macOS, Windows, and `msrv` -- `mypy --strict` and `stubtest` passing on their first CI run, and the cargo cache saved in every job for the first time since PR #26, though Windows's save ran exactly its 5-minute limit, so it likely timed out and saved nothing; merged as `2a5e7ef`, its tree identical to the tested branch; annotated tag `v0.4.3` ("Tesserae Engine 0.4.3"); the wheels run passed every job and the `pypi` deployment was approved; `tesserae-engine==0.4.3` installed from PyPI in a fresh venv reports 0.4.3 -- a bare `grid_auto_rows=96`, Shift+wheel scrolling a horizontal view 60, and 5000 clamped to 800; issue #27 wasn't linked by the PR's "Closes #27" (GitHub listed no closing issues), so it was closed by hand with a comment naming the fix)
- Step 2: Tesserae moves to `0.4.3` — ✅ (2026-09-30: reported by its session, its M79: the release's 24 assets and PyPI's 22 wheels plus sdist checked, #27 closed; its floor `>=0.4.3` and its CI's and release workflow's checkout at `v0.4.3` (committed locally there, `40e2ab1`); 2624 passed unchanged, then 2630 with new tests -- a two-way-bound `scroll_offset` set past the end reading back as the end with one `scroll` writing it back, and Page Down with Ctrl, Alt, or Meta not scrolling but with Shift scrolling; all five examples clean; it dropped its own bare-number conversion for track lists (#27), its grid tests unchanged; Shift+wheel's `delta_x` touched nothing, its scroll view being vertical only; no tre bugs; its observation, not a bug: a vertical scroll view inside another keeps a vertical wheel even when it can't move -- its content fits, or it's at its end -- where browsers chain the wheel to the view outside; it offered to file that as a feature, and pinned today's behaviour in a test)

---

## Milestone 21 — Scroll Chaining

**Status: 🚧 In Progress.** User: "yes, keys should chain too, start M21". Step 1 done (2026-09-30). Scoped (2026-09-30): "scope 1 and 2 as 0.4.4" -- Tesserae's 0.4.3 observation: a scroll view keeps a wheel along its own axis even when it can't move -- its content fits, or it's already at that end -- so a vertical view inside a vertical page stops the wheel there and the page doesn't scroll. Browsers chain: a wheel a box can't use goes on to the box outside it. M17 made a view pass a wheel with no part along its axis; this makes it pass one it can't move by, either. Plan: in core's wheel walk, a scroll view or virtual list passes the wheel on when it can't move in the wheel's direction -- nothing to scroll, or at that end; a view that can move at all takes the whole wheel, clamped, as now (no splitting one wheel across two views). Keys (Step 2) follow the same rule -- the user agreed (2026-09-30), browsers chain keyboard scrolling too -- so Page Down at the end of an inner list moves the page. Tesserae has a test pinning today's behaviour.

### Phase 1 — Chaining 🚧
- Step 1: the wheel: core tests and pytest for content that fits, each end, a view mid-way, a virtual list at its end, and nesting — ✅ (2026-09-30: new `Tree::can_scroll(id, delta)` says whether a scroll view or virtual list can move in a direction -- something to scroll, and not at that end -- and core's wheel walk keeps the wheel only at a view that can, M17's no-part-along-the-axis case included (a zero delta can't move anything); a view that can move takes the whole wheel, clamped; 1 core test -- an inner view mid-way, back at its top with the page at its top too, at its end passing to the page, and with content that fits -- and 3 pytest cases -- an inner view mid-way, clamped, then passing at its end; one with nothing to scroll; a virtual list at its end; cargo 387, pytest 507 + 1 skipped)
- Step 2: keys: `scroll_view_for_key` picks the nearest view that can move in the key's direction; tests — ⬜
- Step 3: the guide's Scrolling section, ARCHITECTURE.md §11.7a, and a 0.4.4 migration section — ⬜

---

## Milestone 22 — A Windows CI Cache That Saves

**Status: ⬜ Proposed.** User (2026-09-30): "scope 1 and 2 as 0.4.4". On 0.4.3's release PR the Windows job's cache save ran exactly its 5-minute limit (PR #26's timeout, working as meant: the job passed instead of hanging) and likely saved nothing. Measured while scoping (2026-09-30): the Windows cache is 3.3 GB against Linux's 2.5 GB (saved in 27 s) and macOS's 1.2 GB; one Windows save did finish, on `main` at 05:46, and restoring it takes about 2 minutes, after which the build takes 1 -- so a cached Windows build is worth keeping, and the save is only needed after a `Cargo.lock` change. The repository's caches total about 8.8 GB against GitHub's 10 GB limit, past which old ones are evicted. Windows's key differs from Linux's and macOS's for the same `Cargo.lock`, likely its checkout's line endings -- harmless, since keys are per OS. Plan: measure, then choose.

### Phase 1 — Measure and Fix ⬜
- Step 1: measure a Windows build with no cache, with only the registry cached, and with everything cached, and a full cache's upload time — ⬜
- Step 2: apply the cheapest shape that saves inside a limit -- cache less on Windows (the registry, or the dependencies' build output without the workspace's own), or a longer limit -- and keep the total under GitHub's 10 GB — ⬜

---

## Milestone 23 — Release `0.4.4`

**Status: ⬜ Proposed.**

### Phase 1 — Release ⬜
- Step 1: PR to `main`, CI green, merge, tag `v0.4.4`, release, PyPI — ⬜
- Step 2: Tesserae moves to `0.4.4` — ⬜

---

## Branch: `0.4.4` — Scaffold

**Status: ✅ Scaffolded (2026-09-30).**

- Branch `0.4.4` created off `main` at `176a7f0` (`v0.4.3` plus its release records) — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.4`; `Cargo.lock` updated via `cargo metadata` — ✅

---

## Branch: `0.4.3` — Scaffold

**Status: ✅ Scaffolded (2026-09-30).**

- Branch `0.4.3` created off `main` at `6b7c70f` (`v0.4.2` plus its release records) — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.3`; `Cargo.lock` updated via `cargo metadata` — ✅

---

## Branch: `0.4.2` — Scaffold

**Status: ✅ Scaffolded (2026-09-29).**

- Branch `0.4.2` created off `main` at `30c7f27` (`v0.4.1` plus its release records) — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.2`; `Cargo.lock` updated via `cargo check` — ✅

---

## Branch: `0.4.1` — Scaffold

**Status: ✅ Scaffolded (2026-09-28).**

- Branch `0.4.1` created off `main` at `53bb1bb` (`v0.4.0` plus its release records) — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.1`; `Cargo.lock` updated via `cargo check` — ✅

---

## Branch: `0.4.0` — Scaffold

**Status: ✅ Scaffolded (2026-09-27).**

- Branch `0.4.0` created off `main` at `a67376d` — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.4.0` (from `0.3.5` and `0.3.5.1`); `Cargo.lock` updated via `cargo check` — ✅
- The 0.3.x tracker moved to `BUILD_TRACKER_ARCHIVE_0.3.md`; this tracker started at M1 — ✅
- GitHub: a `0.4.0` milestone holding issue #4, and a `0.3.5.x` milestone for fixes to the released `0.3.5` — ✅
