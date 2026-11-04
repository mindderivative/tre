# Upgrading to 0.6.x

0.6.0 renames the Python package from `tre` to `tess_re`, and the environment
variables from `TRE_*` to `TESS_RE_*`. Nothing else changed: every class,
function, property and event keeps its name, so a search and replace is the
whole migration. There is no alias and no fallback: `import tre` raises
`ModuleNotFoundError` on 0.6.0, and the old variables are ignored.

The names moved so that the Tesserae projects share one prefix and cannot
collide with other packages: `tess_re` (this engine), `tess_ui`, `tess_el`,
`tess_dl` and `tess_shell`.

The PyPI name does not change: `pip install tesserae-engine`.

## 0.6.0

### Renamed

- **The import name.** `import tre` becomes `import tess_re`, `from tre import
  Window` becomes `from tess_re import Window`, and so on for `tre._core`,
  `tre.snapshot` and the rest. A project that supports both 0.5.x and 0.6.x
  can write `try: import tess_re as tre` / `except ModuleNotFoundError: import
  tre`.
- **The environment variables.** Add `TESS_RE_` in place of `TRE_`; the rest of
  each name is unchanged.

  | Before | After |
  |---|---|
  | `TRE_GPU_DEVICES` | `TESS_RE_GPU_DEVICES` |
  | `TRE_GPU_MEMORY` | `TESS_RE_GPU_MEMORY` |
  | `TRE_GPU_RENDERERS` | `TESS_RE_GPU_RENDERERS` |
  | `TRE_DAMAGE_FULL` | `TESS_RE_DAMAGE_FULL` |
  | `TRE_DAMAGE_VERIFY` | `TESS_RE_DAMAGE_VERIFY` |
  | `TRE_SCROLL_BLIT_MIN_NODES` | `TESS_RE_SCROLL_BLIT_MIN_NODES` |
  | `TRE_SCROLL_BLIT_OFF` | `TESS_RE_SCROLL_BLIT_OFF` |
  | `TRE_FORBID_REMOVED` | `TESS_RE_FORBID_REMOVED` |

  The variables that only the test suite and the tools read changed the same
  way: `TRE_TEST_FORCE_FALLBACK`, `TRE_TEST_PARTIAL`, `TRE_TEST_SIZE`,
  `TRE_TEST_WINDOWS`, `TRE_LIVE_INPUT`, `TRE_LIVE_CLIPBOARD`,
  `TRE_LIVE_BOTTOM_INSET`, `TRE_PRIVATE_COMPOSITOR`, `TRE_RUN_BENCHMARK` and
  `TRE_ICD_DIR`.
- **Error and log text.** Messages that named the engine as `tre` (shader
  errors, GPU debug labels, the default window title) say `tess_re` now. Code
  that matches on that text needs the new word.
- **Exception module.** `UnsupportedPlatform.__module__` is `tess_re`.

### Not changed

- The PyPI distribution name, `tesserae-engine`.
- Every public name inside the package.
- The names of the engine's own threads (`tre-gpu-wake`, `tre-timer-wake`,
  `tre-appearance`, `tre-layer`), so a tool that looks them up by name keeps
  working.
- Earlier upgrade pages (0.3.x to 0.5.x) still say `tre` and `TRE_*`: that was
  right for those releases.

### Check your project

```sh
grep -rnE '\btre\b|TRE_' --include='*.py' --include='*.toml' --include='*.yml' --include='*.sh' .
```

## 0.6.1

### Changed

- **A callback that queues itself stops with the loop (#260).** Once `App.run()` has decided to end (the last
  window closed, or `LoopHandle.stop()`), queued `LoopHandle.call_soon` callbacks are no longer run, so one
  that queues itself again cannot keep `run()` from returning. On macOS (0.5.9 to 0.6.0) such a callback kept
  `run()` from returning after the last window closed. A callback that ran once more after the last window's
  `closed` event now does not. A loop with `keep_alive=True` and no window still runs them.
