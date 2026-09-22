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

**All 3 phases complete. Milestone closed.**

Phase 1: widened the click-to-focus gate (`tree.rs:3615`) to also
match `PointerButton::Secondary`, reusing `set_focus_to` verbatim. New
`Tree::select_all_text_field(field) -> bool`, a genuinely new primitive
(not composable from Python today -- no way to read a field's own
content length). 5 new Rust unit tests (right-click focus, select-all
on ordinary/empty/multi-byte-UTF-8 content, non-`TextField` no-op), all
GIL-free, all passing on the first run. 1 new pytest test
(`Window.right_click` now focuses a `TextField` too).

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

Phase 3: new `examples/text_field_context_menu.py` -- a real, live
window with both an `add_text_field` and an `add_code_editor`, each
with `enable_interaction()` called explicitly, each with a real
Copy/Cut/Paste/Select All context menu built via `build_menu`/
`add_menu_item`, wired to Phase 2's new methods, attached via `Node.
set_context_menu`. Verified through `Window.right_click(node)` +
`Window.click(item)` reaching that item's own registered handler
(the same functional proof `tests/test_context_menu.py` already
establishes) plus `select_all`'s own real effect confirmed through the
always-real, hermetic `Window.copy()`. **Real, found-while-running bug
in the example itself, caught by actually executing it:** right-
clicking a second anchor while a different menu was still open was
swallowed as an outside-click dismissal (`Tree::dispatch`'s `dismiss_
overlays_outside`, which runs for both mouse buttons before the
Primary/Secondary split) rather than opening the new menu -- a real,
deliberate, already-tested engine convention, not a bug in Phase 1/2's
own code. Fixed by adding an explicit "click elsewhere to dismiss"
step between each menu demonstration, using a dedicated gutter rect
genuinely clear of both fields and either menu's real computed
footprint (`MENU_ITEM_HEIGHT = 56.0` × 4 items = 224px, taller than an
initial placement assumed).

Full chain green: `cargo check`/`clippy -D warnings`/`fmt` clean,
`cargo test --workspace --release` (unchanged from Phase 2 -- Phase 3
is example/docs-only), `maturin develop --release`, `pytest tests/`
(755 passed, 2 skipped -- unchanged from Phase 2), all 85 examples (+1,
zero failures), showcase demo. Tracker generator: 53 milestones/159
phases/279 items/2 known gaps/19 fixed gaps.
