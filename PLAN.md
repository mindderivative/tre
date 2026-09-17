# Plan: M13 Phase 1 — Real AppShell Composition (§11.2)

Corresponds to `BUILD_TRACKER.md` M13 Phase 1's own scoping: a real
`Window`-level way to build the shell's named regions (menu bar,
toolbar, status bar, and the stable `content` swap region) in one
call, as ordinary flex-composed containers, no new `engine-core`
primitive.

## Investigation before writing code

- ARCHITECTURE.md §11.2's own struct sketch: `AppShell { menu_bar:
  Option<NodeId>, toolbar: Option<NodeId>, dock: DockLayout, status_
  bar: Option<NodeId>, content: NodeId }` — a passive bookkeeping
  record naming which of a window's own already-built nodes serve
  which chrome role, not a builder of their *content*. `content` is
  the one mandatory field, "the swappable region."
- `PyWindow::new` (`window.rs:213-222`, confirmed by direct read):
  every node any `add_*` method creates attaches directly to `self.
  root`, whose own `Style` is hardcoded `Display::Flex` +
  `FlexDirection::Row`. A real header/content/footer shell needs a
  *column* arrangement — reusing `self.root` directly isn't possible
  without breaking every other `add_*` method's own implicit row flow,
  so shell composition needs its own dedicated child container.
- `Node.add_child` (`node.rs:323-332`) already exists and already
  detaches a node from its current parent first (`Tree::try_add_child`
  — confirmed via direct read) — the exact "re-parent an already-built
  node into the shell" mechanism this phase needs, with zero new
  `engine-core` work.
- `taffy::Style` (pinned 0.14.0, confirmed via direct read of its own
  source) has a real `flex_grow: f32` field — the standard flexbox
  mechanism for "this region fills whatever space remains" the
  `content` region needs, so it isn't squeezed to zero by the chrome
  regions' own explicit sizes.
- No new `engine-core` primitive is needed anywhere in this design —
  confirmed by investigation: `Tree::insert`/`add_child` (already
  real) build the shell's own container structure; each individual
  chrome region's *content* is built by the app itself via already-real
  primitives (`add_rect`, etc.), matching AppShell's own "composition
  convenience," not content-authoring, framing.

## Design

`crates/engine-py/src/window.rs`:

- New `PyWindow.build_shell(menu_bar: Option<PyRef<'_, Node>>,
  toolbar: Option<PyRef<'_, Node>>, status_bar: Option<PyRef<'_,
  Node>>) -> PyResult<Node>`:
  - Each given region is checked with the same `Rc::ptr_eq` same-tree
    guard `set_dock_handle`/`set_drop_zone_highlight` already
    established (M10 Phase 2/3) — a foreign `Window`'s own `Node`
    raises `EngineError::ForeignNode`.
  - Creates one new "shell" `Container` child of `self.root`, sized to
    the window's own real width/height, `FlexDirection::Column` — the
    one new structural node this phase adds.
  - Re-parents each given region into the shell container, in order
    (`menu_bar`, `toolbar`, then `content`, then `status_bar`) via
    `Tree::add_child` (already handles detaching from wherever the
    node currently is).
  - Creates a new, empty `Container` node for `content`, `flex_grow:
    1.0` so it fills whatever vertical space the given chrome regions
    don't take, appended into the shell container between `toolbar`
    and `status_bar`.
  - Returns `content` — the one handle the app needs to keep for
    Phase 2's own navigation.

## Verification plan

- `cargo test --workspace --release`/clippy/fmt — this phase is
  `engine-py`-only, no `engine-core` change.
- `maturin develop --release` + `pytest tests/`. New `test_app_shell.py`
  (mirroring `test_docking.py`'s own file-per-feature convention):
  `build_shell` with all three optional regions given, real functional
  proof each region is genuinely attached and positioned (clicking a
  registered handler on each region's own real node still fires,
  proving it's really part of the live tree, the same "clicking it
  proves it's real" discipline this project's test suite consistently
  uses); `build_shell` with all three omitted still returns a real,
  usable `content` node; a foreign-`Window` region raises the same
  `ForeignNode` `ValueError` every other same-tree guard already does.
- Run all examples — confirm clean exit; a new `examples/app_shell.py`
  demonstrating a real shell with menu bar/toolbar/status bar and an
  initial `content` screen, matching this project's own established
  pattern of pairing a new capability with a real, visible
  demonstration.
