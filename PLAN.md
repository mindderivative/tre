# Plan: M21 Phase 1 — Real Built-Wheel Verification (§13)

Corresponds to `BUILD_TRACKER.md` M21 Phase 1: decide deliberately
between a manylinux-repaired portable wheel and a system-linked wheel,
configure `pyproject.toml` accordingly, build the real wheel, and
verify it end-to-end in a genuinely fresh venv.

## Investigation before writing code

- Confirmed via `maturin build --help`: `--auditwheel repair` is real
  and, per ARCHITECTURE.md's own text, is what happens *by default*
  when `--auditwheel skip` isn't passed — the default `--compatibility`
  is "the lowest compatible `manylinux` tag" auto-detected for this
  dependency set. `pyproject.toml` currently has zero `[tool.maturin]`
  auditwheel/compatibility configuration at all (confirmed via direct
  read) — the exact "don't let the default silently decide this"
  situation ARCHITECTURE.md's own text warns against, even though the
  default's *behavior* already happens to match the user's own chosen
  direction (manylinux-repaired).
- `patchelf` (the real ELF-rewriting tool `maturin`'s own built-in
  repair logic needs — confirmed maturin does not shell out to the
  Python `auditwheel` package, it has its own Rust implementation) was
  missing from this venv (flagged in every prior `maturin develop`
  run's own warning this session: "Failed to execute 'patchelf'").
  Installed via `pip install patchelf`.
- **Decision, made explicitly, not left to the default:** manylinux-
  repaired, portable wheel — the user's own explicit choice. Rather
  than hardcoding a specific old manylinux tag (e.g. `manylinux2014`)
  blindly, which risks a real, unverifiable failure if this dependency
  set's own real glibc/symbol requirements don't actually support that
  old a target, this phase makes the *request* explicit (`--auditwheel
  repair`, not left unspecified) while letting maturin's own real
  auto-detection report which tag is *actually achievable* for this
  exact dependency set — informed, not silent, and verified by
  actually building and inspecting the real artifact, not assumed.

## Design

- `pyproject.toml`'s `[tool.maturin]` gains an explicit `compatibility`
  setting once the real achievable tag is confirmed by a first real
  build (see verification below) — recorded deliberately, not left
  for maturin's own CLI default to decide silently on a future build.
- Build the real wheel via `maturin build --release` (not `develop`,
  which never exercises the repair path at all).
- Install the real built `.whl` file into a genuinely fresh venv
  (no dev-time build artifacts, no editable install) and verify
  `import tre` plus a real example script actually runs — the same
  real, end-to-end verification TRE v1's own missing coverage let a
  production segfault through.

## Verification plan

Real, not simulated: `maturin build --release`, inspect the resulting
wheel's own filename tag and `auditwheel show`-equivalent output
(maturin logs which libraries it repaired/vendored); create a fresh
`venv` outside this project's own `.venv`, `pip install` the built
wheel directly (not `-e`, not from source), run `python -c "import
tre"` and a real example script inside that fresh environment;
confirm no double-loaded-library symptom (the exact TRE v1 failure
mode) by checking the process actually runs a real frame loop to
completion, not just imports. `LOG.md`/`BUILD_TRACKER.md`/tracker
artifact/commit/push/memory.
