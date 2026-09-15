# Log: M3 Phase 1 — Workspace Scaffold

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 1. First real implementation of the v2 rebuild.

## What happened

1. **Found and fixed a real gap while writing the plan, before touching any code.** `ARCHITECTURE.md` §4's dependency diagram never drew an edge from `engine-render` to `engine-core`, even though §6's paint pass (walking `Node`/`PaintProperties` to build a `vello::Scene`) obviously requires it. The diagram was illustrating the *contested* edges (the engine-core/engine-md3 direction, the winit/engine-platform boundary) and simply omitted an uncontroversial one. Fixed in `ARCHITECTURE.md` (diagram edge `H --> D` + crate-boundary prose) before scaffolding, so the actual `Cargo.toml` files match a corrected, accurate graph rather than reproducing the gap in code. This is exactly the kind of thing that stays invisible on paper and only surfaces when you try to write the real dependency declarations — the whole reason implementation was the recommended next step over further design review.
2. Root `Cargo.toml` (workspace, 6 members) + `rust-toolchain.toml` (dev channel `1.98.0`, matching the toolchain actually installed; `rustfmt`+`clippy` components explicit, per TRE v1's own precedent of not trusting the ambient rustup profile on CI runners).
3. Six crate skeletons under `crates/`, each a minimal `Cargo.toml` (package metadata + only the local path dependencies §4 specifies, no external crates yet) and a placeholder `src/lib.rs` (a doc comment stating what's deferred and to which build-order step).
4. `rust-version` (MSRV) deliberately left unset — see `PLAN.md`'s reasoning (LESSONS_LEARNED.md §5: an unenforced claim becomes fiction; there's no real dependency yet to derive a floor from).

## Verification

```
$ cargo check --workspace
    Checking engine-core v0.1.0
    Checking engine-md3 v0.1.0
    Checking engine-platform v0.1.0
    Checking engine-render v0.1.0
    Checking engine-spec v0.1.0
    Checking engine-py v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

$ cargo clippy --workspace
    (clean, same crate list, no warnings)
```

Both commands exit 0 against the whole workspace. That proves: every declared local dependency path resolves, `engine-py`'s five-crate dependency list and every other crate's edges match §4 exactly (Cargo would hard-error on a typo'd path or a cycle), and every placeholder compiles clean under `clippy` as well as `check`.

## Deferred (by design, not oversight)

- All external crates (`vello_hybrid`, `wgpu`, `taffy`, `parley`, `accesskit`, `winit`, `pyo3`, ...) — pinned incrementally as each build-order step (§14) actually needs them, not speculatively now.
- `rust-version` / MSRV CI job — added together, in one commit, once a real dependency exists to check against.
- No `demo/` folder for this step — there's no runnable behavior yet to demonstrate; demos become meaningful starting at build-order step 1 (a real rendered rect).

## Next

`BUILD_TRACKER.md` M3 Phase 1 marked complete; M3 Phase 2 (Render Core Spike, §14 steps 1–4) is next.
