# Plan: M3 Phase 4, Step 7 — Wire `accesskit` (§14 step 7)

Corresponds to `BUILD_TRACKER.md` M3 Phase 4, step 7 of 2 (steps 6-7) -- closes Phase 4.

## Goal

Per §14 step 7: "Wire `accesskit`: confirm one button is correctly
exposed to a screen reader."

## Scope

In scope:
- `engine-core::access`: `AccessNodeData`/`AccessStates` matching §10's
  sketch (narrower: no derived states, no full focus-traversal model --
  see the module's own doc comment for why). `Node::access` field added
  back (deliberately deferred since step 3). `Tree::set_access`,
  `Tree::focused`/`set_focused`, `Tree::build_access_update(root) ->
  accesskit::TreeUpdate` -- built fresh from the current `Node` tree
  every call, bounds from the same taffy `Layout` `engine-render` paints
  from.
- `engine-platform`: real `accesskit_winit::Adapter` wiring via
  `with_event_loop_proxy` (not `with_direct_handlers` -- both handler
  APIs require `Send`, `Tree` is deliberately `!Send`). `run_windowed`
  gains a required second `build_access_update` closure -- every window
  is accessible from the start, matching §10's "keyboard operability
  ships from day one." Both existing call sites (`rect_window.rs`,
  `engine-py`'s `App::run`) updated.
- `crates/engine-platform/tests/access_button.rs`: the actual "confirm
  one button is correctly exposed" proof -- a real window, one `Role::
  Button` node with a label and a `Click` action, verified against this
  process's real AT-SPI registration on the session's dedicated AT-SPI
  D-Bus (not the regular session bus).
- Windows/macOS CI (ARCHITECTURE.md §6's own "no later than step 7"
  trigger): minimal `cargo build --workspace --all-targets`, `cargo
  test --workspace` informational (can't verify headless GPU adapter
  availability on those hosted runners from this environment).

Out of scope: the full "minimal keyboard focus model" (Tab/Shift-Tab
traversal, Enter/Space dispatch -- no interactive component exists yet
to dispatch to, §7.3's own later steps), `ActionRequested`/
`AccessibilityDeactivated` handling (no interactive dispatch wired),
states derived automatically from `NodeKind` payload fields (no
Checkbox/Tab component exists).

## Verification

`cargo test --workspace`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo fmt --check` all clean. `maturin develop` + the
pytest suite + `examples/animate_rect.py` still work end to end after
`App::run`'s signature-internal changes. `access_button.rs`'s claim
verified externally against the real AT-SPI bus, not just "it compiled" --
`GetRole`/`Name`/`GetActions`/`GetExtents` all checked directly.
