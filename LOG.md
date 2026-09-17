# Log: M10 Phase 1 — Real Overlay Dismissal (§11.3)

Corresponds to `BUILD_TRACKER.md` M10 Phase 1.
`OverlayMeta.dismiss_on_outside_click`/`dismiss_on_escape` (real data
since M3 step 13) wired to `Tree::dispatch`'s real `PointerPressed`/
`KeyPressed` arms.

## Investigation before writing code

- `overlay.rs`'s own module doc comment was confirmed stale: claimed
  real dispatch/hit-testing "don't exist anywhere in this codebase yet"
  — true at M3 step 13, false since M4/M5.
- `Tree::dispatch`'s own `Key::Escape` arm already named this exact gap
  in a real comment. Confirmed via grep: `overlay_meta` had exactly one
  real reader before this phase (`dispatch::open_context_menu`'s own
  re-open guard), never a dismissal check.
- `Tree::hit_test(root, point)` treats `root`'s own `parent_transform`
  as identity — confirmed safe to call with an overlay's own `content`
  `NodeId` directly, since `open_overlay` always attaches `content` as
  a direct child of the tree's own true root, which always has identity
  location/transform.

## Real finding during implementation (not anticipated during scoping)

`Tree::close_overlay`'s own original implementation used `Tree::remove`
— full, irreversible destruction. Nothing in `engine-py` ever called it
before this phase (confirmed via grep) — real dismissal was the first
live caller. The moment it became reachable, the single most realistic
use case broke: right-click a menu open, dismiss it, right-click the
*same* anchor again — `Node.set_context_menu` registers one specific,
reusable content `NodeId`; destroying it on first dismissal left
`open_context_menu`'s own stored id dangling, panicking the next
reopen (`open_overlay`'s own `self.get(content).expect(...)`). Caught
via this project's own real pytest-level verification (`test_context_
menu.py`'s existing "right-click twice" test started panicking), not
left as a follow-up: `close_overlay` now detaches, not destroys — the
identical contract `Node.set_context_menu` already committed to.

Also found: `dismiss_overlays_outside`'s first draft treated *any*
press outside the overlay's own bounds as dismissal, including a press
back on the overlay's own anchor — which broke the "right-click the
same anchor twice" test differently (the anchor's own second press was
itself misclassified as an outside click, consuming it before
`SecondaryActivated` could fire). Fixed by also excluding the anchor's
own bounds from the outside-click check.

## What happened

New `Tree::dismiss_overlays_outside(point) -> bool` (private): closes
every open overlay with `dismiss_on_outside_click: true` whose own
content subtree *and* anchor don't contain `point`. New `Tree::
dismiss_escapable_overlays()` (private): closes every open overlay with
`dismiss_on_escape: true`, unconditionally. `Tree::dispatch`'s
`PointerPressed` arm calls the former first, consuming the press (skips
normal hit/ripple registration) if anything was dismissed; `Key::
Escape` calls the latter instead of being a no-op. `close_overlay`
changed from destroy to detach (see finding above). Two stale doc
comments corrected (`overlay.rs`'s own module doc comment; `overlay_
meta`'s own doc comment).

New `engine-core` tests: outside press dismisses; in-bounds press
leaves it open and still dispatches normally; `dismiss_on_outside_
click: false` survives an outside press; a press back on the anchor
itself never dismisses its own overlay (the real regression test); a
real `Key::Escape` dispatch dismisses every `dismiss_on_escape`
overlay; `dismiss_on_escape: false` survives Escape; a dismissed
overlay's own content can be reopened (the real motivating scenario
for the detach-not-destroy fix); `close_overlay`'s own existing test
updated to assert detachment, not destruction. New pytest tests
(`test_context_menu.py`): a real outside click dismisses a live context
menu; a real Escape press dismisses one — both proven functionally
(the menu item's own click handler no longer fires once dismissed),
the same style every other test in that file already uses.

Full `cargo test --workspace --release` clean (`engine-core` gains 8
new tests, 1 existing test updated for the real detach-not-destroy
contract change — every other prior test passed unmodified), `cargo
clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
all clean. `maturin develop --release` + full `pytest tests/` (85
passed, up from 83, 1 skipped) and all sixteen examples confirmed
clean.
