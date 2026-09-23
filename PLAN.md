# PLAN — M69: Release Engineering: Automated Publish + Standalone `.so` (v0.3.0)

*(Replaces the prior M68 plan in this file — M68 is complete,
committed. Second of two milestones from the "wrap up before Tesserae"
request; see this file's own Milestone 69 section in `BUILD_TRACKER.md`
for the full real investigation.)*

## Goal
Build a real release, v0.3.0, with two real additions to the existing
wheel matrix: (1) the wheel matrix's own build output actually reaching
the GitHub Release automatically, and (2) a standalone, non-wheel-
packaged `.so` extension module for the separate Tesserae UI framework
project's own direct consumption.

## Real investigation
No automation has ever published a release -- v0.1.0/v0.2.0 were both
created by hand, confirmed via `git log`/commit-message investigation
(`13bfc7f`'s own commit body explicitly reserved release creation for
separate human authorization). The wheel matrix's own Python-version
list didn't include 3.14, this project's own real dev environment.
`cargo build -p engine-py --features pyo3/extension-module` produces a
raw `cdylib` byte-identical to maturin's own intermediate, confirmed
locally before committing to it as a CI step.

## Design (1 milestone, 2 phases)
1. Version bump + workflow changes.
2. Verification, docs, commit.

## Status

**Complete, both phases.**

`Cargo.toml`/`pyproject.toml` bumped 0.2.0 → 0.3.0. `wheels.yml`
widened: `3.14` added to the `macos`/`windows` Python matrix; a new
`standalone-so` job builds the raw extension module directly via
`cargo build --release`, names it the real CPython import-name
(`_core.cpython-314-x86_64-linux-gnu.so`), and uploads it; a new
`publish` job (`needs:` every build job, `softprops/action-gh-
release@v2`, top-level `permissions: contents: write` added) attaches
every wheel/sdist/the standalone `.so` to the tag's Release
automatically, `generate_release_notes: true` since this repo has no
`CHANGELOG.md`. `ARCHITECTURE.md` §13 gained a real "Built (Milestone
69)" paragraph describing this mechanism.

**A second real, pre-existing inaccuracy caught during verification:**
`docs/installation.md` claimed the release matrix builds CPython
3.9-3.15 including free-threaded `3.14t`/`3.15t` builds and PyPy
3.11 -- none of which `wheels.yml` has ever built. Fixed alongside
this milestone's own real matrix widening, since it's the identical
§13 packaging surface.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` every crate's count
unchanged (confirming the version bump introduced zero behavior
change); `maturin develop --release` (tre 0.3.0 installed); `pytest
tests/` 831 passed, 2 skipped, unchanged; every example ran clean;
`demo/showcase.py` all 5 phases, exit 0. Workflow YAML validated via
`yaml.safe_load` (job graph/`needs:`/`permissions:` all structurally
correct) plus manual review -- no way to trigger a real Actions run
without pushing. `mkdocs build --strict` re-run clean after the
`installation.md` fix. `BUILD_TRACKER.md` updated (Top Metrics, full
Milestone 69 section, Just-closed/Up-next refreshed), tracker
regenerated (20 milestones/59 phases/155 items/3 known gaps/25 fixed
gaps), artifact republished. Committing locally now.

**Pushed and live.** On explicit user confirmation ("Push"), the
accumulated commits and the `v0.3.0` tag were pushed -- the real
publish workflow ran for the first time in this project's history,
every job succeeded, and the release is live at `github.com/
mindderivative/tre/releases/tag/v0.3.0` with all 24 assets correct.
The real run's own asset list (`cp39`-`cp315`, free-threaded `cp314t`/
`cp315t`, `pp311`) proved the earlier `docs/installation.md` fix had
over-corrected -- the Linux `--find-interpreter` step genuinely builds
that full breadth automatically, just not on macOS/Windows. Fixed a
second time, verified against the real published assets. See `LOG.md`
for the full post-release correction writeup.

Next: nothing currently scoped. Both items from the "before we shift
focus to Tesserae" request are complete, pushed, and live. Further
work is the user's to direct.
