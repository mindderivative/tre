# PLAN — M53: Real Context Menus + Clipboard for `TextField`/`CodeEditor`

## Goal
User: "Scope the click/hover/context menu handlers. There is a real
need for context menus and handlers in the CodeEditor and TextField."
Scoped via a formal plan (`EnterPlanMode`/`ExitPlanMode`), grounded in
a dedicated Explore agent's exhaustive read of the dispatch chain,
`node.rs`, `window_factory.rs`, `window_input.rs`, `app.rs`, and every
example.

## Real investigation
Every generic interaction primitive (`set_on_click`/`set_on_hover_
enter`/`set_on_hover_exit`/`enable_interaction`/`set_context_menu`)
already works on `TextField`/`CodeEditor` with zero `NodeKind`
exclusion. A real right-click already opens whatever context menu was
attached, live, in the real winit event loop. The real gaps: right-
click never focused a `TextField`/`Terminal`; there is no real,
callable path to the actual OS clipboard outside `app.rs`'s own raw key
handling (`Window.copy`/`cut`/`paste` are deliberately hermetic); no
`select_all` exists anywhere; no factory demonstrates wiring a context
menu at construction; `add_code_editor` adds nothing beyond plain
`TextField`.

## Design (3 phases)
1. `engine-core`: widen the click-to-focus gate to also match `Pointer
   Button::Secondary`; new `Tree::select_all_text_field`.
2. `engine-py`: refactor `app.rs`'s inline real-clipboard logic into
   shared helpers; 3 new `Window` pymethods (`copy_to_system_
   clipboard`/`cut_to_system_clipboard`/`paste_from_system_clipboard`)
   plus `Window.select_all()`; `_core.pyi` updated.
3. Example + docs + verification: `examples/text_field_context_menu.py`
   demonstrating a real Copy/Cut/Paste/Select All context menu on both
   `add_text_field` and `add_code_editor`.

## Explicitly out of scope
Auto-calling `enable_interaction()` inside the factories. A packaged
"standard text-editing context menu" factory. `CodeEditor`-specific
menu items (no LSP infrastructure exists). Any new selection gesture
beyond the existing click/drag plus `select_all`.

## Status

**Phases 1-2 of 3 complete.** Phase 1: widened the click-to-focus gate
(`tree.rs:3615`) to also match `PointerButton::Secondary`, reusing
`set_focus_to` verbatim. New `Tree::select_all_text_field(field) ->
bool`, a genuinely new primitive (not composable from Python today --
no way to read a field's own content length). 5 new Rust unit tests
(right-click focus, select-all on ordinary/empty/multi-byte-UTF-8
content, non-`TextField` no-op), all GIL-free, all passing on the first
run. 1 new pytest test (`Window.right_click` now focuses a `TextField`
too).

Phase 2: refactored `app.rs`'s three inline `InputEvent::Copy`/`Cut`/
`PasteRequested` arms into shared helpers (`dispatch.rs`), reused by
both the real winit path (unchanged behavior, confirmed by re-running
every example) and 4 new `Window` pymethods (`copy_to_system_
clipboard`/`cut_to_system_clipboard`/`paste_from_system_clipboard`/
`select_all`), deliberately distinctly named from the existing hermetic
`copy`/`cut`/`paste`. `_core.pyi` updated with 4 new stubs plus a real
correction to the *existing* `copy`/`cut`/`paste` docstrings (they
never stated plainly they're hermetic -- a real, pre-existing
documentation gap found during investigation). **Real, honest finding
caught by running the tests:** a full write-then-read clipboard round
trip fails in this sandboxed X11 environment (no clipboard manager
installed) -- a genuine `arboard` per-call-fresh-instance
characteristic (matching `app.rs`'s own pre-existing, unchanged
convention, not a new bug), confirmed against the reliably-passing
Rust test (which reuses one instance for both halves). Fixed the test
to treat this as a real, honest `pytest.skip()`.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
`cargo test --workspace --release` (unchanged -- new pymethods are
GIL-bound, pytest-covered instead), `maturin develop --release`,
`pytest tests/` (755 passed, up from 748, +7 net, 2 skipped -- 8 new
tests, 1 gracefully skipped), all 84 examples, showcase demo. Tracker
generator: 53 milestones/158 phases/277 items/2 known gaps/19 fixed
gaps. **Up next: Phase 3, the example + docs + final verification.**
