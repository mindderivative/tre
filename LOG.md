# LOG — M71: Real Python API for Theme Resolution, `TextField` Composition Fields, and `View` Text Construction

- User-directed: "Let's shift to Tesserae... The goal here is to make
  an UI framework API that uses TRE to allow developers to create
  desktop applications without needing to touch Rust... Unless direct
  rendering is required all other components should be developed
  using Tesserae." Part 1 of a formal plan (`EnterPlanMode`/
  `ExitPlanMode`, `AskUserQuestion`-confirmed) spanning both `tre` and
  the sibling `tesserae` repo. Real investigation (2 parallel Explore
  agents) found `engine_core::NodeKind` has exactly 21 real variants --
  the other 35 real `add_*` factories in `window_factory.rs` are pure
  compositions of `Rect`/`Text`/existing kinds with no dedicated
  render state of their own, blocked from moving to Python by exactly
  one real thing: `ThemeState::role`/`is_set`/`shape`/`elevation`/
  `typography` were all `pub(crate)`, reachable only from inside this
  crate, with zero Python-facing equivalent anywhere (confirmed via
  grep before implementing).

## What shipped

1. New `#[pyclass(unsendable, name = "Theme")]` in `window.rs`,
   holding a cloned `SharedTheme` (the identical cheap-`Rc`-clone shape
   `Node`'s own `theme` field already uses) -- `role(name) ->
   Option<(u8,u8,u8,u8)>`, `is_set() -> bool`, `shape(component,
   variant=None) -> Option<f64>`, `elevation(component, variant=None)
   -> Option<f64>`, `typography(role) -> Option<(family, weight, size,
   line_height)>`, each a thin delegate to the exact already-correct
   `ThemeState` method. `Window.theme` (`#[getter]`) returns a fresh
   wrapper each access -- always live, including after a later
   `set_theme()` call. Registered in `lib.rs`'s `_core` module,
   re-exported from `python/tre/__init__.py`/`__all__`.
2. `add_text_field` widened with `multiline: bool = false`/
   `show_whitespace: bool = false` -- the exact two real `TextFieldState`
   fields `add_code_editor` already sets internally with no
   Python-facing equivalent; both default `false`, a true no-op
   widening for every existing caller.
3. `View.__new__` widened with `source: Option<String> = None` --
   when given, used directly instead of reading `path` from disk;
   `path` stays required, still supplying `include:`/`image.src:`
   base-dir resolution and the real file `ViewWatcher` watches.
   **Real design correction found while implementing, not assumed
   from the plan:** `poll_reload()` unconditionally re-reads
   `self.path` from disk on every real detected change, entirely
   bypassing whatever `source=` the constructor was given -- confirmed
   by direct read before assuming the constructor-only change would be
   enough. Widened `poll_reload` with its own matching `source:
   Option<String> = None` -- the real file-watcher (`watcher.
   poll_changed()`) still gates whether a reload happens at all (a
   real, cheap, inotify-backed check, not bypassed), but the content
   actually reconciled is `source` when given, not a fresh disk read.
- Tests: 2 new Rust unit tests (`view.rs`, GIL-free) proving both
  `source=` overrides are actually used, not just accepted --
  constructing/reconciling against text that differs from the real
  on-disk file's own content, then reading the live `Tree` back to
  confirm which value won. 19 new pytest tests across `test_theme.py`
  (13 -- `is_set`/`role`/`shape`/`elevation`/`typography` each proven
  against real `components:`/`typography:` overrides, unknown names
  returning `None`, a live cross-check feeding `role("primary")`
  straight into a real `add_rect` construction), `test_text_field.py`
  (3 -- a real behavioral proof that `multiline=True` + a real
  dispatched Enter key genuinely inserts `\n`, confirmed via direct
  read of `Tree::dispatch`'s own `Key::Enter` arm before writing the
  test, not just "doesn't raise"), and `test_hot_reload.py` (3 -- the
  real Python binding wiring, end to end, for both `View(source=...)`
  and `poll_reload(source=...)`).
- **A real bug caught and fixed while writing tests, not by
  inspection:** an `Edit` to `test_text_field.py` initially displaced
  an existing test's own real closing assertion (`assert field.
  get_text() == "untouched"`) past a large new inserted block of test
  code -- caught immediately by running the new tests and seeing a
  `NameError` from the orphaned, now-unreachable-context line, fixed
  by restoring it to its rightful place in the original test.
- Verification: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean; `cargo test --workspace --release` (`engine-py` 32, up from
  30, +2; every other crate's own count unchanged); `maturin develop
  --release` (installed `tre` 0.3.1 into both `tre`'s own `.venv` and
  `tesserae/.venv`, so Tesserae-side work can build against the real
  new API immediately without waiting on a release); `pytest tests/`
  850 passed, 2 skipped, up from 831, +19; every file in `examples/`
  ran clean; `demo/showcase.py` all 5 phases, exit 0.

## Status

**M71 is complete, all four phases.** The real, single, confirmed
blocker to moving `tre`'s composition-only MD3 catalog to Tesserae is
closed. Committed locally on the `0.3.1` branch, not `main`; push
deferred pending explicit user confirmation, per standing policy.

Next: Tesserae-side Parts 2/3 of the approved plan -- the real widget/
component catalog and the YAML component macro-expansion layer, both
tracked in Tesserae's own `BUILD_TRACKER.md`, not here (per the user's
own explicit direction that Tesserae-scoped work goes in the Tesserae
project).
