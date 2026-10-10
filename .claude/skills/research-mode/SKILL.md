---
name: research-mode
description: Use when the user says the project is in "optimization, performance, efficiency, feature, and research mode" (or asks to research, baseline, audit or find better ways to do something in an engine or app). Sets the five goals, and a measure-first method: baseline and inventory, ranked findings as Backlog issues, nothing built until the user approves.
---

# Research mode

The user's own definition (2026-10-03), verbatim:

> I think we are in optimization, performance, efficiency, feature, and research mode.
>
> * Optimization: Looking for ways to optimize the code without the lose of functionality and features.
> * Performance: Looking for ways to increase performance ensuring a smooth and enjoyable experience while providing extensive functionality, expected and new features, with crisp and clean rendering.
> * Efficiency: Ensuring and expanding the efficiency of functionality and feature that will run great on all forms of desktop hardware.
> * Feature: looking for ways to expand the capabilities of the engine with expected and new features for a UI rendering engine.
> * Research: Performing research to achieve optimization, performance, efficiency, and feature goals. This means identifying our current functionality and features, researching what would be expected in a UI rendering engine, exploring new ideas and features that could be added, and working out ways to optimize the code, increase performance, expand efficiency, and implement/expand new and old features. If there is a better way to accomplish something, then we should be looking at it.

Read the five goals as one test for every idea: it must not lose functionality or features, it must keep rendering smooth and crisp, it must run well on all kinds of desktop hardware, and it should widen what the engine can do. "If there is a better way, look at it" is a standing instruction: question the current approach, not only the bugs.

## Method that worked (tre, Oct 2026)

Measure and survey first. Change no engine code until the user picks from ranked findings.

1. **Baseline.** Measure before optimizing: frame time per stage and per scenario (static window, large tree, scrolling, animated shader, many text nodes, video frames), idle CPU and wake-ups, memory per node, startup time. Reuse existing benchmarks. Say which hardware it was measured on and which it was not (a Mac, a Windows PC, low-end machines usually cannot be measured; that gap becomes its own held issue).
2. **Inventory and gaps.** List what the engine does, checked against the source, not recalled. Compare with what a UI rendering engine is expected to have. Mark each gap with its evidence. State when something was confirmed from code only because it could not be run.
3. **Test guesses against numbers.** Check each hypothesis (for example "the damage tracker walks the whole tree") by measuring or by a throwaway experiment that is reverted afterwards. Report what the experiment showed, and that it was reverted.
4. **Findings become Backlog issues**, one per idea, each with evidence, expected gain, effort and risk, ranked by benefit, effort and risk. Put the write-up page in the repo's docs design section and link it from the issues. The user reorders the ranking.
5. **Report honestly:** the headline numbers, the limits (one machine, one GPU vendor, one display server), and anything extra touched (for example a deleted orphan file).
6. **Then wait.** Nothing moves from Backlog until the user approves it. The user may drive with "approve N and start N+1": move N to Done, move N+1 to In progress, and follow the project's own board, commit and push rules.

## Keep going after the first pass

A second sweep is part of this mode, not an extra: look for expected features the first inventory did not list, and for newer ideas. Examples that were left unfiled after the tre baseline: subpixel (LCD) text, wide-gamut and HDR output, masks, overscroll. File them as Backlog issues the same way.

## Do not

- Do not optimize by intuition; every claimed gain needs a before and after.
- Do not trade away a feature or visual quality for speed; offer the fast path as opt-in when output would differ (as `glyph_cache` was).
- Do not push, merge or tag without the user's separate instruction.
