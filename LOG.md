# LOG — Branch `0.3.1`: Release Prep

- User-directed: "Let's release the change. From here we will work on
  the 0.3.1 branch" -- M70's rename was merged to `main` (`27fc846`)
  but never tagged/released. This repo has always tagged releases
  straight off `main`, with no prior release-branch pattern, so the
  exact relationship between a new `0.3.1` branch and `main` was
  genuinely ambiguous -- clarified via `AskUserQuestion`: work
  accumulates on `0.3.1`, gets merged back into `main` when ready
  (keeping `main` always reflecting the latest released state, rather
  than letting it permanently diverge), and only then does `v0.3.1`
  get tagged/pushed for real. The version bump happens now; the actual
  tag/push waits on a later, separate, explicit confirmation, matching
  this session's own standing discipline for every real release
  trigger (the same one `019e7d4`/M69's own precedent already
  established).

## What shipped

1. Branch `0.3.1` created off `main`, post-M70-merge.
2. `Cargo.toml`/`pyproject.toml` bumped 0.3.0 → 0.3.1, mirroring the
   `019e7d4`/M69 precedent exactly. `Cargo.lock` updated via `cargo
   check`.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` -- every crate's own test
  count unchanged from M70's own already-merged state (engine-core
  229, engine-md3 24, engine-platform 11, engine-py 30, engine-render
  34, engine-spec 85), real, direct proof the version bump introduced
  zero behavior change; `maturin develop --release` (installed `tre`
  0.3.1); `pytest tests/` 831 passed, 2 skipped, unchanged; every file
  in `examples/` ran clean; `demo/showcase.py` all 5 phases, exit 0.

## Status

**Version bump complete, committed on the `0.3.1` branch.** Further
work continues on this branch going forward, per the user's own
explicit instruction. When ready, the plan is: merge `0.3.1` back into
`main`, then tag and push `v0.3.1` for real -- both steps wait on a
later, separate, explicit confirmation, not assumed here.

Next: whatever the user directs next, on the `0.3.1` branch.
