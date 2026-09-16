# Log: M4 Phase 7 — Right-Click Context Menus (§11.3)

Corresponds to `BUILD_TRACKER.md` M4 Phase 7. `PointerButton::Secondary`
existed and was unit-tested to never start a drag, but nothing acted
on it — this phase wires a real right-click to §11.3's already-real
overlay mechanism.

## Investigation before writing code

Re-read §11.3 in full, then read `overlay.rs`/`open_overlay`/
`close_overlay` end to end, then grepped `engine-py` for any existing
overlay wrapper. The real scope turned out bigger than the tracker's
own one-line summary:

- **No Python-facing overlay API existed at all** — `open_overlay`/
  `close_overlay`/`OverlayMeta` were only ever referenced in doc
  comments across `engine-py`, never called. This phase is the first
  to expose any of it to Python.
- **`open_overlay` requires `content` to already be unattached** —
  confirmed by reading `add_child`'s real implementation (a plain
  unconditional push, no dedup): attaching an already-attached node
  would corrupt the tree. Every existing node-creation method attaches
  immediately, so `Node.set_context_menu` needs to `Tree::detach` its
  content first — the exact existing mechanism docking's own
  tab-switching already uses, reused verbatim.
- **`close_overlay` is destructive** (`Tree::remove`s the subtree) —
  correct for M3's one-shot dropdown; unchanged and unexercised here.
- **Dismissal is a real, already-named, still-open gap** —
  `OverlayMeta`'s own fields have carried a "future dismiss
  dispatch... not built yet" comment since M3 step 13. That dispatch
  now exists (M4 Phases 1-6), but wiring dismissal is a distinct
  feature from "right-click opens a menu" — scoped out explicitly, not
  silently dropped; a menu that opens but needs a direct `close_overlay`
  call to close is a real, honest, narrower deliverable.

## What happened

`engine-core`: `DispatchOutcome` gains `SecondaryActivated(NodeId)`.
`Tree::dispatch`'s `PointerReleased` arm generalizes its press/release
match to branch on button (`Primary` → `Activated`, `Secondary` →
`SecondaryActivated`, `Middle` → `None`) instead of gating the whole
match on `button == Primary` — the exact change an existing test's own
comment had predicted ("reserved for a future context-menu mechanism").
That test's Secondary case now asserts `SecondaryActivated`, with a new
Middle-button case preserving the original "no real meaning yet" claim
for the one button that still has none.

`engine-py`: `Node.set_context_menu(content)` — detaches `content` if
it has a parent (`Tree::detach`, reused verbatim), then records
`(anchor_id, content_id)` in a new, plain `Rc<RefCell<HashMap<NodeId,
NodeId>>>` shared field on `Node`/`PyWindow`/`View` (no `Py<PyAny>`
involved at all, so — unlike `handlers` — no GC-traversal obligation).
New `dispatch::open_context_menu(tree, context_menus, root, outcome)`:
on `SecondaryActivated(anchor)`, looks up a registered menu; if the
content isn't already an open overlay (checked via `overlay_meta`,
guarding against a double-`add_child` if the same anchor is
right-clicked again), calls `open_overlay` with both `dismiss_on_*`
flags `true` (the real, correct intent, inert until a future phase
wires dismissal — the same "real data, inert until its own step"
precedent `OverlayMeta`'s own fields already established). Called
alongside the existing `run_dispatch_outcome` at all three real
dispatch call sites. New `Window.right_click(node)`/`View.right_click
(node)`, mirroring `.click()`/`.hover()`'s own no-live-window-needed
pattern exactly.

## Verification

Extended the existing `dispatch_activates_only_...` `engine-core` test:
a same-node Secondary press/release now asserts `SecondaryActivated`
(previously `None`), plus a new Middle-button case preserving the
"still no real meaning" claim. 4 new pytest tests in
`test_context_menu.py`: right-clicking an anchor opens its registered
menu (proven functionally — clicking the now-live menu item fires its
own handler, not an inspection of internal state Python has no getter
for); an anchor with no registered menu is a safe no-op; right-clicking
the same anchor twice doesn't crash or duplicate the child; a plain
left-click never opens a context menu. All passed on the first run.
New `examples/context_menu.py`, matching `resizable_panes.py`/
`ripple_button.py`'s own stated honesty about what's automatable.

```
$ cargo build --workspace                                    # clean
$ cargo clippy --workspace --all-targets -- -D warnings       # clean
$ cargo fmt --check                                           # clean
$ cargo test --workspace                                      # all green, existing test's assertions updated

$ maturin develop
$ python -m pytest tests/ -v
53 passed, 1 skipped (the TRE_RUN_BENCHMARK-gated benchmark)

$ python examples/animate_rect.py       # exited cleanly, unaffected
$ python examples/resizable_panes.py    # exited cleanly, unaffected
$ python examples/ripple_button.py      # exited cleanly, unaffected
$ python examples/two_windows.py        # exited cleanly, unaffected
$ python examples/context_menu.py       # exited cleanly, new
```

## Next

M4 Phase 8: scroll-wheel input plumbing (§11.7/§11.8 groundwork). Real,
stated-not-silent gaps unchanged: context-menu dismissal
(`dismiss_on_outside_click`/`dismiss_on_escape`) is real data, still not
wired to any dispatch; a right-click-and-drag gesture isn't
distinguished from a plain right-click (mirrors the existing primary
click/drag distinction being absent for Secondary too, since nothing
needs it yet); reopening a *closed* (not just already-open) menu isn't
exercised, since `close_overlay` destroys its content and this phase
never calls it.
