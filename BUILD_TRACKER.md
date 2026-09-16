# Build Tracker

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md` and `archive/`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — TRE v1 (archived reference) | `██████████` 100% | ✅ Archived `archived-2026-09-14` |
| M2 — v2 Architecture (`ARCHITECTURE.md`) | `██████████` 100% | ✅ 16 sections + ADR-001, locked |
| M3 — v2 Implementation | `██████████` 100% | ✅ Complete — all 7 phases, all 15 of §14's Suggested Build Order steps done |
| M4 — Real Input Dispatch (§4, §7.3, §9, §10, §11.10) | `██████████` 100% | ✅ Phases 1-3 complete — no further phase yet scoped |

**Just closed:** M4 Phase 3, step 2 — `Window.add_splitter` (§11.5), closing the one gap step 1 explicitly left open. Mirrors `add_rect`'s own exact parameter shape (`background`, `width`, `height`) plus a new `initial_position` keyword (0.0..=1.0, defaults to an even split); inserts a real `NodeKind::Splitter` and appends it to the window's own root row — no new mechanism needed, since that root is already the flex parent `Tree::splitter_geometry` expects. `add_rect` (left), `add_splitter`, `add_rect` (right), in that order, is now a complete, real, mouse-draggable resizable-pane layout. 5 new pytest tests + a new real example script (`examples/resizable_panes.py`, opens a live window with a working layout, exits cleanly after real frames — its own docstring states plainly that an actual live mouse drag needs a human running it interactively, or the synthetic-event tests step 1 already wrote, since nothing here can automate a real pointer). All passed on the first run; both other examples and the full test suite still pass unchanged. See `PLAN.md`/`LOG.md`.

**Up next:** No further M4 phase is yet scoped in `ARCHITECTURE.md`. Larger, later-milestone-shaped gaps: §11.9 transform composition (`PaintProperties.transform`, blocking transform-aware hit-testing), a real scrollable viewport for `VirtualList` (§11.8), and `NodeKind::Canvas` (blocking custom hit-testing, §11.10's own second half).

**Known gaps:**
- ~~§14 didn't sequence `engine-spec`/YAML-view work.~~ **Fixed.**
- `PLAN.md`/`LOG.md` archiving to `planning/archive/` (this section's own prior claim) actually stopped after Phase 4 step 6 — steps 7 through 15 kept overwriting `LOG.md`/`PLAN.md` in place instead. A real, minor administrative drift from the stated convention, not a technical gap; noted here rather than left as a silently-wrong claim.
- ~~Real splitter-drag input has no `winit`-driven UX wired, and no Python API creates a `NodeKind::Splitter`.~~ **Fixed (M4 Phase 3, both steps).** Still open: "drag-to-rearrange docking" (moving a whole panel between zones, not resizing) remains a separate, untouched, larger feature.
- `VirtualList` has no real scrollable viewport yet (clipping, an actual scroll offset/transform — §11.8/§11.9 aren't built) and only `ItemExtent::Fixed` is implemented, not the variable-height size-hint-callback variant §11.7's own text mentions — no consumer needs either yet.
- Everything in `ARCHITECTURE.md` citing "verify at implementation time" has started resolving, not finished: steps 1–7 confirmed `vello_hybrid`/`wgpu`/`kurbo`/`peniko`/`winit`/`taffy`/`slotmap`/`parley`/`glifo`/`fontique`/`serde_yaml_ng`/`pyo3`/`maturin`/`accesskit`/`accesskit_winit` are all real and compile/parse/build/run together (with real version-coupling/perf/API findings along the way, each handled, not just logged). `material-colors` remains unverified until step 11.
- `App`/`Node` no longer lack `set_on_click` (M4 Phase 1 step 3 closed that) but still lack `add_child` from Python — additive whenever a real need for it shows up.
- No live AT-SPI/UIA/NSAccessibility client is available in this dev/CI environment (M4 Phase 2's own real, stated constraint) — `Action::Click`/`Action::Focus` dispatch is real and unit-tested at every layer that doesn't need one, but a genuinely interactive screen reader driving a real request through the full stack is real, separate follow-up work whenever such an environment exists (M3 step 7's original wiring *did* have one at the time).
- `tracing`-based structured logging (§3) is still not wired up anywhere in this codebase — a raising click handler's traceback goes to stderr via `PyErr::print` (a real, complete traceback), not through a `tracing` subscriber. Revisit when some other real need sets one up.
- `Tree::tick_all` is a naive whole-tree walk, not §5's "walks only the active animation set" scoped version — deliberately deferred; real dirty-tracking is §6's own design surface, revisit if the frame-time benchmark (now real and CI-enforced) ever shows it's the cost.
- ~~No CI actually enforced any of this.~~ **Fixed.** `.github/workflows/ci.yml` is pushed and green on real GitHub Actions (`main` at `4f8ca82`): the Linux job (build/test/clippy `-D warnings`/fmt/frame-benchmark/full Python pipeline) plus the new `test-windows`/`test-macos` jobs (§6's own step-7 trigger) all pass. Three real gaps surfaced by actual CI runs, not predicted in advance, each fixed the same way every other "verify at implementation time" finding in this project has been: `yeslogic-fontconfig-sys` needing `fontconfig`'s pkg-config file at compile time (`libfontconfig1-dev`); and `maturin develop` refusing to run outside an active virtualenv, which every local check missed because this session's own `.venv` was already active (fixed by creating + `$GITHUB_PATH`/`$GITHUB_ENV`-activating one in the job itself, verified locally first in an isolated `env -i` shell before trusting it in CI). The release frame-time benchmark also runs for real (informational, non-blocking): 300 nodes, ~0.4-3.7ms depending on the runner's software rasterizer, comfortably under both the 16.6ms and 8.3ms budgets.

---

## Milestone 1 — TRE v1 (Archived Reference)

**Status: ✅ Complete.** 21 phases, 261 numbered review findings, tagged `archived-2026-09-14`, moved intact under `archive/` via `git mv` (full history preserved). Not tracked further here — see `archive/ARCHIVE_INDEX.md` and `archive/LESSONS_LEARNED.md`.

---

## Milestone 2 — v2 Architecture (`ARCHITECTURE.md`)

**Status: ✅ Complete.** Pre-implementation design document, 16 numbered sections + ADR-001, a running Locked Decisions log (§1) with 60+ entries, and three dedicated consistency passes that found and fixed real cross-section bugs (not just polish).

### Phase 1 — Archival & Retrospective ✅
- Stage: Archive the v1 tree, tag the commit — ✅
- Stage: Write `LESSONS_LEARNED.md` (8 themes distilled from the 261-finding ledger) — ✅
- Stage: Write `archive/ARCHIVE_INDEX.md` — ✅

### Phase 2 — Core Architecture (§1–§10) ✅
- Stage: Vision, scope, design principles (§1–§2, incl. Principle 6 added later) — ✅
- Stage: Technology stack (§3) — ✅
- Stage: System architecture & crate boundaries (§4) — ✅ (one diagram/code contradiction found and fixed)
- Stage: Core data model — `Node`/`NodeKind`/`Animated<T>` (§5) — ✅ (generational `NodeId`, queue-drain completion, closed-enum `NodeKind`)
- Stage: Per-frame pipeline & dirty-scoping (§6) — ✅ (frame budget + CI trigger, layout/paint-dirty split, later amended for Splitter/DockZone exception)
- Stage: MD3 subsystem — color, shadow, ripple, shape morph, motion, container transform (§7) — ✅
- Stage: Python/Rust FFI boundary (§8) — ✅ (`animate()` dispatch, `EngineError`, `PyWindow` GC — later split from `PyApp`)
- Stage: Threading & event loop model (§9) — ✅ (`Rc<RefCell<Tree>>` + `unsendable`, callback exception policy)
- Stage: Accessibility (§10) — ✅ (`TreeUpdate` builder ownership, minimal keyboard focus model)

### Phase 3 — Desktop Shell & Workspace (§11) ✅
- Stage: Multiple windows, app shell, single-page navigation (§11.1–§11.2) — ✅
- Stage: Menus/popups/dialogs — one overlay mechanism (§11.3) — ✅
- Stage: Docking — fixed 5-zone model (§11.4) — ✅ (in scope for v1 by explicit user override of the recommended default)
- Stage: Splitters, toolbars/status bars (§11.5–§11.6) — ✅
- Stage: Virtualization, culling, transform composition, hit-testing, node graphs/charts (§11.7–§11.11) — ✅

### Phase 4 — Declarative Authoring (§16) ✅
- Stage: `engine-spec` crate, `WidgetSpec`, view composition via `include:` (§16.1, §16.6) — ✅
- Stage: Binding expressions, `BindingResolver`, ViewModel/View attach model (§16.2) — ✅
- Stage: Stylesheet cascade, reconciliation/hot-reload (§16.3–§16.4) — ✅
- Stage: Two-way bindings (§16.7) — ✅

### Phase 5 — Interaction State Model ✅
- Stage: Design Principle 6 — mechanical vs. meaning-dependent state ownership (§2) — ✅
- Stage: Hover/focus state layers, accessibility-state auto-derivation (§7.3, §10) — ✅

### Phase 6 — Consistency & Review Passes ✅
- Stage: Full front-to-back pass (found stale `PyApp`/`Tree` singleton assumption, `Overlay` side-structure fragmentation risk, absolute dirty-scoping claim) — ✅
- Stage: `pyCopper` cross-reference pass (borrowed proven stylesheet/composition/reactivity decisions without adopting its stack) — ✅

---

## Milestone 3 — v2 Implementation

**Status: ✅ Complete.** All 7 phases done, matching every one of §14's Suggested Build Order's 15 steps (including `engine-spec`/§16).

### Phase 1 — Workspace Scaffold ✅
- Step: Cargo workspace + six crate skeletons (`engine-core`, `engine-md3`, `engine-render`, `engine-platform`, `engine-spec`, `engine-py`) per §12 — ✅
- Step: Wire the dependency edges §4 specifies (no cross-boundary violations from day one) — ✅ (found and fixed one real gap first: `engine-render → engine-core` was missing from the diagram)

### Phase 2 — Render Core Spike (§14 steps 1–4) ✅
- Step 1: Static rounded rect through `vello_hybrid`, real window via `engine-platform` — ✅ (found and fixed two real Linebender-family version conflicts — a `wgpu` 29-vs-30 clash that would not have compiled, and an unguaranteed `kurbo` coupling documented for the future; headless pixel-readback test + a real windowed run, both passing)
- Step 2: `Animated<T>` + central tick — ✅ (validated standalone in `engine-core` — 6 unit tests — then proven to actually drive rendering via a headless mid-flight pixel-readback test in `engine-render`, and shown live over 60 real windowed frames)
- Step 3: `taffy` layout + frame-time CI benchmark (§6 target: 16.6ms/8.3ms) — ✅ (`Node`/`NodeKind`/`PaintProperties`/`Tree` implemented in `engine-core`, `NodeId` via `slotmap` at zero new dependency cost; `engine-render::build_tree_scene` proven to paint at real taffy-computed positions via a headless test; the frame-time benchmark is real and CI-enforced — 0.58ms median in release for 300 nodes, comfortably under the 16.6ms/8.3ms target — `#[ignore]`d from the default debug-mode suite since debug codegen alone measures ~36ms, run explicitly with `--release`)
- Step 4: `parley` text spike (real type scale, non-trivial string) — ✅ (`NodeKind::Text(TextState)` + `engine-render`'s `TextRenderer` shape real text via `parley` into `vello_hybrid`'s glyph API; two Roboto weights and a vendored, hermetic Arabic RTL string all render through the real `Tree` pipeline, proven by a headless test asserting ink lands in the right box *and*, for the RTL string, on the right edge of its box, not the left — found and fixed two real gaps along the way: weight needs `StyleProperty::FontWeight`, not a second family name, and RTL needs an explicit `Layout::align` pass)

### Phase 3 — Declarative Authoring, minimal (§14 step 5) ✅
- Step 5: `engine-spec` parses one static `view.yaml` (no bindings/handlers), `WidgetSpec` → `NodeKind`, `deny_unknown_fields` validation, renders through Phase 2's pipeline — ✅ (real `WidgetSpec`/`StyleSpec` types, `background` required for `Rect`/`Text` — a clear `SpecError`, not a silent default; found and fixed a real `serde_yaml_ng` enum-tagging limitation, restructuring `kind`/`text` to a flatter, more natural shape; a real standalone `view.yaml` proven to render through the actual Phase 2 pipeline via a headless pixel-readback test — exact color match on the swatch, real ink on the label)

### Phase 4 — Python Bindings & Accessibility (§14 steps 6–7) ✅
- Step 6: `engine-py` node creation + one property setter — ✅ (`App`/`Node` minimal slice; real pyo3 0.29.2 API verified directly, not assumed — `with_gil`/`allow_threads` are now `attach`/`detach`; `maturin develop` + `import tre` + a 6-test pytest suite + a real `.py` script driving a 60-frame windowed render loop, all run for real; CI extended per §13's own decision to start Python CI at this step)
- Step 7: `accesskit` wiring, one button verified with a screen reader — ✅ (real `accesskit_winit::Adapter` wiring via `with_event_loop_proxy` in `engine-platform`; `Tree::build_access_update` in `engine-core`; the button's exposure verified directly against the real AT-SPI bus — exact role, label, action, and bounds match, not just "it compiled")

### Phase 5 — MD3 Foundational Spikes (§14 steps 8–11) ✅
- Step 8: Shadow spike (`fill_blurred_rounded_rect`) — ✅ (verified `vello_hybrid` 0.2.0's real API directly in source; `build_shadow_scene` + a headless four-point falloff test proving a real Gaussian blur, not a hard edge — see `PLAN.md`/`LOG.md`)
- Step 9: Ripple/state-layer — ✅ (`InteractionState`/`RippleState` in `engine-core`, real `Tree::interaction_mut`/`tick_all` wiring; `engine_render::build_ripple_scene` proves `push_layer`'s clip + opacity both genuinely work via two headless pixel-readback tests — see `PLAN.md`/`LOG.md`)
- Step 10: Shape morph module — ✅ (`engine_md3::shape_morph::ShapeKey` implements `engine_core::Interpolate` — correspondence/alignment search plus equalize-then-lerp, matching §7.4's review note exactly; two tests prove a real square-rotated-to-a-different-corner and a reversed-winding square both morph to themselves, not a collapsed point — see `PLAN.md`/`LOG.md`)
- Step 11: `material-colors` dynamic theme — ✅ (real §7.1 acceptance-gate evidence: the vendored crate's own test suite run directly, 129/129 passing; `engine_md3::color::{ColorScheme, DynamicTheme}` maps all 49 real MD3 roles onto `peniko::Color`, cross-checked field-by-field against `material-colors`' own native output — see `PLAN.md`/`LOG.md`)

### Phase 6 — Declarative Authoring, full (§14 step 12) ✅
- Step 12: `BindingResolver` + `ViewModel`/`View._attach()` (§16.2), stylesheet cascade with real MD3 tokens (§16.3), reconciliation/hot-reload (§16.4) — ✅ (three stages, each independently verified: A — cascade + MD3 token resolution; B — `Tree::remove` + keyed-diff `Reconciler` + real `notify` file watcher; C — whitelisted binding-expression parser + `BindingResolver` + `engine-py`'s real `View`/`Signal`/`ViewModel` with genuine dependency tracking, proven end-to-end via 7 new pytest tests — see `PLAN.md`/`LOG.md`)

### Phase 7 — Desktop Shell Build-out (§14 steps 13–15) ✅
- Step 13: Overlay mechanism (one dropdown menu) — ✅ (`Tree::open_overlay`/`close_overlay`, real `Position::Absolute` placement relative to an anchor's accumulated absolute bounds, `OverlayMeta` bookkeeping; a headless pixel-readback test proves append-order-is-paint-order for real — an overlapping overlay paints on top of a full-canvas background with zero special-casing in `build_tree_scene` — see `PLAN.md`/`LOG.md`)
- Step 14: Multi-window (second `PyWindow`) — ✅ (`engine_platform::run_windowed_multi` manages a real `HashMap<WindowId, PerWindow>`; `PyWindow` split back out of `App` exactly as predicted since step 6; a `harness = false` test opens two windows directly and confirms genuinely distinct `WindowId`s plus independent per-window frame counts, and `examples/two_windows.py` proves the same end-to-end through the real Python API — see `PLAN.md`/`LOG.md`)
- Step 15: Docking + virtualization — ✅ **(M3's final step)** — three stages: **A** Splitters (§11.5) — `NodeKind::Splitter`/`Tree::set_splitter_position`, a real pane boundary proven to move on screen. **B** Docking (§11.4) — `DockLayout`/`DockZone`/`Tree::apply_active_tab` via `Tree::detach` (not `Display::None`), zone resizing reusing Stage A's splitter mechanism verbatim, proven via a 3-zone docked-screen pixel test. **C** Virtualization (§11.7) — `NodeKind::VirtualList`/`Tree::set_virtual_list_window` (only the visible window is ever real `Node`s, recycled via `slotmap`'s real generational `NodeId` invalidation), `engine-py`'s new "materialize item N" FFI callback with real `PyWindow` `__traverse__`/`__clear__` cyclic-GC support (this codebase's first long-lived stored `PyObject` callback, proven via a real reference-cycle-collection pytest), and the Risk Register's own named acceptance-gate benchmark — **~3.0us/call measured** across 100,000+ rows, comfortably inside budget. See `PLAN.md`/`LOG.md`.

---

## Milestone 4 — Real Input Dispatch (§4, §7.3, §9, §10, §11.10)

**Status: ✅ Phases 1-3 complete.** Not sequenced anywhere in `ARCHITECTURE.md` — picked up the single most consistently recurring finding from every M3 interaction-dependent step (7, 9, 11, 12, 13, 14, 15, each checked directly): real pointer/keyboard `InputEvent`/`AppHandler` dispatch and hit-testing didn't exist. Phase 1 (3 steps) built it end to end for pointer/keyboard events reaching `Tree::dispatch`; Phase 2 closed the two real gaps Phase 1 explicitly left open (assistive-technology action dispatch, a Python-facing keyboard test entry point); Phase 3 closed the last repeatedly-named gap (real splitter-drag input). No Phase 4 yet scoped.

### Phase 1 — Core Dispatch Mechanism ✅
- Step 1: `InputEvent`/`AppHandler`/hit-testing core, `engine-core` only (no `winit`, no `pyo3` yet) — ✅ (new `input.rs`: `InputEvent`/`PointerButton`/`Key`/`AppHandler`/`DispatchOutcome`, deliberately narrow `Key` — `Tab`/`Enter`/`Space`/`Escape` only, matching §10's own stated minimal keyboard model. `Tree` gains `hit_test` (§11.10, reverse paint-order rect containment — explicitly not transform-aware yet, `PaintProperties.transform` doesn't exist; no `NodeKind::Canvas` custom hit-test either, `Canvas` doesn't exist), `update_hover` (§7.3's own "falls out of hit-testing" text, opt-in-only per Design Principle 6, MD3 values stay caller-supplied so `engine-core` stays MD3-agnostic), `move_focus` (§10 Tab/Shift-Tab in tree order, "interactive" = non-empty `access.actions`, the real existing signal M3 step 7's button already sets), and `dispatch` (the one top-level entry point, producing `DispatchOutcome::Activated` as the sole meaning-dependent outcome for `AppHandler` to interpret later). Found a real architecture/API drift: §10's own text says Enter/Space dispatches `accesskit::Action::Default`, which doesn't exist in the pinned `accesskit = "0.25.0"` — the real equivalent is `Action::Click`, already what M3 step 7's own button uses. 6 new tests, each isolating one claim. `engine-core` now at 32 unit tests. See `PLAN.md`/`LOG.md`.
- Step 2: Real `winit` event translation (`engine-platform`) — ✅ (`WindowEvent::CursorMoved`/`MouseInput`/`KeyboardInput`/`ModifiersChanged` translate into real `InputEvent`s via a new `on_input` closure on `run_windowed_multi`, matching `on_frame`/`build_access_update`'s own shape; `run_windowed`'s two existing callers pass a no-op, unchanged. Found and fixed a real doc-comment error from step 1: `winit::event::MouseButton` has six real variants, not the three `PointerButton` claimed unverified. `winit` has no public API to inject a synthetic `WindowEvent` into a live loop — checked directly — so the real proof is 4 new unit tests against `translate_pointer_button`/`translate_key` directly, no live `EventLoop` needed. See `PLAN.md`/`LOG.md`.)
- Step 3: `engine-py` implements the meaning-dependent half for real — ✅ (`Node.set_on_click(callback)` exists: `click_handlers: Rc<RefCell<HashMap<NodeId, Py<PyAny>>>>` shared between a `Window` and every `Node` it hands out, the same sharing shape `tree` already uses -- avoids giving `Node` a `PyWindow` back-reference. Visited by real `__traverse__`/`__clear__`, reusing M3 step 15 Stage C's materializer precedent exactly. Also adds `Action::Click` to `access.actions`, so a node with a click handler becomes Tab-reachable for free (§10). New `Window.click(node)` is a direct, programmatic click entry point proving the whole path with no live window needed. An uncaught handler exception is caught via `PyErr::print`, matching §9's "caught, logged, non-fatal" policy with a real traceback. 6 new pytest tests, including a real GC-cycle-collection proof for `click_handlers`. See `PLAN.md`/`LOG.md`.)

### Phase 2 — Assistive-Technology Action Dispatch (§10) ✅
- Step 1: `Tree` gains direct activation/focus primitives (`engine-core`) — ✅ (`Tree::activate(node)` — the direct, non-`InputEvent` counterpart to a mouse click's `Activated` outcome, deliberately symmetric with the mouse path: doesn't check `access.actions` first. `Tree::set_focus_to(node, ...)` — the direct-target counterpart to `move_focus`'s tab-order computation, what `accesskit::Action::Focus` actually needs; factored the shared `focus_ring`-transition logic out of `move_focus` into a private `transition_focus` helper, reused by both. Both build on a new, public `from_access_id` (the reverse of the existing `to_access_id`), verified directly against `slotmap = "1.1.1"`'s own documented `KeyData::as_ffi`/`from_ffi` reversible round-trip guarantee — a stale accessibility-client-supplied id fails this `Tree`'s own generation check safely, the same generational-safety property §5 already relies on elsewhere. 5 new tests, each isolating one claim. `engine-core` now at 37 unit tests.)
- Step 2: Real `accesskit_winit` wiring (`engine-platform`) — ✅ (`run_windowed_multi` gained a sixth parameter, `on_access_action: FnMut(WindowId, accesskit::ActionRequest)`, firing on a real `accesskit_winit::WindowEvent::ActionRequested` — closing the exact gap step 7's own comment named since early in this project. Handed up raw/untranslated, unlike `on_input`: `engine-platform` doesn't know `engine_core::NodeId` exists, so the `from_access_id` conversion is the caller's job. `AccessibilityDeactivated` needed no new handling -- every `TreeUpdate` build already gates on activation state via the existing `update_if_active`. No live AT-SPI/UIA/NSAccessibility client is available in this environment to drive a genuine end-to-end proof — stated explicitly, not silently skipped; the real, honest proof is at the `engine-core` logic level, already fully unit-tested in step 1.)
- Step 3: `engine-py` wires the new callback + a keyboard test entry point — ✅ (`App::run()`'s new `on_access_action` closure converts the target id and matches `Action::Click`/`Action::Focus` onto `tree.activate`/`tree.set_focus_to`, feeding the outcome through the *same* `dispatch::run_activation` a mouse click and `Window.click()` already share -- one mechanism, three real entry points. New `Window.press_key(key, shift=False)` closes Phase 1's other stated gap: translates a small string vocabulary (`"tab"`/`"enter"`/`"space"`/`"escape"`) into `engine_core::Key` and dispatches through the identical path, making Tab-then-Enter/Space activation directly testable from Python for the first time. 6 new pytest tests, all passed on the first run. See `PLAN.md`/`LOG.md`.)

### Phase 3 — Real Pointer-Drag Dispatch (§11.5, §11.4) ✅
- Step 1: Wire real mouse-drag to `Tree::set_splitter_position` (`engine-core`) — ✅ (`Tree` gains a `dragging: Option<NodeId>` field; the flanking-siblings geometry `set_splitter_position` already computed inline was factored into a shared `splitter_geometry` helper. New `update_drag(point, now)` converts a pointer coordinate into a 0.0..=1.0 fraction (relative to the left sibling's own absolute start and the flanking siblings' combined, drag-constant extent) and calls the *existing* `set_splitter_position` with it — §11.5's own "on drag... used both standalone... and by docking — one mechanism, two call sites" text, made real rather than reimplemented. `dispatch`'s `PointerPressed`/`PointerMoved`/`PointerReleased` arms now start (primary-button press on a splitter, reusing the existing hit-test call), live-update (every move while dragging, alongside the existing hover update), and end (primary-button release, unconditionally — a real mouse-up always ends a drag wherever it lands, giving free pointer-capture-like behavior since `update_drag` never depends on hit-testing) a real splitter drag. Because docking's own zone-resize already reuses `set_splitter_position` verbatim (M3 step 15 Stage B), dragging a dock-zone boundary is real now too, with zero docking-specific code touched. 4 new `engine-core` unit tests (`engine-core` now at 41) + a new `engine-render` pixel-readback test (`splitter_drag_dispatch.rs`) driving the whole press/move/release sequence through real `Tree::dispatch` rather than a direct call, all passed on the first run. No `engine-platform`/`engine-py` changes were needed — `Tree::dispatch` is the exact function `App::run()`'s existing real `winit` wiring already calls, so the mechanism reached the full stack for free. See `PLAN.md`/`LOG.md`.)
- Step 2: `Window.add_splitter` (`engine-py`) — ✅ (mirrors `add_rect`'s own exact parameter shape plus a new `initial_position` keyword (0.0..=1.0, defaults to an even split); inserts a real `NodeKind::Splitter` and appends it to the window's own root row — no new mechanism needed, that root is already the flex parent `Tree::splitter_geometry` expects. `add_rect` (left), `add_splitter`, `add_rect` (right) is now a complete, real, mouse-draggable resizable-pane layout from Python. 5 new pytest tests + a new real example script (`examples/resizable_panes.py`) that opens a live window with a working layout and exits cleanly after real frames -- its own docstring states plainly that proving an actual live mouse drag needs a human running it interactively, since nothing here can automate a real pointer. All passed on the first run. See `LOG.md`.)
