# LOG — M69: Release Engineering: Automated Publish + Standalone `.so` (v0.3.0)

- User-directed, same request as M68: "I want to build a release and
  as part of the normal release I want a single .so library release
  created. Tesserae will use the .so library of tre." Clarified via
  `AskUserQuestion`: version 0.3.0; the `.so` is a standalone compiled
  extension module attached directly as its own GitHub Release asset,
  not an `abi3` packaging switch (which would reverse `ARCHITECTURE.
  md`'s own recorded "per-version wheels, no abi3" decision -- full
  PyO3 API access was the stated reason for that choice).

## What shipped (single milestone, both phases)

1. Real investigation found no release has ever been published by
   automation -- `13bfc7f` ("Phase 21: Release prep for v0.1.0 alpha")
   explicitly reserved tagging/`gh release create` for separate human
   authorization, and both v0.1.0 and v0.2.0 were created entirely by
   hand. `wheels.yml` built the real wheel matrix but only ever did
   `actions/upload-artifact` -- CI-internal, ephemeral, never reaching
   the Release itself.
2. Version bumped: `Cargo.toml` workspace version + `pyproject.toml`,
   0.2.0 → 0.3.0, mirroring `019e7d4`'s exact prior pattern.
   `Cargo.lock` regenerated via `cargo check`.
3. `wheels.yml` widened: `3.14` added to the `macos`/`windows` jobs'
   Python-version matrix -- a real gap found while scoping the
   standalone `.so`: this project's own dev environment (Python 3.14.7,
   confirmed via `.venv`) had drifted ahead of what the matrix itself
   covered.
4. New `standalone-so` job: builds `engine-py`'s raw compiled
   extension module directly via `cargo build --release -p engine-py
   --features pyo3/extension-module` (confirmed locally, before
   committing to it as a CI step, to produce a `.so` byte-identical to
   maturin's own intermediate) -- bypassing maturin's wheel packaging
   entirely, since this asset is for the separate Tesserae UI
   framework project's own direct consumption on a matching dev
   environment, not portable end-user redistribution, so it
   deliberately runs on a plain `ubuntu-latest` runner rather than
   inside the `linux` job's manylinux container. Named the real
   CPython import-name maturin already uses locally (`_core.
   cpython-314-x86_64-linux-gnu.so`, confirmed via `.gitignore`'s own
   comment), so Tesserae can drop it straight into a vendored `tre/`
   package directory with zero renaming.
5. New `publish` job: `needs: [linux, macos, windows, sdist,
   standalone-so]`, top-level `permissions: contents: write` added
   (absent before -- every prior job ran under the default read-only
   token), `softprops/action-gh-release@v2` (no existing precedent in
   this repo to match, picked cold as the standard, well-maintained
   choice) downloads every artifact and attaches them to the pushed
   tag's Release in one step, `generate_release_notes: true` since
   this repo has no `CHANGELOG.md` to source notes from instead.
6. `ARCHITECTURE.md` §13 gained a new "Built (Milestone 69, both
   phases)" paragraph describing this real, now-automated mechanism,
   alongside the existing M21 wheel-matrix paragraph M68 already
   corrected.
- **A second real, pre-existing inaccuracy caught during Phase 2
  verification, unrelated to what M68 already covered:**
  `docs/installation.md` claimed the release matrix builds CPython
  3.9-3.15 including free-threaded `3.14t`/`3.15t` builds and PyPy
  3.11 -- none of which `wheels.yml` has ever built (no PyPy job, no
  free-threaded feature anywhere in the workflow). Fixed alongside
  this milestone's own real matrix widening, since it's the identical
  §13 packaging surface this milestone already touches.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` -- every crate's own test
  count unchanged (engine-core 229, engine-md3 24, engine-platform 11,
  engine-py 30, engine-render 34, engine-spec 83), real, direct proof
  the version bump introduced zero behavior change; `maturin develop
  --release` (installed `tre` 0.3.0); `pytest tests/` 831 passed, 2
  skipped, unchanged; every file in `examples/` ran clean; `demo/
  showcase.py` all 5 phases, exit 0. Workflow YAML validated by
  parsing it with `yaml.safe_load` (confirms the job graph, `needs:`,
  and `permissions:` block are all structurally correct) plus a direct
  manual review -- there's no way to trigger a real GitHub Actions run
  from here without actually pushing. `mkdocs build --strict` re-run
  clean, 0 warnings, after the `installation.md` fix.

## Status

**M69 is complete, both phases.** Both items from the user's "before
we shift focus to Tesserae" request -- M68's documentation refresh and
M69's release engineering -- are now done and committed locally.
Committing this milestone now; push deferred pending explicit user
confirmation, per standing policy.

**Gated, separate from this local commit:** pushing the accumulated
local commits and tagging/pushing `v0.3.0` -- which triggers the real
publish workflow for the first time in this project's history -- waits
on a final, separate, explicit confirmation, distinct from this
conversation's broader "build a release" authorization. Matching this
project's own standing policy, the same one the archived Phase-21
commit itself named for v0.1.0.

Next: nothing currently scoped. Further work is the user's to direct
-- most likely the start of the separate Tesserae UI framework project
itself, once the release is confirmed and pushed.
