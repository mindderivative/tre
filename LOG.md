# Log: M13 Phase 1 — Real AppShell Composition (§11.2)

Corresponds to `BUILD_TRACKER.md` M13 Phase 1. A real `Window`-level
way to build the shell's named regions (menu bar, toolbar, status bar,
and the stable `content` swap region) in one call, no new `engine-core`
primitive.

## Investigation before writing code

ARCHITECTURE.md §11.2's own struct sketch: `AppShell` is a passive
bookkeeping record naming which of a window's own already-built nodes
serve which chrome role, not a builder of their content. `PyWindow::
new`'s own root is hardcoded `Flex Row` -- every other `add_*` method's
own implicit flow depends on that, so shell composition needs its own
dedicated child container, `Flex Column`. `taffy::Style` has a real
`flex_grow: f32` field for "this region fills whatever space remains."
No new `engine-core` primitive was needed anywhere -- confirmed by
investigation.

## What happened

New `PyWindow.build_shell(menu_bar=None, toolbar=None, status_bar=
None) -> Node`: checks each given region with the same `Rc::ptr_eq`
same-tree guard `set_dock_handle`/`set_drop_zone_highlight` already
established; creates one new `Flex Column` `Container` child of `self.
root`; re-parents each given region into it in order; creates a new,
empty `content` `Container` with `flex_grow: 1.0`; returns `content`.

**Real finding, caught live, not by pytest:** the first draft
re-parented `menu_bar`/`toolbar`/`status_bar` using the cheap `Tree::
add_child` (the same primitive `add_rect` itself uses for a
freshly-inserted node) -- but every `add_*` method already attaches its
result to `self.root` immediately, so these nodes always already have a
real parent by the time `build_shell` runs. `add_child` doesn't detach
first (confirmed via direct read of its own doc comment: only correct
for "a freshly-inserted node, or a reparent already proven disjoint"),
so this left a node listed as a child of *both* its old parent and the
new shell -- real tree corruption. Not caught by any pytest test (none
render a real frame); caught by `examples/app_shell.py`'s own live run,
which panicked with `TreeUpdate includes duplicate child` from
`accesskit`'s own validation -- the exact same "caught by the live
example, not pytest" pattern `dock_panel`'s own doc comment already
names for an analogous M4 Phase 9 bug. Fixed by using `Tree::try_add_
child` (which detaches first, the same mechanism `Node.add_child`'s own
pyo3 wrapper already uses) for the three re-parenting calls; the
freshly-inserted `shell`/`content` nodes still correctly use the cheap
`add_child`, since they genuinely have no prior parent.

New `tests/test_app_shell.py`: all three regions given, real functional
proof each is genuinely attached (clicking it fires its own handler);
no regions given still returns a real, usable `content`; `content`'s
own `flex_grow` genuinely fills remaining space after a fixed-height
menu bar (a child added to `content` is still real and clickable, not
squeezed to zero height); a foreign-`Window` region raises the same
`ForeignNode` error every other same-tree guard already does. New
`examples/app_shell.py`: a real, live four-region shell.

Full `cargo test --workspace --release`/clippy `-D warnings`/fmt clean
(no `engine-core` change, `engine-py`-only). `maturin develop --release`
+ full `pytest tests/` (104 passed, up from 100, 1 skipped) and all
eighteen examples (seventeen existing + new `app_shell.py`) confirmed
clean, including the real bug fix verified live.
