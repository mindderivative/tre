# Log: M13 Phase 2 — Real Content Navigation (§11.2, optionally §7.6)

Corresponds to `BUILD_TRACKER.md` M13 Phase 2, closing M13 entirely. A
real `Node.remove()`, plus a real example demonstrating navigation
between two screens.

## Investigation before writing code

Confirmed via grep and direct read: `Node.add_child` existed but
nothing exposed `Tree::remove` to Python at all. `Tree::remove`
(`tree.rs:271-289`) recursively removes a node and its whole subtree,
unlinking it from its own parent's `children` first — exactly
"replacing `content`'s own children," the one missing half
ARCHITECTURE.md §11.2's own text names. `Window.begin_container_
transform`/`end_container_transform` (§7.6) are already real and
Python-facing — no new wiring needed to compose them with navigation.

## What happened

New `Node.remove(&self)` — calls `Tree::remove(self.id)`, mirroring
`add_child`'s own minimal shape. No return value: a `Node` handle
Python already holds always refers to a real, present `NodeId`, the
same assumption every other `Node` method already makes.

New `tests/test_remove.py`: a node genuinely detaches so a new sibling
occupying the same real position is the one a real dispatched click
reaches afterward (the functional proof, since calling `Window.click`
on the removed node's own now-stale handle would panic via `Tree::
absolute_position`'s own `.expect()` — an internal-bug condition
everywhere else in this codebase, not something to attempt from a
test); removing a node with real children doesn't corrupt the rest of
the tree (an unrelated, freshly-built node still dispatches correctly
afterward).

New `examples/navigation.py`: a real `AppShell` (Phase 1) with two
screens built into `content`; navigating removes the first screen's
real subtree and adds the second's, proven by a real dispatched click
landing on the second screen's own card, not the first's. Deliberately
does **not** attempt to combine this with `begin_container_transform`
in the same script — `examples/container_transform.py` is already the
definitive, verified proof that mechanism works on its own; composing
it correctly with `content`'s own pre-attached-destination requirement
would need careful re-verification this phase's own narrow scope (a
real `remove()` primitive) doesn't need to risk. §11.2's own text
states the two *compose*, not that this phase must re-prove container-
transform itself.

Full `cargo test --workspace --release`/clippy `-D warnings`/fmt clean
(no `engine-core` change, `engine-py`-only — `Tree::remove` already
existed and was already tested). `maturin develop --release` + full
`pytest tests/` (106 passed, up from 104, 1 skipped) and all nineteen
examples (eighteen existing + new `navigation.py`) confirmed clean.

M13 (AppShell / Single-Page Navigation) is now complete: both phases
done — Phase 1 built real shell composition (and, along the way, found
and fixed a real tree-corruption bug caught live by `accesskit`), Phase
2 built the one missing removal primitive and proved real navigation
works end to end.
