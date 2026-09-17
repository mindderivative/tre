# Plan: M10 Phase 1 — Real Overlay Dismissal (§11.3)

Corresponds to `BUILD_TRACKER.md` M10 Phase 1's own scoping:
`OverlayMeta.dismiss_on_outside_click`/`dismiss_on_escape` (real data
since M3 step 13) wired to `Tree::dispatch`'s real `PointerPressed`/
`KeyPressed` arms.

## Investigation before writing code

- `overlay.rs`'s own module doc comment is confirmed stale: "real
  pointer/keyboard `InputEvent`/`AppHandler` dispatch and hit-testing...
  don't exist anywhere in this codebase yet" — true at M3 step 13,
  false since M4 built all of it.
- `Tree::dispatch`'s own `Key::Escape` arm already names this exact
  gap directly, in a real comment: "No overlay-dismiss consumer exists
  yet to route this to (§11.3's own 'dismissed on outside-click or
  Escape' isn't wired) -- explicitly deferred, not silently dropped."
  This phase closes it.
- Confirmed via grep: `overlay_meta` has exactly one real reader today
  (`dispatch::open_context_menu`'s own re-open guard) — never a
  dismissal check anywhere.
- `Tree::hit_test(root, point) -> Option<NodeId>` (M5 Phase 2) is
  `hit_test_at(root, point, Affine::IDENTITY)` — treats `root`'s own
  `parent_transform` as identity. Confirmed safe to call with an
  overlay's own `content` `NodeId` directly (not the true tree root):
  `open_overlay` always attaches `content` as a direct child of the
  tree's own true root (`self.add_child(root, content)`), and every
  real root this framework constructs has `layout.location == (0, 0)`
  and `transform == Affine::IDENTITY` (`PaintProperties::new`'s own
  default, never overridden for a root) — so `hit_test(content, point)`
  correctly reports whether `point` lands anywhere in that overlay's
  whole visible subtree, the same real assumption `open_context_menu`
  already relies on implicitly.
- Real, stated design decision: dismissal is wired to `PointerPressed`
  only, not `PointerReleased` — matching real desktop/MD3 convention
  (a press outside a menu closes it immediately, not on release).
  Dismissing an overlay this way *consumes* that press (skips the
  normal hit/ripple registration for it) rather than also clicking
  through to whatever's now exposed underneath — matching Android's
  own real "outside touch dismisses, doesn't pass through" convention.
- `Escape` dismisses every currently-open overlay whose own `OverlayMeta
  .dismiss_on_escape` is true (in practice almost always at most one,
  since this codebase has no real nested-overlay stacking concept
  today) — the plain "simplest thing that could work" v1 scope, not
  a real stacking/topmost-only model this codebase has no other need
  for yet.
- `self.overlays: HashMap<NodeId, OverlayMeta>` is a private `Tree`
  field, directly iterable from within `tree.rs` — collecting matching
  IDs into a `Vec` first (ending the immutable borrow `hit_test` needs)
  before calling `self.close_overlay` (which needs `&mut self`) avoids
  any borrow conflict.

## Real finding during implementation (not anticipated above)

`Tree::close_overlay`'s own original implementation (M3 step 13) used
`Tree::remove` — full, irreversible destruction of the overlay's whole
subtree. Nothing in `engine-py` ever called `close_overlay` before this
phase (confirmed via grep), so this was never actually exercised by a
live app. The moment real dismissal made `close_overlay` reachable for
the first time, the single most realistic use case broke: right-click
a context menu open, dismiss it, right-click the *same* anchor again.
`Node.set_context_menu` registers one specific, app-owned content
`NodeId` meant to be reopened repeatedly — destroying it on the very
first dismissal left `dispatch::open_context_menu`'s own stored
`content` id dangling, panicking the next real reopen attempt
(`open_overlay`'s own `self.get(content).expect(...)`). Fixed by
changing `close_overlay` to detach (not destroy) — the identical
contract `Node.set_context_menu` already committed to for exactly this
"alive, parentless, ready for `add_child` elsewhere later" reason.
Found and fixed before this phase shipped, via the same real, "does
this actually work end to end" pytest-level verification this project
holds every phase to — not left as a follow-up.

## Design

- New private `Tree::dismiss_overlays_outside(&mut self, point: Point)
  -> bool`: collects every open overlay's own `content` `NodeId` where
  `meta.dismiss_on_outside_click` is true and `self.hit_test(content,
  point).is_none()`, closes each via the already-real `close_overlay`,
  returns whether anything was actually dismissed.
- New private `Tree::dismiss_escapable_overlays(&mut self)`: closes
  every open overlay whose own `meta.dismiss_on_escape` is true,
  unconditionally (Escape always means "close it," no position check
  needed).
- `Tree::dispatch`'s `PointerPressed` arm calls `dismiss_overlays_
  outside` first; if it dismissed anything, sets `self.pressed = None`
  and returns `DispatchOutcome::None` immediately, skipping the
  existing hit/ripple logic for that same press.
- `Tree::dispatch`'s `Key::Escape` arm calls `dismiss_escapable_
  overlays` instead of being a no-op; still returns `DispatchOutcome::
  None` (a mechanical consequence, the same shape ripple-spawn/hover-
  update already use — no new outcome variant).
- `overlay.rs`'s own module doc comment corrected to state real
  dispatch has existed since M4, not that it's still missing.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt. New tests: a real
  press outside an open `dismiss_on_outside_click` overlay closes it
  (the overlay's own subtree is genuinely gone from the tree, not just
  hidden); a real press *inside* that same overlay leaves it open and
  still processes the press normally (hit/ripple registration, a real
  regression check that dismissal doesn't wrongly consume in-bounds
  presses); an overlay with `dismiss_on_outside_click: false` stays
  open regardless of where the press lands; a real `Key::Escape`
  dispatch closes every `dismiss_on_escape` overlay open at once; an
  overlay with `dismiss_on_escape: false` survives a real `Key::Escape`
  dispatch.
- `maturin develop --release` + `pytest tests/` + all examples — this
  phase touches no Python-facing API surface directly (dismissal is a
  new, automatic real-dispatch-only mechanical consequence, the same
  as hover/ripple), so this is a pure regression check. `examples/
  context_menu.py` already opens a real context menu via `open_overlay`
  — worth a direct check that it still runs cleanly with dismissal now
  wired in (it never dispatches an outside click/Escape itself today,
  so this is confirming a true no-op for existing behavior, not new
  coverage).
