# Build Tracker — `tre` 0.5

Updated after every milestone/phase/stage/step completion, kept in sync with `ARCHITECTURE.md`. Status legend: ✅ done · 🚧 in progress · ⬜ not started.

**Artifact:** generate with `python3 tools/generate_tracker_artifact.py --project "tre 0.5" --out tools/build-tracker-0.5.0.generated.html` (after committing the tracker, so the stamp names the commit) and publish to the 0.5.0 tracker page. The [0.4 line's page](https://claude.ai/artifact/PTt1sBpABbM2XxTrL7xdFb) stays the record of `v0.4.0`–`v0.4.4`.

**This tracker is the `0.5.0` line** -- custom windowing ([issue #28](https://github.com/mindderivative/tre/issues/28)): undecorated windows whose title bar and borders the framework draws, with `tre` doing the dragging, resizing, and window controls -- and its milestones restart at 1. Earlier history is archived: `v0.4.0`–`v0.4.4` (M1–M23) in [`BUILD_TRACKER_ARCHIVE_0.4.md`](BUILD_TRACKER_ARCHIVE_0.4.md), `v0.1.0`–`v0.3.5.1` in [`BUILD_TRACKER_ARCHIVE_0.3.md`](BUILD_TRACKER_ARCHIVE_0.3.md), and M1–M49 of that line in [`BUILD_TRACKER_ARCHIVE_M1-M50.md`](BUILD_TRACKER_ARCHIVE_M1-M50.md). Fixes to the released `0.4.4` ship as `0.4.x` patches from `main`, recorded in the 0.4 archive; fold any into it when 0.5.0 merges.

---

## Top Metrics

| Milestone | Progress | Status |
|---|---|---|
| M1 — Design: Custom Windowing ([issue #28](https://github.com/mindderivative/tre/issues/28)) | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M2 — Undecorated Windows and Window Controls | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M3 — Drag and Resize Regions | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M4 — macOS: Content Under a Transparent Title Bar | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M5 — Example, Docs, and Manual Checks | `░░░░░░░░░░` 0% | ⬜ Proposed |
| M6 — Release `0.5.0` | `░░░░░░░░░░` 0% | ⬜ Proposed |

**Just closed:** `v0.4.4` (0.4 line M23, 2026-09-30) -- scroll chaining for the wheel and keys, and a CI cache trimmed to the dependencies; see [`BUILD_TRACKER_ARCHIVE_0.4.md`](BUILD_TRACKER_ARCHIVE_0.4.md).

**Up next:** M1 Step 1 -- the custom-windowing design page, for the user's and Tesserae's review.

**Known gaps:**
- None open on this line.

**Fixed gaps:**
- None yet on this line.

---

## Milestone 1 — Design: Custom Windowing ([issue #28](https://github.com/mindderivative/tre/issues/28))

**Status: ⬜ Proposed.** User (2026-09-30): "yes, scope #28 for 0.5.0". Issue #28 (the user's): `tre` should let a window drop its native title bar and borders so the framework draws its own -- title, icon, minimize/maximize/close, and extras like a search field -- while `tre` turns decorations off, handles the input, and does the dragging and resizing, the framework telling it which areas are the title bar and which are the borders. Found while scoping, in `winit` 0.30.13's source (2026-09-30): `with_decorations(false)` / `set_decorations` work everywhere; `drag_window()` works on X11, Wayland, Windows, and macOS but must follow a primary press at once -- so `tre` should start it on the press itself, from a region the framework marked, not from a later callback; `drag_resize_window()` is unsupported on macOS, so an undecorated macOS window couldn't be resized -- the native macOS answer is a transparent title bar over full-size content (`with_titlebar_transparent`, `with_fullsize_content_view`), keeping the traffic lights, resizing, and rounded corners; Windows drops an undecorated window's shadow unless `with_undecorated_shadow` is set, and has a native title-bar menu (`show_window_menu`); maximize, minimize, and restore are there with queries, and window focus arrives as an event, which a title bar needs for its inactive look.

### Phase 1 — Design ⬜
- Step 1: a design page, `docs/design/custom-windowing.md`: the split between `tre` and the framework; how the framework marks drag and resize regions (a node property, hit-tested on a primary press); window methods (`minimize`, `maximize`, `restore`, `close`) and the state and events a title bar needs (maximized, minimized, active); each platform's limits and `tre`'s answer to them; and the open questions -- double-click to maximize, the Windows window menu, macOS's transparent title bar -- for the user and for Tesserae — ⬜
- Step 2: the user's decisions, with Tesserae's review, recorded; M2–M5 revised to match — ⬜

---

## Milestone 2 — Undecorated Windows and Window Controls

**Status: ⬜ Proposed.** Subject to M1's design.

### Phase 1 — Decorations and Controls ⬜
- Step 1: `Window(decorations=False)` and a live `window.set(decorations=...)`; Windows keeps its shadow — ⬜
- Step 2: `window.minimize()`, `maximize()`, `restore()`, and `close()`; `window.get("maximized")`, `"minimized"`, and `"active"`, and window events when they change — ⬜
- Step 3: tests and docs — ⬜

---

## Milestone 3 — Drag and Resize Regions

**Status: ⬜ Proposed.** Subject to M1's design.

### Phase 1 — Regions ⬜
- Step 1: a node property naming a window region -- the title bar, or an edge or corner -- hit-tested on a primary press, starting `winit`'s drag or resize at once — ⬜
- Step 2: resize cursors over edge regions; double-click and the Windows window menu, if the design keeps them — ⬜
- Step 3: tests and docs — ⬜

---

## Milestone 4 — macOS: Content Under a Transparent Title Bar

**Status: ⬜ Proposed.** Subject to M1's design: `winit` can't resize an undecorated macOS window.

### Phase 1 — macOS ⬜
- Step 1: a transparent native title bar over full-size content on macOS, keeping the traffic lights and resizing, with its height reported so the framework lays out under it — ⬜

---

## Milestone 5 — Example, Docs, and Manual Checks

**Status: ⬜ Proposed.**

### Phase 1 — Example and Checks ⬜
- Step 1: `examples/custom_titlebar.py`, a guide page, and an ARCHITECTURE.md section — ⬜
- Step 2: the user's manual checks on KDE Wayland and X11; macOS and Windows by CI only — ⬜

---

## Milestone 6 — Release `0.5.0`

**Status: ⬜ Proposed.**

### Phase 1 — Release ⬜
- Step 1: PR to `main`, CI green, merge, tag `v0.5.0`, release, PyPI — ⬜
- Step 2: Tesserae moves to `0.5.0` — ⬜

---

## Branch: `0.5.0` — Scaffold

**Status: ✅ Scaffolded (2026-09-30).**

- Branch `0.5.0` created off `main` at `a6f1853` (`v0.4.4` plus its release record) — ✅
- `Cargo.toml` and `pyproject.toml` bumped to `0.5.0`; `Cargo.lock` updated via `cargo metadata` — ✅
- The 0.4.x tracker moved to `BUILD_TRACKER_ARCHIVE_0.4.md`; this tracker started at M1 — ✅
