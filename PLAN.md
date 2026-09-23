# PLAN — M71: Real Python API for Theme Resolution, `TextField` Composition Fields, and `View` Text Construction

*(Replaces the prior "Branch: 0.3.1" version-bump entry in this file
— that step is complete. This is Part 1 of the formal "shift to
Tesserae" plan approved via `EnterPlanMode`/`ExitPlanMode`; see this
file's own Milestone 71 section in `BUILD_TRACKER.md` for the full
real investigation.)*

## Goal
Expose the real, single, confirmed blocker to moving `window_
factory.rs`'s ~35 composition-only MD3 factories to Tesserae (Python):
`ThemeState::role`/`is_set`/`shape`/`elevation`/`typography` were all
`pub(crate)`, unreachable from Python. Also widen `add_text_field`
with the two real fields `add_code_editor` already sets internally
(`multiline`/`show_whitespace`), and give `View` a way to construct/
reconcile from pre-expanded YAML text while keeping hot-reload
watching the real source file.

## Real investigation
2 parallel Explore agents: `engine_core::NodeKind` has exactly 21 real
variants; the other 35 `add_*` factories are pure compositions with no
dedicated render state, blocked only by theme resolution having no
Python API. `View`'s own constructor reads a file path directly, no
in-memory-text path existed.

## Design (1 milestone, 4 phases)
1. `Theme` Python API.
2. `TextField` composition fields.
3. `View` text construction.
4. Verification, docs, commit.

## Status

**Complete, all four phases.**

New `Theme` pyclass (`window.rs`) wraps a cloned `SharedTheme`,
delegating to the exact already-correct `ThemeState` methods --
`role`/`is_set`/`shape`/`elevation`/`typography`. `Window.theme`
(`#[getter]`) returns a fresh wrapper each access, always live.
Registered in `lib.rs`, re-exported from `python/tre/__init__.py`.

`add_text_field` widened with `multiline`/`show_whitespace` (both
default `false`, a true no-op for existing callers) -- the exact two
fields `add_code_editor` already set internally with no Python
equivalent.

`View.__new__`/`poll_reload` both widened with a matching `source:
Option<String> = None`. **Real design correction found while
implementing:** `poll_reload()` unconditionally re-read `self.path`
from disk, entirely bypassing a constructor-only `source=` -- caught
by direct source reading before assuming the simpler design would
work, fixed by widening `poll_reload` itself too. The real file-
watcher still gates whether a reload happens at all; only the content
actually reconciled changes when `source` is given.

Tests: 2 new Rust unit tests (`view.rs`, GIL-free) proving `source=`
overrides are actually used, not just accepted. 19 new pytest tests
across `test_theme.py`/`test_text_field.py`/`test_hot_reload.py`, all
real behavioral proofs -- e.g. `multiline=True` genuinely making a
dispatched Enter key insert `\n` (confirmed via direct read of
`Tree::dispatch`'s own `Key::Enter` arm before writing the test), not
just "doesn't raise."

**A real bug caught and fixed while writing tests, not by inspection:**
an `Edit` to `test_text_field.py` initially displaced an existing
test's own real closing assertion (`assert field.get_text() ==
"untouched"`) past a large new inserted block -- caught by running the
new tests and seeing a `NameError` from the orphaned line, fixed by
restoring it to its rightful place.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean; `cargo test --workspace --release` (`engine-py` 32, up from 30,
+2; every other crate unchanged); `maturin develop --release` (tre
0.3.1 installed, into both `tre`'s own `.venv` and `tesserae/.venv`);
`pytest tests/` 850 passed, 2 skipped, up from 831, +19; every example
ran clean; `demo/showcase.py` all 5 phases, exit 0. `BUILD_TRACKER.md`
updated (Top Metrics, full Milestone 71 section, Up-next refreshed),
tracker regenerated (22 milestones/65 phases/173 items/3 known gaps/25
fixed gaps), artifact republished. Committing locally on the `0.3.1`
branch now.

Next: Tesserae-side Parts 2/3 (the widget catalog and YAML macro-
expansion layer), tracked in Tesserae's own `BUILD_TRACKER.md`.
