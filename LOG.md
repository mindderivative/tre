# Log: M21 Phase 2 — Real Cross-Platform Packaging CI (§13), closing M21

Corresponds to `BUILD_TRACKER.md` M21 Phase 2, closing M21 entirely
(both phases). `maturin-action` wired into GitHub Actions, matrixed
across Linux/macOS/Windows × supported Python versions.

## Investigation before writing code

`engine-py`'s own `pyo3` dependency has no `abi3-py*` feature —
confirmed via direct read — so real wheels are Python-version-
specific, not stable-ABI; this phase works with that real, pre-
existing fact rather than silently changing it. **Real, deliberate
design decision:** a new, separate `.github/workflows/wheels.yml`,
not an added job in `ci.yml` — building 3 platforms × 5 Python
versions of real wheels on every ordinary push would burn real CI
time for a per-commit gate; ARCHITECTURE.md's own text frames this as
"the full cross-platform release matrix," a real release-time
capability. Triggers only on a real `v*` tag push or a manual
`workflow_dispatch` run — the existing `ci.yml`'s own per-push jobs
stay completely untouched.

## What happened

New `.github/workflows/wheels.yml`: a `linux` job using `PyO3/
maturin-action`'s own real manylinux Docker container (`manylinux:
auto`) with `--find-interpreter` (builds wheels for every real
interpreter the container finds in one job, rather than a separate
container spin-up per Python version — the real, standard pattern for
manylinux builds); `macos`/`windows` jobs matrixed over Python 3.9–
3.13 via `actions/setup-python` (native runners, one interpreter at a
time); a real `sdist` job. Built artifacts uploaded via `actions/
upload-artifact`.

**Real, decisive verification — not simulated, actually triggered
twice:** pushed the workflow, then ran a real `workflow_dispatch`
trigger via `gh workflow run`. The first real run found a genuine,
previously-unknown gap: `linux` failed — `yeslogic-fontconfig-sys`'s
build script couldn't find `fontconfig`'s own pkg-config file. The
real manylinux container is a different, RPM-based image (CentOS/
AlmaLinux) than `ci.yml`'s own Ubuntu `test` job, with no fontconfig-
dev installed by default — a real, concrete finding only a real
triggered run could surface, not assumed or guessed. Fixed with
`before-script-linux: yum install -y fontconfig-devel || dnf install
-y fontconfig-devel || (apt-get update && apt-get install -y
libfontconfig1-dev)` — caught and fixed a real shell-logic bug in an
earlier draft of this exact fallback chain before it shipped (`||`/
`&&` operator precedence meant a successful `yum` would still fall
through to `apt-get` without the added parens).

The second real triggered run passed cleanly, all 12 jobs (5 macOS, 5
Windows, `linux`, `sdist`). **Real, decisive confirmation of Phase 1's
own prediction:** the real `linux` job's own build produced wheels
tagged `manylinux_2_17`/`manylinux2014` — the oldest, most broadly
portable tag — a dramatically better result than this dev host's own
locally-confirmed `manylinux_2_38` floor (M21 Phase 1), exactly
because the real manylinux container has a genuinely old glibc
baseline built in, with zero `zig` cross-compilation workaround
needed there at all. `--find-interpreter` discovered and built for 10
real interpreters in that one container run (`cp39` through `cp315`,
including free-threaded `cp314t`/`cp315t` variants, and even a real
PyPy 3.11 wheel) — downloaded and inspected directly, not assumed
from the build log alone.

Full `cargo test --workspace --release`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean (no
Rust-code changes this phase — pure CI-configuration work).
`ci.yml`'s own existing jobs confirmed untouched.

M21 — Real Wheel Packaging is now fully complete.

