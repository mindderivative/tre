# Plan: Phase 21 — Release Prep (v0.1.0 Alpha)

**Status: Complete (2026-09-13).** See `documentation/IMPLEMENTATION.md`'s
own "Phase 21: Release Prep (v0.1.0 Alpha)" section for the technical
account. This file records the plan as designed via `EnterPlanMode`/
`ExitPlanMode` and approved by the project owner, archived here now that
the work is done, per this project's own "one active plan, archived once
real work begins" convention.

## User request (verbatim)

> It is time for a release plan for the project. Make a step-by-step plan
> and list our options for release.

Two decisions were confirmed directly with the project owner via
`AskUserQuestion` before finalizing this plan:
- **Scope**: a GitHub tag + Release with real release notes, plus
  publishing the existing local MkDocs site to GitHub Pages. Explicitly
  *not* a crates.io publish or a PyPI wheel for `tre-python` in this pass.
- **Framing**: an early/alpha release, Linux + Vulkan only, with
  DirectX 12/Metal and Windows/macOS windowing listed as roadmap
  placeholders rather than silently implied as supported.
- **License**: MIT OR Apache-2.0 (the Rust ecosystem default), confirmed
  separately after discovering — via a targeted grep across every `*.md`
  and `Cargo.toml` — that no license had ever been decided on anywhere in
  this repository.

## Design (as executed)

- `LICENSE-MIT` / `LICENSE-APACHE`: standard dual-license boilerplate text,
  two separate files (matching how `rust-lang/rust`/`tokio-rs/tokio`
  structure the identical dual license), copyright `mindderivative`.
- Root `Cargo.toml`: `license = "MIT OR Apache-2.0"` added to
  `[workspace.package]`; every one of the workspace's 16 crates (not 15 —
  this plan's own draft miscounted before checking `find crates -maxdepth
  2 -name Cargo.toml`) got `license.workspace = true` added to its
  `[package]` table via one uniform `sed` pass, verified afterwards with
  `cargo metadata` showing `"MIT OR Apache-2.0"` resolved for all 16.
- `CHANGELOG.md`: one curated `[0.1.0]` entry organized by subsystem,
  built from `documentation/IMPLEMENTATION.md`'s phase headings rather
  than copying its prose — explicitly states the Linux+Vulkan-only scope
  and lists DX12/Metal/Windows/macOS as known limitations up front.
- `README.md`: "Status" section reframed as release-facing (v0.1.0 early
  alpha, link to `CHANGELOG.md` and the Pages docs URL), new "Platform
  support" table separating what's real from what's a placeholder, and a
  new "License" section at the bottom pointing at both license files.
- `mkdocs.yml`: added the previously-unset `site_url` pointing at the
  GitHub Pages URL the new workflow will publish to.
- `.github/workflows/docs.yml`: a new, separate workflow (different
  trigger semantics and toolchain from `ci.yml`, so not folded into it) —
  a `build` job (`mkdocs build --strict`, uploads the site as a Pages
  artifact) on every push/PR/dispatch, and a `deploy` job
  (`actions/deploy-pages`) gated to `push` on `main` only.

## Real, disclosed remaining scope

Three things this phase's own local file changes cannot complete, each
requiring a separate, explicit go-ahead from the project owner rather than
being done autonomously (per this project's standing policy of never
pushing or touching public/remote state without direct authorization):

1. Enabling GitHub Pages itself (repo Settings → Pages → Source: GitHub
   Actions) — a repository settings change.
2. Pushing this phase's commit(s) to `origin/main`.
3. Tagging `v0.1.0`, pushing that tag, and creating the actual GitHub
   Release (`gh release create`) using `CHANGELOG.md`'s `[0.1.0]` section
   as the release body.

The `docs.yml` deploy job is therefore verified by inspection plus a local
`mkdocs build --strict` run, not a live Pages deployment, until confirmation
point 1 above happens — there is no way to observe a real Pages deploy
without it.

This phase also deliberately has no `demo/phase21/` folder, unlike every
prior phase — there is no runtime/engine behavior to demo here (licensing,
changelog, README, and a docs-only CI workflow), so an artificial demo was
not manufactured just to satisfy the convention's letter.
