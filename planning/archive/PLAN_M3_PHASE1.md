# Plan: M3 Phase 1 — Workspace Scaffold

Corresponds to `BUILD_TRACKER.md` M3 Phase 1. This is the first real implementation step of the v2 rebuild — everything before this was design (`ARCHITECTURE.md`, M1/M2).

## Goal

Stand up the Cargo workspace and the six crate skeletons `ARCHITECTURE.md` §12 specifies, wired with exactly the dependency edges §4 defines — no more, no less — and prove the graph is sound (buildable, acyclic) before any real logic exists in any of them. This is Design Principle 5 (de-risk before building on it) applied one level below the actual feature spikes: prove the *structure* compiles before proving any *behavior* does.

## Scope

In scope:
- Root `Cargo.toml` (workspace manifest) + `rust-toolchain.toml` (dev-channel pin, matching TRE v1's own precedent of separating the dev toolchain from the MSRV floor).
- Six crate directories under `crates/`, each with a minimal `Cargo.toml` and a `src/lib.rs` that compiles.
- Local (path) dependency edges only, exactly matching §4's diagram plus the `engine-render → engine-core` edge found missing while writing this plan (fixed in `ARCHITECTURE.md` first, see below).
- `cargo check` (or `build`) succeeding across the whole workspace as the verification step.

Explicitly out of scope (deferred to their own build-order steps, §14):
- Any external crate (`vello_hybrid`, `wgpu`, `taffy`, `parley`, `accesskit`, `winit`, `pyo3`, ...) — each gets pinned when the step that actually needs it runs, not speculatively now.
- `rust-version` (MSRV) — not claiming a floor before any real dependency exists to derive it from. TRE v1's own lesson (`LESSONS_LEARNED.md` §5) is exactly "an unenforced claim becomes fiction"; better to state none yet than state a guess.
- Any actual rendering/layout/FFI code — every crate's `src/lib.rs` is a placeholder.

## Dependency edges being wired (from §4)

| Crate | Depends on (local) |
|---|---|
| `engine-core` | — (base crate) |
| `engine-md3` | `engine-core` |
| `engine-render` | `engine-core` *(the edge this plan's prep work found missing from the diagram — fixed in `ARCHITECTURE.md` first)* |
| `engine-platform` | `engine-core` |
| `engine-spec` | `engine-core`, `engine-md3` |
| `engine-py` | `engine-core`, `engine-md3`, `engine-render`, `engine-platform`, `engine-spec` |

## Steps

1. Fix the `engine-render → engine-core` gap in `ARCHITECTURE.md` §4 (diagram edge + crate-boundary prose) — done ahead of this plan file, same session.
2. Write root `Cargo.toml` (workspace) + `rust-toolchain.toml`.
3. Scaffold each of the six crates: `Cargo.toml` (package metadata + the local deps above) + `src/lib.rs` placeholder.
4. `cargo check` the whole workspace; fix anything that doesn't compile.
5. Record this step in `LOG.md`, commit, update `BUILD_TRACKER.md` (mark both Phase 1 steps ✅) and regenerate/republish the tracker artifact.

## Verification

`cargo check --workspace` exits 0. That alone proves: every declared local dependency resolves, the graph has no cycle (Cargo would hard-error), and every crate's placeholder code is valid Rust.
