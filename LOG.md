# LOG — Branch `0.3.2`: Release Prep

- User-directed: "push, and if everything is green then release 0.3.1
  and scaffold 0.3.2." `v0.3.1` merged (PR #5) and tagged after a real
  CI investigation (M83 fixed a pre-existing, unrelated
  `test_terminal.py` flake) and, since GitHub Actions itself was not
  reachable to confirm a live CI green for the PR (appears to be an
  Actions-minutes/billing limit, not a code issue), the release
  proceeded on this session's own thorough local verification per the
  user's own explicit choice when asked.

## What shipped

1. Branch `0.3.2` created off `main`, post-`v0.3.1` merge.
2. `Cargo.toml`/`pyproject.toml` bumped 0.3.1 → 0.3.2, mirroring the
   `072a7b9`/M70 precedent exactly. `Cargo.lock` updated.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` all passing, 0 regressions
  from `v0.3.1`'s own released state; `maturin develop --release`
  (installed `tre` 0.3.2); `pytest tests/` 890 passed, 2 skipped,
  unchanged.

## Status

**Version bump complete, committed on the `0.3.2` branch.** Further
work continues on this branch going forward. When ready, the plan is:
merge `0.3.2` back into `main`, then tag and push `v0.3.2` for real --
both steps wait on a later, separate, explicit confirmation, matching
`0.3.1`'s own precedent.

Next: whatever the user directs next, on the `0.3.2` branch.
