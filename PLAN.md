# PLAN — M47: `VirtualList` Scrollbar Thumb + Build Tracker "Fixed Gaps"

## Goal
Two related asks in one message: (1) scope and build the one real,
still-open gap the last audit found -- `VirtualList` has never had a
real scrollbar/visual affordance; (2) add a "Fixed Gaps" section
(expand/collapse) to the Build Tracker, with a convention that closed
"Known gaps" get *moved* there, added to the reusable `build-tracker`
skill template, not just this project. Per the approved plan
(`/home/phil/.claude/plans/reflective-sleeping-falcon.md`).

## Part A — `VirtualList` scrollbar thumb
1. Read `ScrollView`'s own identical M38 Phase 6 capability as the
   concrete precedent: `ScrollViewState::thumb_geometry`, `Tree::
   grabs_scroll_view_thumb`/`update_scroll_view_thumb_drag`, `engine-
   render::paint_scroll_view_thumb`.
2. `VirtualListState` gained `thumb_drag_anchor` + `thumb_geometry`
   (using `total_extent()` for content extent, vertical-only).
3. `Tree` gained `virtual_list_viewport_extent`/`grabs_virtual_list_
   thumb`/`update_virtual_list_thumb_drag`, wired into `PointerPressed`
   's ancestor-walk and `update_drag`'s match.
4. `engine-render` gained `paint_virtual_list_thumb`, sharing a new
   `fill_scrollbar_thumb` helper with `paint_scroll_view_thumb`.
5. 6 new `engine-core` unit tests + 2 new `engine-render` pixel tests
   (found and fixed two real test-fixture bugs along the way: a
   too-small viewport left zero real drag travel; a shared item
   fixture's own opaque full-width paint covered the "no thumb" check
   point). `examples/scrollable_list.py` doc comment updated, matching
   `docking.py`/`resizable_panes.py`'s established honesty that no
   synthetic Python-level drag primitive exists for any drag gesture.

## Part B — Build Tracker "Fixed Gaps" (the template)
1. `~/.claude/skills/build-tracker/generate_tracker_artifact.py`
   (master copy, then re-copied verbatim to `tools/`): `Tracker` gained
   `fixed_gaps`; `parse_narrative` parses a second `**Fixed gaps:**`
   bullet list; new `render_fixed_gaps_section` renders a collapsed-
   by-default `<details>` archive; `main()`'s summary line gained the
   count.
2. **Real bug found and fixed along the way, not anticipated:** the
   existing "Just closed"/"Up next" `_grab()` searched the *whole*
   file and took the *last* match -- correct only if fresh pairs are
   appended, but this project's real convention prepends newest-first
   at the top, and the file also has many legitimate historical
   mentions buried in old milestone bodies. The published artifact's
   own highlight box had been showing stale (~M31-era) text for over a
   dozen milestones. Fixed by bounding the search to the real front
   matter (before the first `## Milestone` heading) and taking the
   first match there, falling back to the original whole-file/last-
   match behavior only when front matter has nothing (the *other* real,
   documented convention -- a single pair at the true tail -- needs
   that fallback, confirmed by re-testing `BUILD_TRACKER_TEMPLATE.md`
   itself after the first fix broke it).
3. `BUILD_TRACKER_TEMPLATE.md`/`SKILL.md` updated: template gained a
   `**Fixed gaps:**` block; SKILL.md's format section + maintenance
   workflow + "Lessons learned" documented both new conventions.

## Part C — Applied to this project's own `BUILD_TRACKER.md`
Split the 19-bullet "Known gaps" list (every bullet re-verified against
current source, not trusted from its own prior note) into a trimmed
"Known gaps" (2 genuinely open items) + new "Fixed gaps" (19 entries,
including three that had never actually been struck through despite
being real: context-menu dismissal, `Node.add_child`, `tracing`
wiring). Also fixed the newly-discovered stale "Up next" pointer
(unrefreshed since ~M18) and M47's own milestone section.

## Status
Complete. Full verification chain green: `cargo check`/`clippy -D
warnings`/`fmt`, `cargo test --workspace --release` (`engine-core` 219,
+6; `engine-render` virtual_list_scroll suite 4, +2), `maturin develop
--release`, `pytest tests/` (629 passed, unchanged), all 82 examples,
showcase demo. Tracker generator re-verified against both real
conventions (front-matter stack and single-tail-pair). **M47 -- is now
fully complete.** Push appropriate (tre repo); skill files are not
git-tracked (confirmed), no push needed there.
