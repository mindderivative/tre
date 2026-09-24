# PLAN — Branch `0.3.2`: Release Prep

*(Replaces the prior M74 plan in this file — M74 through M83 are all
complete, released as `v0.3.1`. This is a new, workflow-continuing
branch scaffold, not a numbered milestone: see this file's own
"Branch: 0.3.2" section in `BUILD_TRACKER.md` for the full context.)*

## Goal

`v0.3.1` is tagged and released (`github.com/mindderivative/tre/releases/tag/v0.3.1`),
`main` merged and up to date. Start the next real release-prep branch,
`0.3.2`, per the same deliberate workflow `0.3.1` itself established
(`072a7b9`/M70's own precedent): work accumulates here rather than on
`main` directly, merges back when ready, and only then gets tagged/
pushed for real.

## Status

**In progress.** Branch `0.3.2` created off `main`, post-`v0.3.1`
release. `Cargo.toml`/`pyproject.toml` bumped 0.3.1 → 0.3.2, `Cargo.lock`
updated via `cargo check`.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` all passing, 0 failures,
counts unchanged from `v0.3.1`'s own released state; `maturin develop
--release` (tre 0.3.2 installed); `pytest tests/` 890 passed, 2
skipped, unchanged. `BUILD_TRACKER.md` updated (new "Branch: 0.3.2"
section), tracker regenerated and republished. Committing on the
`0.3.2` branch now.

Next: whatever the user directs next, on the `0.3.2` branch.
