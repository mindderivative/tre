# LOG — Branch `0.4.0`: Milestone 1

- The user: "Scaffold 0.4.0 and start a new build tracker. Annotate as 0.4.0
  and restart the Milestones at 1. 0.4.0 will cover vello_hybrid fix. Any
  current issues for 0.3.5 will stay in as 0.3.5.x."
- Branch `0.4.0` off `main` at `a67376d`; version 0.4.0; the 0.3.x tracker
  archived as `BUILD_TRACKER_ARCHIVE_0.3.md`; a new `BUILD_TRACKER.md` with
  M1–M6 proposed. `vello_hybrid` 0.2.0 is still the latest release.
- Upstream renamed `vello_hybrid` to `vello_gpu` (PR #1883) and added the
  no-clear / rect-clear render (PR #1869); unpublished. The user, 2026-09-28:
  "D1 - pinned upstream commit, D2 - pinned Git dependency, D3 - small set
  with a full-redraw fallback, D4 - as recommended".
- Phase 2: pinned `b408cd00` (upstream `main`, green CI). `peniko`,
  `kurbo`, `skrifa`, `parley` match; `vello_common`/`glifo` from the same
  commit; `wgpu` 29 → 30 is the one major bump (68 source uses, 487 in
  tests). A spike on the commit built and showed both render settings.

## Status

**Complete (2026-09-28).** M2 next.
