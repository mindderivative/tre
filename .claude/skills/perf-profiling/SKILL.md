---
name: perf-profiling
description: Use when measuring, comparing or optimising tre's performance (frame time, GPU or CPU cost, idle wake-ups, memory or VRAM), when a change might make rendering slower, or as the measuring step inside research mode. Gives the existing benchmarks, how to run them fairly, and how to report a before/after.
---

# Performance profiling

tre has no separate benchmark crate. Its measurements are `#[ignore]`d tests that print tables, plus a Python-side counter. Use those, so a new number can be compared with an old one. `research-mode` says *when* to measure; this says *how*.

## The tools that exist

| What | Where | Run |
|---|---|---|
| Frame cost by stage (update, tick, layout, prepare, draw, gpu) across scene kinds, 1920x1080 | `crates/engine-render/tests/baseline.rs` | `cargo test -p engine-render --test baseline --release -- --ignored --nocapture --test-threads=1` (`BASELINE_FILTER=text` narrows it) |
| What partial redraw saves (small, full, idle workloads) | `tests/partial_redraw_bench.rs` | `cargo test -p engine-render --test partial_redraw_bench --release -- --ignored --nocapture` |
| Scene, damage and frame budget costs | `scene_cost.rs`, `damage_cost.rs`, `frame_budget.rs` | `cargo test -p engine-render --test <name> --release -- --nocapture` (check each file for `#[ignore]`) |
| A running window's frames: stages (`tick`, `layout`, `prepare`, `acquire`, `draw`, `present`), `fps`, `cpu_ms`, `gpu_ms`, and redraws that were `nothing`, `full` or `partial` | `window.frame_stats()` (`recent`, `last`); `reset=True` clears the history; `stats_handle()` reads it from another thread | `docs/reference/window.md`, "frame_stats" |
| Video memory: `allocated_bytes`, `reserved_bytes`, `blocks`, `largest` | `frame_stats()["memory"]` (#222); `tools/vram_probe.py` | `docs/design/vram-per-window.md` |
| Which node kinds and nodes cost the most to build | `window.set(profile_nodes=True)`, then `frame_stats()["profile"]` | |
| A timeline of every frame, with GPU time on its own track | `window.start_trace(path)` ... `stop_trace()`, opened at ui.perfetto.dev | |
| Idle CPU and wake-ups on a real window | `/proc` sampling, method in `docs/design/performance-baseline.md` | |

Switches: `TRE_DAMAGE_FULL=1` turns the damage shortcut off (to compare timings); `TRE_GPU_MEMORY=performance` brings back the old, larger GPU memory blocks (a setting to compare against, not a measurement: the default is small blocks, #219); `TRE_DAMAGE_VERIFY=1` makes it check itself and is **slow, never use it when timing**; `TRE_TEST_FORCE_FALLBACK=1` selects the software adapter.

## Method

1. **Release builds only.** `--release`, never debug (`[profile.dev]` is also trimmed for size, not speed).
2. **Fix the conditions.** Same machine, same adapter, nothing else heavy running, `--test-threads=1`. Record the adapter name the harness prints, the CPU, the compositor, and the commit. If power management is on, say so.
3. **Baseline before the change.** Run the relevant benchmark at the parent commit (a worktree is cleanest) and save the output to a file outside the repo.
4. **Repeat.** One run is an anecdote. Run at least three times and report the spread as well as the typical figure. The benchmarks already warm up (8 frames) before timing (60).
5. **Change one thing, measure again** with the same command.
6. **Split the cost.** Say which stage moved (prepare, draw, gpu), not only the total. A CPU-side win can be invisible when the GPU is the bottleneck, and the reverse.
7. **Both adapters when you can.** Software rendering (lavapipe) and a real GPU have different bottlenecks. A gain on one is not a gain on the other until measured.
8. **Check output did not change.** Run the pixel tests. Faster but different is a feature change, not an optimisation (see `research-mode`'s "do not").

Read frame times with the display in mind: with vsync on (the default) `acquire_ms` is the loop waiting for the next refresh, so `total_ms` sits near the refresh interval even when little work is done. Judge the app's own cost by `cpu_ms`.

## When there is no number for it yet

Write a new `#[ignore]`d test beside the others, in the same style (header comment saying what it measures and how to run it, a table on stdout), rather than a one-off script that is lost. Add it to `docs/design/performance-baseline.md` if it is meant to be rerun.

## Report

For each claim: the command, the machine and adapter, before and after with the spread, which stage moved, and what was not measured (Mac, Windows, low-end hardware and other GPU vendors usually are not; issue #118 holds that gap). Do not say "faster" without a pair of numbers. A change inside the run-to-run spread is "no measurable change".
