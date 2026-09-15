# Log: M3 Phase 4, Step 7 — Wire `accesskit` (§14 step 7)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 4, step 7 of 2 (steps 6-7) -- closes Phase 4.

## What happened

**Verified accesskit's real current API before writing anything**
(ARCHITECTURE.md's own first-hand review note warned of real breaking
changes across 0.17→0.25). `cargo add --dry-run` resolved `accesskit
0.25.0` / `accesskit_winit 0.34.0` -- exactly the upper end of the
note's own range. Confirmed directly in source: `TreeInfo` (not `Tree`,
which is now `#[deprecated] type Tree = TreeInfo`), no `app_name` field
(only `toolkit_name`/`toolkit_version`), `TreeUpdate.tree_id: TreeId`
required (`TreeId::ROOT` for the single-tree case here). One correction
to ARCHITECTURE.md §10's own prose: it names `accesskit::Action::
Default` for Enter/Space dispatch, but that variant doesn't exist in
0.25.0 -- the real equivalent is `Action::Click` ("do the equivalent of
a single click or tap"), used throughout this step instead.

**`engine-core::access`**: `AccessNodeData`/`AccessStates` matching
§10's own struct sketch, narrowed -- `AccessStates` carries only
`disabled` (no component exists yet with a `checked`/`selected`/
`expanded` field to derive from). `Node::access` added back (deferred
since step 3's own scope-narrowing note predicted exactly this).
`Tree::build_access_update` walks the real `Node` tree, converts each
`engine_core::NodeId` to `accesskit::NodeId` via `slotmap`'s own
`KeyData::as_ffi` (a stable, purpose-built u64 conversion for exactly
this "hand a key to a foreign API" case), and reads bounds from the
same taffy `Layout` `engine-render` already paints from -- the
accessible tree's geometry can't disagree with what's on screen because
it's the same data, not a parallel copy. One unit test proves this in
isolation: a `Role::Button` node with a label and a `Click` action
produces a `TreeUpdate` with the exact expected role, label, action, and
bounds.

**Real finding: `accesskit_winit`'s direct-handler API requires `Send`,
same conflict as `engine-py`'s own pyo3 finding last step.**
`Adapter::with_direct_handlers` takes `impl ActivationHandler + Send`
etc. ("each of these handlers may be called on any thread, depending on
the underlying platform adapter") -- incompatible with `Rc<RefCell<
Tree>>`. Used `Adapter::with_event_loop_proxy` instead: only a thin,
genuinely-`Send` `PlatformEvent` enum crosses threads; the actual
`Tree`-touching `build_access_update` call always happens back on the
main thread, inside winit's own `user_event`/`window_event` callbacks.

**`engine-platform::run_windowed` gains a required second closure**
(`build_access_update`), not an optional one -- every window this
framework opens reports a real accessibility tree from the start,
matching §10's "keyboard operability ships from day one" stance; a
caller with no interesting `AccessNodeData` set yet still gets a valid
(if minimal, all `Role::Unknown`) tree. Also required the window to be
created invisible, then the adapter, then shown -- `accesskit_winit`'s
own hard panic-on-violation requirement. Both existing call sites
(`engine-render/tests/rect_window.rs`, `engine-py`'s `App::run`) updated
to pass a second closure calling `Tree::build_access_update`.

**The actual "confirm one button is correctly exposed" proof**
(`crates/engine-platform/tests/access_button.rs`): a real window, one
`Role::Button` node (label "Save", one `Action::Click`), run through
the real pipeline above. Verified externally against this process's
real AT-SPI registration -- found the session has its own dedicated
AT-SPI D-Bus (`unix:path=$XDG_RUNTIME_DIR/at-spi/bus_0`, distinct from
the regular session bus; querying the wrong bus was the first, quickly
self-corrected attempt), located the test process registered there
under its own bus name (alongside real running apps -- Chrome, dolphin,
etc.), and walked the tree from the AT-SPI registry root down to the
button node. Confirmed directly: `Name` = `"Save"`, `GetRole` = `43`
(`atspi-common`'s own real `u32`-to-`Role` conversion table maps
`43 => Button`, checked in its source, not assumed), `org.a11y.atspi.
Action.GetActions` = `[("click", "", "")]` (exactly the one action
added), and `org.a11y.atspi.Component.GetExtents` = `(0, 0, 120, 40)` --
an exact match for the node's real taffy-computed bounds. This is the
identical tree a real screen reader (Orca) reads from; a full manual
Orca session wasn't run, but the AT-SPI layer it consumes was checked
directly, not assumed to work because the code compiled.

**Windows/macOS CI added**, per ARCHITECTURE.md §6's own "Decision
recorded" naming this step as the trigger. Deliberately narrower than
the Linux job: `cargo build --workspace --all-targets` is the real,
blocking check (every platform-specific `accesskit_winit` backend at
least compiles); `cargo test --workspace` runs but is
`continue-on-error: true` -- this session has no way to verify from a
Linux sandbox whether GitHub's Windows/macOS hosted runners have a
working software GPU adapter the way `mesa-vulkan-drivers` was directly
confirmed to provide on Linux.

## Verification

```
$ cargo test --workspace     # all green, including the new
                              # engine-core unit test and the real
                              # windowed access_button.rs
$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check          # clean

$ ./target/debug/deps/access_button-<hash> &
$ gdbus call --address unix:path=/run/user/1000/at-spi/bus_0 \
    --dest org.a11y.atspi.Registry \
    --object-path /org/a11y/atspi/accessible/root \
    --method org.a11y.atspi.Accessible.GetChildren
# -> found this process's own bus name among real registered apps
$ gdbus call ... --method org.a11y.atspi.Accessible.GetRole
(uint32 43,)
$ gdbus call ... --method org.a11y.atspi.Action.GetActions
([('click', '', '')],)
$ gdbus call ... --method org.a11y.atspi.Component.GetExtents 0
((0, 0, 120, 40),)
$ gdbus call ... Properties.Get org.a11y.atspi.Accessible Name
(<'Save'>,)

$ maturin develop && pytest tests/ -q && python examples/animate_rect.py
6 passed; animate_rect.py: exited cleanly after 60 frames
```

## Next

`BUILD_TRACKER.md` updated: Phase 4 (both steps) done, M3 to 57% (4 of 7
phases). Next: M3 Phase 5 (§14 step 8) -- MD3 shadow spike via
`fill_blurred_rounded_rect`, standalone, against the exact pinned Vello
version (§7.2 risk). First step to touch `engine-md3` for real.
