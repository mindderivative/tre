# Build Tracker

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md` and `archive/`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — TRE v1 (archived reference) | `██████████` 100% | ✅ Archived `archived-2026-09-14` |
| M2 — v2 Architecture (`ARCHITECTURE.md`) | `██████████` 100% | ✅ 16 sections + ADR-001, locked |
| M3 — v2 Implementation | `██⬜⬜⬜⬜⬜⬜⬜⬜` 14% | 🚧 In progress — Phase 1 of 7 complete |

**Just closed:** M3 Phase 1 (Workspace Scaffold) — the Cargo workspace, `rust-toolchain.toml`, and all six crate skeletons, wired with exactly §4's local dependency edges (plus one real gap `PLAN.md` found and fixed first: `engine-render → engine-core` was missing from the diagram entirely). `cargo check --workspace` and `cargo clippy --workspace` both clean. First real code in the v2 rebuild. See `PLAN.md`/`LOG.md`.

**Up next:** M3 Phase 2 — Render Core Spike (§14 steps 1–4): a static rounded rect through `vello_hybrid` into a real window, then `Animated<T>`, `taffy` layout, and a `parley` text spike. First contact with the actual pinned dependency versions.

**Known gaps:**
- ~~§14 didn't sequence `engine-spec`/YAML-view work.~~ **Fixed.**
- ~~No `PLAN.md`/`LOG.md` existed yet for M3.~~ **Fixed** — both now exist, one per phase/step going forward, matching TRE v1's own convention.
- Everything in `ARCHITECTURE.md` citing "verify at implementation time" (accesskit's real current API, Vello/Linebender family version compatibility, `material-colors`' test-vector conformance) is still unverified — resolves incrementally, crate by crate, as each build-order step actually pins its dependency. Phase 1 touched zero external crates, so this hasn't started yet; Phase 2 is where it begins.

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

**Status: 🚧 In progress.** Phase 1 complete; this mirrors §14's Suggested Build Order (all 15 steps now sequenced, including `engine-spec`/§16).

### Phase 1 — Workspace Scaffold ✅
- Step: Cargo workspace + six crate skeletons (`engine-core`, `engine-md3`, `engine-render`, `engine-platform`, `engine-spec`, `engine-py`) per §12 — ✅
- Step: Wire the dependency edges §4 specifies (no cross-boundary violations from day one) — ✅ (found and fixed one real gap first: `engine-render → engine-core` was missing from the diagram)

### Phase 2 — Render Core Spike (§14 steps 1–4) ⬜
- Step 1: Static rounded rect through `vello_hybrid`, real window via `engine-platform` — ⬜
- Step 2: `Animated<T>` + central tick — ⬜
- Step 3: `taffy` layout + frame-time CI benchmark (§6 target: 16.6ms/8.3ms) — ⬜
- Step 4: `parley` text spike (real type scale, non-trivial string) — ⬜

### Phase 3 — Declarative Authoring, minimal (§14 step 5) ⬜
- Step 5: `engine-spec` parses one static `view.yaml` (no bindings/handlers), `WidgetSpec` → `NodeKind`, `deny_unknown_fields` validation, renders through Phase 2's pipeline — ⬜

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
