# PLAN — Branch `0.3.5`: Milestone 102, Docs, Examples, and Tests Rewrite

*(Replaces the M101 plan — M101 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Docs, examples, and tests that describe the engine `tre` now is — the
building blocks — rather than the one it was. Nothing here changes behavior.

## Decisions

- **The guide is one page per building block**, plus a walkthrough that
  builds one complete widget from them. The four old guide pages go.
- **The API reference says what is**, not what changed: the "new in 0.3.4" and
  "0.3.5 removed" notes move out, since the migration pages hold that history.
  The pages stay where they are, so links keep working.
- **Docstrings count as docs**: `_core.pyi` and the Rust doc comments that
  still name removed things (`show_view`, `add_*` factories, MD3, themes) are
  corrected in the same pass.
- **The design pages stay** under Design: they're the reference Tesserae ported
  the widgets from.
- **The showcase is rebuilt** from primitives as `examples/showcase.py`, as the
  user decided at M99: the proof widgets together in one window.

## Steps

**Phase 1 — Docs**
1. The MkDocs site: guide pages for nodes and layout, painting, animation,
   events and input, text, accessibility, layers, threading, and docking; a
   "Building a widget" walkthrough; the overview, getting started, and API
   reference brought to the current API; `_core.pyi` docstrings corrected.
2. `README.md`, `ARCHITECTURE.md`, and `docs/architecture.md` rewritten to
   match; stale Rust doc comments corrected; `mkdocs build --strict` and the
   API audit clean.

**Phase 2 — Examples and tests**
1. Examples: one per building block, the proof widgets (slider, switch, ripple,
   keyed reorder, menu layer), and the showcase; each runs headless under
   `timeout 60`.
2. Tests: remove or rewrite tests of removed features and tests named for old
   milestones; confirm every public name in `_core.pyi` is exercised by a test.

Each step: the full standing chain, the tracker, a local commit, memory.

## Status

**Started (2026-09-27).** Phase 1 Step 2 next.
