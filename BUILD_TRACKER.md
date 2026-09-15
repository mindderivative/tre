# Build Tracker

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md` and `archive/`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — TRE v1 (archived reference) | `██████████` 100% | ✅ Archived `archived-2026-09-14` |
| M2 — v2 Architecture (`ARCHITECTURE.md`) | `██████████` 100% | ✅ 16 sections + ADR-001, locked |
| M3 — v2 Implementation | `████⬜⬜⬜⬜⬜⬜` 43% | 🚧 In progress — Phases 1-3 of 7 complete, Phase 4 next |

**Just closed:** M3 Phase 3 (§14 step 5) — `engine-spec` minimal: `WidgetSpec`/`NodeKindSpec`/`StyleSpec` parse a real, standalone `view.yaml` (no `bindings:`/`handlers:` yet) with `deny_unknown_fields` validation, and build into a real `engine_core::Tree`, rendered through Phase 2's pipeline unchanged. A real finding surfaced and was fixed: `serde_yaml_ng` doesn't support serde's classic single-key-map external tagging for data-carrying enum variants (`kind: {Text: {...}}` failed — its `deserialize_enum` only accepts a bare scalar or YAML's native `!Tag` syntax) — restructured to a flatter, more YAML-natural shape (`kind: Text` + a sibling `text:` block) rather than asking view authors to write obscure `!Text` tags. Proven end-to-end by a headless `engine-render` integration test: a parsed swatch's rendered pixel matches its declared `#6750A4` exactly, and its label draws real ink. See `PLAN.md`/`LOG.md`.

**Up next:** M3 Phase 4 (§14 steps 6-7) — wire `engine-py`: expose node creation + one property setter to Python, driving step 2's animation from a real `.py` script; then wire `accesskit`, confirming one button is correctly exposed to a screen reader. First step to touch `pyo3` at all.

**Known gaps:**
- ~~§14 didn't sequence `engine-spec`/YAML-view work.~~ **Fixed.**
- ~~No `PLAN.md`/`LOG.md` existed yet for M3.~~ **Fixed** — both now exist, one per phase/step, archived to `planning/archive/` on completion, matching TRE v1's own convention exactly (verified directly against `archive/crates/tre-rhi-vulkan`, not assumed).
- Everything in `ARCHITECTURE.md` citing "verify at implementation time" has started resolving, not finished: steps 1–5 confirmed `vello_hybrid`/`wgpu`/`kurbo`/`peniko`/`winit`/`taffy`/`slotmap`/`parley`/`glifo`/`fontique`/`serde_yaml_ng` are all real and compile/parse together (with real version-coupling/perf/API findings along the way, each handled, not just logged). `pyo3` and `accesskit` remain unverified until their own build-order steps land; `material-colors` until step 11.
- `Tree::tick_all` is a naive whole-tree walk, not §5's "walks only the active animation set" scoped version — deliberately deferred; real dirty-tracking is §6's own design surface, revisit if the frame-time benchmark (now real and CI-enforced) ever shows it's the cost.
- ~~No CI actually enforced any of this.~~ **Fixed.** `.github/workflows/ci.yml` is pushed and has run for real on GitHub Actions (`main` at `e3deff2`): build/test/clippy `-D warnings`/fmt all green on a bare Ubuntu runner. One real gap surfaced and was fixed by the run itself, not predicted in advance: `yeslogic-fontconfig-sys` (pulled in by `parley`'s `fontique` backend) needs `fontconfig`'s pkg-config file at *compile* time regardless of `TextRenderer`'s runtime `system_fonts: false` choice — this dev machine already had `fontconfig-dev` installed, the bare runner didn't; fixed by adding `libfontconfig1-dev` to the workflow's apt install list. The release frame-time benchmark also ran for real (informational, non-blocking): 300 nodes, median 3.663ms on lavapipe's software rasterizer — slower than this machine's real-GPU 0.4ms as expected, still comfortably under both the 16.6ms and 8.3ms budgets.

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

**Status: 🚧 In progress.** Phases 1–3 complete, Phase 4 next; this mirrors §14's Suggested Build Order (all 15 steps now sequenced, including `engine-spec`/§16).

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

### Phase 4 — Python Bindings & Accessibility (§14 steps 6–7) ⬜
- Step 6: `engine-py` node creation + one property setter — ⬜
- Step 7: `accesskit` wiring, one button verified with a screen reader — ⬜

### Phase 5 — MD3 Foundational Spikes (§14 steps 8–11) ⬜
- Step 8: Shadow spike (`fill_blurred_rounded_rect`) — ⬜
- Step 9: Ripple/state-layer — ⬜
- Step 10: Shape morph module — ⬜
- Step 11: `material-colors` dynamic theme — ⬜

### Phase 6 — Declarative Authoring, full (§14 step 12) ⬜
- Step 12: `BindingResolver` + `ViewModel`/`View._attach()` (§16.2), stylesheet cascade with real MD3 tokens (§16.3, now that Phase 5 gives it a real color scheme), reconciliation/hot-reload (§16.4) — ⬜

### Phase 7 — Desktop Shell Build-out (§14 steps 13–15) ⬜
- Step 13: Overlay mechanism (one dropdown menu) — ⬜
- Step 14: Multi-window (second `PyWindow`) — ⬜
- Step 15: Docking + virtualization — ⬜
