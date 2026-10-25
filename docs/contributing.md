# Contributing

## Development setup

From a fresh machine (Linux shown; macOS and Windows need only Rust and Python):

```bash
# 1. System packages (Debian/Ubuntu names; the same libraries exist elsewhere).
#    The list CI uses is in .github/workflows/ci.yml; see Installation for details.
#    python3-dev is needed here, not in CI: `cargo test` links engine-py against libpython.
sudo apt-get install -y build-essential pkg-config git python3 python3-dev python3-venv \
    libxkbcommon-dev libwayland-dev libx11-dev libxrandr-dev libxi-dev libxcursor-dev \
    libxinerama-dev libudev-dev libfontconfig1-dev mesa-vulkan-drivers libgl1-mesa-dri

# 2. Rust: rustup installs the toolchain pinned in rust-toolchain.toml by itself.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. The code, a venv with the Python tools, and the extension built into it.
git clone https://github.com/mindderivative/tre.git
cd tre
git checkout 0.5.6                 # or main; see "Branches and releases" below
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements-dev.txt
maturin develop --release

# 4. Check that everything works.
tools/verify.sh
```

`tools/verify.sh` is the whole check chain in one script (below), and
`tools/verify.sh --cross` adds the Windows and macOS clippy runs and the MSRV
check. The live-window tests run only with a display (a desktop session, X11 or
Wayland) and skip without one. On a machine with no GPU, Mesa's software Vulkan
(`mesa-vulkan-drivers`, lavapipe) is enough: set
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json`.

See [Installation](installation.md) for the Rust toolchain requirement
and portable-wheel build notes.

## Branches and releases

Each minor line has its own branch cut from `main` (`0.5.5`, `0.5.6`, ...); its
work is committed there, one commit per finished item with the issue number
(`0.5.6: what it does (#166)`), and the version in `Cargo.toml`, `pyproject.toml`
and `Cargo.lock` is bumped on the branch. Nothing is released until it is told to:

1. Push the branch and open a pull request into `main`; CI (`ci.yml`) runs on it. Bumping the
   version makes it the [full tier](#ci-tiers), so the release PR gets the whole run.
2. Merge it. The merge also deploys the docs to GitHub Pages (`docs.yml`).
3. Tag the merge commit `vX.Y.Z` (annotated) and push the tag. That starts
   `wheels.yml`: it builds the wheels and sdist, creates the GitHub Release, and
   then waits at the **`pypi` environment** for a reviewer's approval before it
   uploads to PyPI (trusted publishing, no token). PyPI never accepts the same
   version twice, so check the version before tagging.
4. After approval, `pip install tesserae-engine==X.Y.Z` in a clean venv is the
   check that it is live.

## The GitHub project

Work is tracked on the GitHub project **Tesserae Rendering Engine** (project 3 of
`mindderivative`). An item moves **Backlog -> Ready -> In progress -> In review ->
Done**: an issue is drafted into Backlog with a scope comment (what goes in, what
is left out and why, size, risks); once the scope is approved it goes to Ready;
In progress while it is built; In review when it is committed, with a comment of
what was checked and what was not; Done when the work is approved. Findings,
failures and fixes go in the issue's comments.

[Tesserae](https://github.com/mindderivative/tesserae) (the UI framework built on
`tre`) files its requests as issues here. For each one, the scope is agreed with
the Tesserae side before the work starts, and a local wheel is handed over for it
to test before anything is published.

Picking those requests up on a timer is written down in
[`tools/intake-routine.md`](https://github.com/mindderivative/tre/blob/main/tools/intake-routine.md):
the process and the prompt to run in Claude Code, so it can be restarted on a new
machine.

### What has not been checked on real hardware

Open issues #47, #48, #91, #118 and #132-#134 are checks that need a Mac, a
Windows PC, touch hardware or real GPUs. CI runs on software GPUs, and nothing
else is verified on real hardware yet, notably: an IME (East Asian input) on any
platform, screen-reader output for the accessibility states, the feel of scroll
snapping with a real wheel or touch fling, window placement outside Wayland, and
`skip_taskbar` on Windows.

## CI tiers

`ci.yml` does not run everything on every pull request. A small job, `plan`
(`tools/ci_plan.py`), reads what the change touches and picks one of three tiers; **`CI result`**
is the one job that says whether it passed (a job the tier does not use is skipped, which is fine).
The plan is also printed in the run's summary, with the reason.

| Tier | When | What runs |
|---|---|---|
| **docs** | Only documentation changed: `docs/`, any `*.md`, `mkdocs.yml`, `.claude/`. | `mkdocs build --strict`, the generated Python API page is current, `CLAUDE.md` still only imports `AGENTS.md`, the version agrees. No Rust, no Python tests, no examples. |
| **partial** | Code changed: the standard run. | `cargo fmt --check`; clippy and `cargo test` for the **changed crates and every crate that depends on them** (a change to `engine-core` reaches all four, one to `engine-render` reaches `engine-render` and `engine-py`); then, when Python is reached, the extension is built, `mypy --strict`, stubtest and the API page are checked, and the **Python tests that cover the change** run, with **the examples that relate to it** (the ones that name what changed or share a word with the file's name, such as `gradient.rs` and `gradients.py`) and three broad ones (`showcase.py`, `layout.py`, `animation.py`). Linux only. |
| **full** | A release (the workspace `version` in `Cargo.toml` moved, such as 0.5.7 to 0.5.8); a build-configuration change (`Cargo.lock`, `rust-toolchain.toml`, the root `Cargo.toml`, `ci.yml`); the `full-ci` label on the PR; or a manual run (`Actions` > `CI` > `Run workflow`). | Everything: Linux (build, test, clippy, fmt, mypy, stubtest, the whole pytest suite, **all 45 examples**), Windows, macOS and the MSRV check. |

Which Python tests a partial run takes. The crates are layered: `engine-core` is under
`engine-render` and `engine-platform`, which are under `engine-py`, the Python surface. A change to
a lower crate reaches Python indirectly, and its Rust names are not in the Python tests, so it runs
the **whole** pytest suite. A change to `engine-py` or `python/` runs the tests that name what
changed (the same match `tools/tests_for.py` prints) plus a small smoke set, and the whole suite
when a changed file is named by no test or the picks are over 40% of the suite. That match is by
name, not coverage data, so it can miss a test that reaches the code another way; add the `full-ci`
label to a PR that deserves the whole run. The pytest picks, and the live window tests that skip on
a runner with no display, are listed in the job's log.

To see the plan for your branch before pushing: `python3 tools/ci_plan.py --base origin/main`.

## Running the checks

`tools/verify.sh` runs all of these in order and stops at the first failure.
Each on its own:

```bash
cargo test --workspace --release
cargo clippy --workspace --all-targets --release -- -D warnings
cargo fmt --all --check
cargo +1.90 check --workspace --all-targets   # MSRV
python -m pytest tests/
mypy --strict python/tre
python -m mypy.stubtest tre --allowlist tools/stubtest_allowlist.txt
mkdocs build --strict
```

The damage tracker walks only the nodes that changed (see
`crates/engine-render/src/damage.rs`). `TRE_DAMAGE_VERIFY=1` makes every
tracker also walk the whole tree and panic if the two disagree, so run the
suites that way after touching anything that writes to a node, the layout or
the tracker, and CI does: `TRE_DAMAGE_VERIFY=1 cargo test --workspace`. The
randomised `damage_incremental` tests drive it with thousands of edits.
`TRE_DAMAGE_FULL=1` turns the shortcut off, to compare timings.

`stubtest` compares the type stub with the built extension, so run it after
`maturin develop`; CI runs it and `mypy --strict` on Linux.

Platform-only code (`#[cfg(target_os = ...)]`, such as the macOS title bar
and the Windows double-click time) can be type-checked from Linux without
the other OS's SDK:

```bash
rustup target add aarch64-apple-darwin x86_64-pc-windows-msvc
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/macos \
  cargo clippy --workspace --all-targets --target aarch64-apple-darwin -- -D warnings
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/windows \
  cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

This only compiles; CI's macOS and Windows jobs run the tests.

Building on Linux needs a few system packages; see
[Installation](installation.md#option-3-build-from-source).

Every real engine capability has its own headless, GPU-backed pixel test
under `crates/engine-render/tests/`, proving it actually paints what it
claims to, not just that the code compiles.

## Helper scripts

Small scripts in `tools/` take over the routine parts of the workflow. All of them read before
they write, and the ones that change something (`pr_chain.py`, `board.py move`/`create`,
`restamp_version.py`) do nothing until told to (`--yes`, `--apply`) or show a dry run first.

| Script | What it does |
|---|---|
| `tools/status.sh` | Branch, the chain of local branches, unpushed work, version, the newest `LOG.md` section and the board, in one screen. |
| `tools/branches.py` | The local branches as a tree: commits over the parent, merged, pushed. |
| `tools/tests_for.py` | The clippy, cargo and pytest commands that cover the files you changed (by name match; it runs nothing). |
| `tools/with_adapter.sh` | Runs a command on one GPU adapter: `radv`, `lvp` (llvmpipe) or `fallback`. |
| `tools/sway_live.sh` | Runs a command inside a private headless sway, for the live `LayerWindow` tests, and shuts it down. |
| `tools/pr_chain.py` | Status, push, open, retarget (with the close and reopen that start CI), wait, merge and cleanup for a stack of branches. |
| `tools/board.py` | The project board: `status`, `move` (Done needs `--approved`) and `create` (an issue into Backlog or Ready). |
| `tools/check_version.py` | Every place the version lives agrees; `--expect V --pypi` before a release. |
| `tools/restamp_version.py` | Re-stamps "X.Y.Z (#N)" notes written for a version that was released without that work. |
| `tools/wrapup.py` | What `LOG.md`, the docs and the board still need after a piece of work. |

## Checking transparent windows on a new platform

Transparency, blur-behind and click-through depend on the OS and compositor, and
the test suite can only prove them on the machine it runs on. On Windows, macOS,
GNOME or any other desktop, run `python tools/check_transparency.py`: it opens
a frameless transparent window, steps through three phases (normal,
click-through, normal) and prints a report to paste into the issue. The lines
marked LOOK need your eyes.

## Building this documentation site

```bash
pip install mkdocs mkdocs-material
mkdocs serve   # live preview at http://127.0.0.1:8000
mkdocs build   # static site in site/
```

!!! note
    Don't use `pip install '.[docs]'` for this — installing extras from a
    local `pyproject.toml` always builds the base package too (Tesserae Engine's
    real Rust extension), which needs a full Rust toolchain and
    `fontconfig` dev headers you don't need just to build docs. The
    plain install above only ever runs `mkdocs`, never imports `tre`.

## Project discipline

This project holds itself to one discipline end to end for every real
change: investigate → plan → implement → test → document → commit. See
the GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3) (0.5.0 on; its
README describes how a step is recorded),
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md), and
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tre/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md) for the
milestone-by-milestone history this discipline has produced, and
[Architecture](architecture.md) for the design principles that shape it.

Commit messages match the existing history's style (see `git log`); real,
empirically-verified findings are preferred over assumptions — a claim
about behavior is checked by actually running the code, not inferred
from reading it alone.

## The API pages

[`docs/api/rust.md`](api/rust.md) and [`docs/api/python.md`](api/python.md) are
generated, not written by hand. Regenerate them when a public item changes:

```bash
# Rust: rustdoc's JSON, then the page (descriptions come from doc comments;
# tools/rust_api_descriptions*.json replaces any that read badly)
for c in engine-core engine-render engine-platform; do
  RUSTC_BOOTSTRAP=1 cargo rustdoc -p $c --lib -- -Z unstable-options --output-format json
done
python tools/gen_rust_api.py

# Python: from python/tre/_core.pyi (the Node and Window property and event
# tables are at the top of the script)
python tools/gen_python_api.py
```

Write a doc comment's first sentence as the one-line description a reader
wants: it is what the page shows.
