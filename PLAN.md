# Plan: M21 Phase 2 — Real Cross-Platform Packaging CI (§13), closing M21

Corresponds to `BUILD_TRACKER.md` M21 Phase 2, closing M21 entirely:
`maturin-action` wired into GitHub Actions, matrixed across
Linux/macOS/Windows × supported Python versions.

## Investigation before writing code

- `engine-py`'s own `pyo3` dependency has no `abi3-py*` feature —
  confirmed via direct read of `Cargo.toml`. This means real wheels
  are Python-version-specific (one per exact CPython minor version),
  not stable-ABI — a real, pre-existing fact this phase works with,
  not silently changes (switching to `abi3` is real, separate scope).
- `pyproject.toml` states `requires-python = ">=3.9"`, an open lower
  bound with no stated ceiling. A real, defensible "supported Python
  versions" matrix for this phase: 3.9 (the stated floor) through 3.13
  (the newest stable release at a reasonable, real cutoff) — matching
  common real-world practice for a Rust/Python package (this project's
  own dev environment already runs 3.14, confirmed via `.venv`, but a
  brand-new release isn't yet a real "supported" floor to commit CI
  resources to).
- **Real, deliberate design decision:** a full 3-platform × 5-Python-
  version wheel matrix (15 real build jobs, several needing a real
  manylinux Docker container or a real Windows/macOS runner) is
  genuinely expensive CI resource/time for a per-commit gate — this
  session alone has pushed roughly a dozen commits in the last few
  hours. ARCHITECTURE.md's own text frames this as "the full cross-
  platform release matrix," a real release-time capability, not a
  per-push CI gate — matching how most real Rust/Python packages
  (confirmed by common real-world practice) build release wheels on a
  tag/release event or manual dispatch, not on every ordinary commit.
  A **new, separate** GitHub Actions workflow file, triggered on
  `workflow_dispatch` (manual) and a real `v*` tag push, is the real,
  proportionate design — the existing `ci.yml`'s own per-push
  `test`/`test-windows`/`test-macos` jobs stay completely untouched.
- `maturin-action` (`PyO3/maturin-action`) is the real, standard
  GitHub Action ARCHITECTURE.md's own text names — handles the real
  manylinux Docker container build on Linux automatically (no zig
  workaround needed there, confirmed by this phase's own investigation
  in Phase 1: a real manylinux container already has an old enough
  glibc baseline natively) and native builds on macOS/Windows.

## Design

- New `.github/workflows/wheels.yml`: `workflow_dispatch` + `push:
  tags: ["v*"]` triggers (never on an ordinary push to `main`).
- Three real jobs (`linux`, `macos`, `windows`), each matrixed over
  Python 3.9–3.13, using `PyO3/maturin-action` with `--release`
  (Linux gets `manylinux: auto`, `maturin-action`'s own real default
  that picks a real manylinux container — no zig/shim workaround
  needed, unlike this phase's own local dev-host investigation).
  Built wheels uploaded as real workflow artifacts (`actions/upload-
  artifact`), the real, inspectable proof a manual run actually
  produced them.
- A final `sdist` job builds a real source distribution too
  (`maturin build --sdist`), matching ARCHITECTURE.md's own §13 text
  naming both.

## Verification plan

Real, not simulated where possible: validate the new workflow file's
own YAML syntax; trigger a real `workflow_dispatch` run via `gh
workflow run` and watch it to completion, inspecting the real
uploaded wheel artifacts it produces (their real platform tags) — the
first real proof this phase's own configuration actually builds valid
wheels on real GitHub-hosted runners, not just locally. Existing
`ci.yml` jobs re-verified unaffected (a new, separate workflow file
touches nothing there). `LOG.md`/`BUILD_TRACKER.md`/tracker artifact/
commit/push/memory — closing M21 entirely (both phases).
