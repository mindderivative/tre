# Log: M21 Phase 1 — Real Built-Wheel Verification (§13)

Corresponds to `BUILD_TRACKER.md` M21 Phase 1: decide deliberately
between a manylinux-repaired portable wheel and a system-linked wheel
(the user's own explicit choice: manylinux-repaired), configure
`pyproject.toml` accordingly, build the real wheel, and verify it
end-to-end in a genuinely fresh venv.

## Investigation before writing code

`patchelf` (the real ELF-rewriting tool `maturin`'s own built-in
repair logic needs) was missing from this venv — installed via `pip
install patchelf`.

**Real, load-bearing finding, exactly the "don't let the default
silently decide" failure mode ARCHITECTURE.md's own text warns
against:** a bare `maturin build --release` with no flags on this
session's own dev machine (glibc 2.44 — a bleeding-edge host) silently
fell back to a plain, *non-portable* `linux` platform tag — "No
compatible platform tag found, using the linux tag instead" — with no
error. Explicit `--auditwheel repair --compatibility manylinux2014`
then failed loudly with a real, concrete reason: this dependency
set's own compiled symbols require `GLIBC_2.44` at the high end, far
newer than manylinux2014's `GLIBC_2.17` baseline allows.

Installed `maturin[zig]` (real cross-compilation with an older glibc
baseline shimmed in) and found a second real gap: the `ziglang` PyPI
package only installs a `python-zig` entry point, not a plain `zig`
executable `maturin` itself looks for on `PATH` — fixed with a
one-line shim script in `.venv/bin/zig` (gitignored, local-only).

`--zig --compatibility manylinux2014` then failed with a *different*
real reason — two *vendored system libraries* (`libbrotlicommon`/
`libfontconfig`, pulled in from `/usr/lib` at build time, not this
project's own compiled code) need `GLIBC_2.38`. Maturin itself
recommended `manylinux_2_38`, which built successfully; a subsequent
attempt at the more portable `manylinux_2_35` (which maturin's own
earlier log claimed the wheel was "eligible for") failed on the same
vendored `libfreetype`'s own `GLIBC_2.38` requirement — the real,
practical floor for this exact dependency set on this exact host is
`manylinux_2_38`, confirmed by actually trying narrower tags and
reading the real failure reasons, not guessed.

## What happened

Built the real wheel: `maturin build --release --zig --compatibility
manylinux_2_38` → `tre-0.1.0-cp314-cp314-manylinux_2_38_x86_64.whl`,
with six real system libraries (`libbrotlicommon`/`libbrotlidec`/
`libbz2`/`libfontconfig`/`libfreetype`/`libpng16`) vendored into the
wheel's own `tre.libs` directory — the exact class of vendoring
ARCHITECTURE.md's own TRE v1 account describes. **Real, deliberate
design decision:** `pyproject.toml` does *not* pin a `compatibility`
value — the real achievable tag is a fact about the build
*environment*'s own glibc, not a constant of this project; pinning
one chosen on this bleeding-edge dev host would needlessly narrow (or
outright break) a build on a genuinely older-glibc host, in particular
`maturin-action`'s own real manylinux Docker container (M21 Phase 2),
which will achieve a substantially broader, more portable tag
automatically with no zig cross-compilation needed at all. This
investigation's own real findings — the silent-fallback risk, the
`zig`/shim requirement, the confirmed-working local command — are
recorded directly in `pyproject.toml`'s own comments.

**Real, decisive verification, the exact missing step TRE v1 never
had:** installed the real built `.whl` directly (`pip install`, not
`-e`, not from source) into a genuinely fresh venv outside this
project entirely. `import tre` succeeded; five real example scripts
(`text_field.py` — real keyboard input; `clipboard.py` — real hermetic
copy/cut/paste; `theme.py` — real live MD3 theming; `two_windows.py`
— a real two-window session) all ran a real frame loop to completion
with no symptom of the double-loaded-library class of failure TRE v1
hit (no `XKBNotFound`, no GLib abort) — the winit/GTK `dlopen`'d
libraries ARCHITECTURE.md's own account names specifically aren't
even visible to `maturin`'s own static vendoring scan (confirmed via
direct read of its own build log, which only lists *directly linked*
libraries), so this real, live runtime exercise is the only way to
actually prove the specific failure mode TRE v1 hit doesn't reproduce
here — not merely inspecting the repair log.

Full `cargo test --workspace --release`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean (no
Rust-code changes this phase — pure build-configuration/verification
work). `maturin develop --release` + full `pytest tests/` (168
passed, unchanged, 1 skipped) and all twenty-seven examples confirmed
clean in the normal dev environment too.
