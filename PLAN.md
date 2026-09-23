# PLAN — Branch `0.3.1`: Release Prep

*(Replaces the prior M70 plan in this file — M70 is complete, merged
to `main` via PR #1. This is a new, workflow-establishing request, not
a numbered milestone: see this file's own "Branch: 0.3.1" section in
`BUILD_TRACKER.md` for the full context.)*

## Goal
Release M70's rename (currently merged to `main` but untagged) as
`v0.3.1`. Establish a new, deliberate branch workflow going forward:
work accumulates on a dedicated `0.3.1` branch rather than `main`
directly, gets merged back into `main` when ready (so `main` always
reflects the latest released state, matching the existing tags-off-
main precedent), and only then gets tagged/pushed for real.

## Real clarification
This repo has always tagged releases straight off `main`, with no
prior release-branch pattern — genuinely ambiguous how a "0.3.1
branch" should relate to `main` going forward. Resolved via
`AskUserQuestion`: merge-back-then-tag (not a permanently-diverging
branch); version bump + branch creation now, actual tag/push held for
a later, separate, explicit confirmation.

## Status

**In progress.** Branch `0.3.1` created off `main` post-M70-merge.
`Cargo.toml`/`pyproject.toml` bumped 0.3.0 → 0.3.1, mirroring the
`019e7d4`/M69 precedent exactly. `Cargo.lock` updated.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` every crate's own count
unchanged from M70's own state (engine-core 229, engine-md3 24,
engine-platform 11, engine-py 30, engine-render 34, engine-spec 85);
`maturin develop --release` (tre 0.3.1 installed); `pytest tests/` 831
passed, 2 skipped, unchanged; every example ran clean; `demo/
showcase.py` all 5 phases, exit 0. `BUILD_TRACKER.md` updated (new
"Branch: 0.3.1" section, Up-next refreshed), tracker regenerated (21
milestones/61 phases/161 items/3 known gaps/25 fixed gaps), artifact
republished. Committing on the `0.3.1` branch now.

Next: further work continues on this branch. When ready, merge back
into `main` and tag/push `v0.3.1` for real — both wait on a later,
separate, explicit confirmation.
