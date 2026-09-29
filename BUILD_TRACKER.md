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
| M8 — Show What's Redrawn | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M9 — Housekeeping and Release `0.4.1` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** M7 (2026-09-28) -- partial redraw's fixed costs, measured: with partial redraw on, a whole-window change pays only the 0.1 ms damage walk (4-5%; the rest of `v0.4.0`'s reported gap was GPU clocks and run order), and a small partial frame's copy to the surface is 0.25 ms, which can't be narrowed safely without swapchain buffer age. Neither cut is worth making.

**Up next:** M8 -- `window.set(show_damage=True)`, tinting each frame's redrawn areas.

**Known gaps:**
- None open on this line.

**Fixed gaps:**
- ~~Every frame repaints the whole window: `vello_hybrid` 0.2.0's public `Renderer::render` always clears the target and takes no scissor, and `tre` renders straight into the swapchain image, which keeps no previous frame. The idle loop sleeps when nothing changes (0.3.x, M29), so the cost is paid only while something animates -- but then it's the full window, however small the change. This line exists to close it.~~ Fixed in M5 (2026-09-28): each window renders only inside its damage rects, into a target that keeps its frame, and nothing when nothing changed.

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

**Status: ⬜ Proposed.**

### Phase 1 — The Overlay ⬜
- Step 1: `window.set(show_damage=True)` tints each frame's redrawn areas, so partial redraw can be seen working and wasted redraws found — ⬜

---

## Milestone 9 — Housekeeping and Release `0.4.1`

**Status: ⬜ Proposed.**

### Phase 1 — Housekeeping ⬜
- Step 1: retire the local `0.3.5.3` branch and its `tre-0.3.5.2` worktree (its sdist fix shipped in `0.4.0`), prune merged local release branches, and move `rect_window.rs` onto the shared test helpers where it can — ⬜

### Phase 2 — Release ⬜
- Step 1: the user's manual check of partial redraw on real hardware -- X11, Wayland, macOS, Windows: `partial_redraw_active`, and repaint on uncover, resize, and minimize — ⬜
- Step 2: PR to `main`, CI green, merge, tag `v0.4.1`, release, PyPI — ⬜
- Step 3: Tesserae moves to `0.4.1` — ⬜

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
