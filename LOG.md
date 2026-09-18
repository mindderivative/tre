# Log: M30 Phase 2 Step 4 — Menu (closes Phase 2)

## Real, deliberate reuse, not a second overlay mechanism

`Tree::open_overlay`/`close_overlay` (§11.3, M3 step 13) already
exist for exactly this: `overlay.rs`'s own module doc comment states
"menu bars, dropdown menus, context menus, tooltips, and MD3 dialogs
are all the same missing primitive." `open_menu` calls the identical
primitive `dispatch::open_context_menu`'s own right-click path
already uses (`open_overlay` with `dismiss_on_outside_click: true,
dismiss_on_escape: true`), just exposed as a direct Python-callable
method rather than gated behind synthetic secondary-button dispatch —
a real dropdown menu opens on a plain click (or any app-chosen
trigger), not a right-click. The existing context-menu mechanism is
completely untouched — confirmed by `test_context_menu.py` passing
unmodified, not just by not editing its file.

## Real MD3 data, verified before writing any code

A direct fetch for `_md-comp-menu-item.scss` 404s — a real, confirmed
finding, not an oversight: Material Web has no dedicated menu-item
token file at all. A real MD3 menu genuinely reuses the plain List
Item's own tokens for its rows (`_md-comp-list.scss`: 56dp height,
24dp icon, 16dp leading space, `on_surface` label, `on_surface_
variant` icon) — confirmed, not assumed consistent. The panel itself
has its own real tokens (`_md-comp-menu.scss`): `surface_container`
fill, `corner-extra-small` (4dp), real rest-state elevation (level 2).

## Real re-parenting, mirroring `close_overlay`'s own established pattern

`build_menu` moves each item — `Tree::detach` then `add_child` — into
the returned panel, which is itself returned genuinely unattached
anywhere. `open_menu`'s own `open_overlay` call is what actually
attaches it. This is the identical "detach, not destroy, ready for
later `add_child`" contract `close_overlay`'s own doc comment already
established for context-menu content, reused here for the same real
reason rather than invented fresh.

## A real finding, caught by a failing test, not predicted in advance

`tests/test_menu.py`'s first draft of its click-dispatch test built a
menu item, called `build_menu`, and immediately tried `window.click`
on it — and it failed. Traced the real cause: the item's own subtree
was re-parented under a panel that was itself never attached to the
window's real root, so it was never part of any `compute_layout` pass
— its hit-test box was stale/uncomputed, and the click missed. This
is correct, not a bug: a menu item genuinely isn't on-screen, and so
genuinely isn't clickable, until its menu is actually open. Fixed the
test's own premise (call `open_menu` first), not the implementation —
confirmed this really is the right behavior before "fixing" anything,
the same discipline that kept the earlier `Tree::hit_test_at` finding
(Phase 1) from being dismissed as a test bug instead of a real one.

## Verification

`cargo check --workspace --all-targets`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check` — all clean. `cargo
test --workspace --release`: 42 binaries, all green, unchanged (pure
composition plus existing overlay primitives, no new engine-render
capability). `maturin develop --release` rebuilt. `pytest tests/`:
273 passed, 1 skipped (10 new in `test_menu.py`, zero regressions,
`test_context_menu.py` itself unmodified and still green). All 38
examples and the showcase demo re-run clean. `mypy --strict` clean
against `examples/menu.py`.

This closes M30 Phase 2 (Selection) entirely: `Radio Button`,
`Switch`, `Chip`, `Menu`, all with paired `.pyi` stubs.
