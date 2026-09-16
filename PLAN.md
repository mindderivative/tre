# Plan: M4 Phase 7 — Right-Click Context Menus (§11.3)

## Context

`PointerButton::Secondary` exists and is unit-tested to correctly never
start a drag, but nothing acts on it yet. §11.3's overlay mechanism
(`Tree::open_overlay`/`close_overlay`/`OverlayMeta`, built at M3 step
13) is real and already proven for one dropdown menu — the missing
piece is wiring a real right-click to it.

## Investigation before writing code

Re-read §11.3 in full, then read `overlay.rs`/`open_overlay`/
`close_overlay` end to end, then grepped `engine-py` for any existing
overlay wrapper. Found the real scope is bigger than the tracker's own
one-line summary implied:

- **No Python-facing overlay API exists at all.** `open_overlay`/
  `close_overlay`/`OverlayMeta` are only ever referenced in doc comments
  across `engine-py` — never called. This phase is the first to expose
  any of it to Python, not just the dispatch half.
- **`open_overlay`'s own doc comment requires `content` to already be a
  real `Node`, unattached** (or its `add_child` would push a duplicate
  parent/child edge, corrupting the tree — confirmed by reading
  `add_child`'s real implementation, a plain unconditional push with no
  dedup). Every existing node-creation method (`add_rect`, etc.)
  attaches immediately to the window's root. `Tree::detach` (already
  real, built for docking's own tab-switching) is the exact existing
  mechanism to un-attach a node "alive, parentless, ready for
  `add_child` elsewhere later" — reused verbatim, not reimplemented.
- **`close_overlay` is destructive** (`Tree::remove`s the whole
  subtree) — closing a menu once means it can never be reopened as the
  same content. Real, correct for M3's one-shot dropdown; this phase
  doesn't change it, and doesn't build dismissal (see below), so it
  isn't exercised here.
- **Dismissal (`dismiss_on_outside_click`/`dismiss_on_escape`) is a
  real, already-named, still-open gap** — `OverlayMeta`'s own fields
  have existed since M3 step 13 with a doc comment stating plainly
  "a future dismiss dispatch... not built yet." That dispatch now
  exists (M4 Phases 1-6), but wiring dismissal is a distinct, separate
  feature from "right-click opens a menu" — a menu that opens but can
  only be closed by a direct `close_overlay` call is a real, honest,
  narrower deliverable, not a broken one. Scope narrowed to NOT build
  dismissal this phase; named explicitly as the next real gap, not
  silently dropped.

## Approach

1. **`engine-core`**: `DispatchOutcome` gains `SecondaryActivated
   (NodeId)`. `Tree::dispatch`'s `PointerReleased` arm generalizes its
   press/release-pair match to branch on button (`Primary` →
   `Activated`, `Secondary` → `SecondaryActivated`, `Middle` → `None`)
   instead of gating the whole match on `button == Primary` — the exact
   change the existing test's own comment already predicted ("reserved
   for a future context-menu mechanism"). That test's assertion updates
   from `None` to `SecondaryActivated`, with its comment corrected.
2. **`engine-py`**: `Node.set_context_menu(content: &Node)` — detaches
   `content` if it has a parent (reusing `Tree::detach` verbatim), then
   records `(anchor_id, content_id)` in a new, plain
   `Rc<RefCell<HashMap<NodeId, NodeId>>>` (no `Py<PyAny>` involved at
   all, so no GC-traversal obligation, unlike `handlers`). New
   `dispatch::open_context_menu(tree, context_menus, root, outcome)`:
   on `SecondaryActivated(anchor)`, looks up a registered menu for
   `anchor`; if the content isn't already an open overlay (checked via
   `overlay_meta`, guarding against a double-`add_child` if the same
   spot is right-clicked again), calls `open_overlay` with
   `dismiss_on_outside_click`/`dismiss_on_escape` both `true` (the
   real, correct intent, even though nothing dispatches to it yet —
   the same "real data, inert until its own step" precedent
   `OverlayMeta`'s own fields already established) and recomputes
   layout, matching `open_overlay`'s own documented contract. Called
   alongside the existing `run_dispatch_outcome` at all three real
   dispatch call sites (`app.rs`'s `on_input` closure,
   `Window`/`View`'s `.click()`-adjacent methods). New
   `Window.right_click(node)`/`View.right_click(node)`, mirroring
   `.click()`/`.hover()`'s own no-live-window-needed pattern exactly,
   dispatching a Secondary press+release pair.
3. **Tests**: `engine-core` unit test proving a same-node Secondary
   press+release produces `SecondaryActivated`, mirroring the existing
   `dispatch_activates_only_...` test's shape. New
   `tests/test_context_menu.py`: building a real window, an anchor, and
   a menu-item content node with its own `on_click` handler,
   `set_context_menu`, `window.right_click(anchor)`, then
   `window.click(menu_item)` and asserting the menu item's own handler
   fired — the real, functional, end-to-end proof that the content
   actually became a live, laid-out, dispatchable part of the tree, not
   an inspection of internal state Python has no getter for.

## Files to touch

- `crates/engine-core/src/input.rs` — `DispatchOutcome::
  SecondaryActivated`.
- `crates/engine-core/src/tree.rs` — `dispatch`'s `PointerReleased`
  arm, the corrected existing test.
- `crates/engine-py/src/node.rs` — `set_context_menu`.
- `crates/engine-py/src/window.rs`, `view.rs` — `context_menus` field,
  `right_click`.
- `crates/engine-py/src/dispatch.rs` — `open_context_menu`.
- `crates/engine-py/src/app.rs` — wire the new call alongside
  `run_dispatch_outcome`.
- `tests/test_context_menu.py` — new.

## Verification

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo fmt --check`.
- `maturin develop && python -m pytest tests/ -v` plus all examples run.
