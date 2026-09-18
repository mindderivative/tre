# Plan: M27 Phase 4 — Data & Layout Screen

Corresponds to `BUILD_TRACKER.md` M27 Phase 4. Written retroactively
alongside implementation — see `LOG.md` and `BUILD_TRACKER.md`'s own
Phase 4 entry for the complete real investigation, findings, and
verification record.

## What changed

- `demo/showcase.py`'s new "data" screen: a real 5,000-row virtualized
  list with real paging, a real minimal docking layout exercised via
  the same headless drag-and-drop `tests/test_docking.py` already
  establishes, and a declarative `View` panel (`demo/data_panel.yaml`
  + `demo/data_panel_sheet.yaml`) using M26's real stylesheet/token
  support.
- Real cross-path data flow: a `View`-dispatched click updates a
  `Signal`, read back and shown on an ordinary `Window` label — the
  honest, buildable interpretation of "embedded alongside" given
  `View`'s real, headless-only architecture.
- Fixed the zero-argument-handler bug (found and fixed 3 instances of
  in Phase 1) in a 4th and 5th place a narrower grep pattern missed:
  two docs pages *and* the actually-shipped `python/tre/__init__.py`
  docstring.
- Real finding: this engine's hit-testing does not bubble from a hit
  child to its parent's own click handler — fixed the nav rail's own
  new labels and three of this screen's own trigger-button labels,
  which all silently absorbed clicks by sitting on top of their
  button's own geometric center.
- `Window.add_virtual_list` takes no `x`/`y` (unlike every other
  `add_*` method) — fixed by wrapping it in a positioned container.
- Extended `Node.set_text`/`get_text` to also handle plain `Text`
  labels (previously `TextField`-only) — the real, concrete need this
  screen's own dynamic counter readout surfaced.

See `LOG.md` for the full narrative and verification results.
