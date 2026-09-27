# PLAN — Branch `0.3.5`: Milestone 100, Apply the Naming Convention

*(Replaces the M99 plan — M99 is complete. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Every surviving public name takes its M93 target form, so the Python API is
exactly the target API: `create`/`set`/`get`/`on`/`simulate`, no parallel
legacy spellings. Tesserae confirmed (M97) it uses none of the old names.

## Steps

**Phase 1 — Renames**
1. Renames. New: `Window.read_clipboard()`/`write_clipboard(text)`, and
   `CanvasContext` becomes `Painter`. Removed, their replacements already
   existing: the `add_*` factories (`create`), `get_monospace_cell_size`
   (`measure_text`), `resize_terminal`/`copy_terminal_selection`
   (`terminal.set(cols=, rows=)`/`get("selection")`),
   `set_virtual_list_window`, `redraw_canvas` (`canvas.redraw()`); on `Node`,
   `set_layout`/`set_text`/`set_clip_children`/`set_syntax_spans`/
   `set_folded_ranges`/`set_terminal_selection`/`push_frame` (`set`),
   `get_text`/`is_focused` (`get`), the six `set_on_*` (`on`); `Event.kind`/
   `node`. Tests and examples migrate by a codemod plus hand fixes where
   semantics differ (`on("click")` bubbles; `set(text=)` fires no `change`;
   a canvas draws on creation; virtual lists build their own rows).
2. The headless-testing surface (D9): the 12 synthetic-input methods and the
   three `*_system_clipboard` methods go. Terminal key and Ctrl-byte routing,
   the terminal wheel, and the Ctrl+C/X/V/A text-input shortcuts move into
   `dispatch::process_input`, so `simulate` drives them exactly as the live
   loop does. Clipboard tests use `read_clipboard`/`write_clipboard` and skip
   where no OS clipboard is reachable.
3. The migration table becomes a published page; `_removed.py` and its tests
   follow any rename beyond `279e640`, and Tesserae gets the updated file;
   full standing chain.

Each step: tests, examples, and docs follow; the tracker, a local commit,
memory.

## Status

Step 1 done (2026-09-26). Step 2, the headless-testing surface, next.
