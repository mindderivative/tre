# Log: M10 Phase 3 — Drop-Zone Highlight Overlay for Docking (§11.4)

Corresponds to `BUILD_TRACKER.md` M10 Phase 3, closing M10 entirely.
`dock.rs`'s own module doc comment named this exact gap from M4 Phase
9: "Deliberately no drop-zone highlight overlay... `open_overlay`'s own
`inset` computation is hardcoded to place content *below* its anchor...
it can't cover a target zone's own bounds." This phase closes it.

## Investigation before writing code

Confirmed by direct re-read: `Tree::open_overlay` (`tree.rs`) only ever
sets `inset.left`/`inset.top` from an anchor's own absolute bounds,
leaving `right`/`bottom` `auto()` and `content`'s own size untouched --
anchor-relative-below only, no way to cover an arbitrary rect. Every
type this needed (`Size`, `length`, `auto`, `TaffyRect`, `Position`,
`Rect`) was already imported in `tree.rs`. `dock.rs`'s own doc comment
also named the real, related consequence: no `PointerMoved`-time
callback for docking existed at all before this phase, only press
(`start_drag`) and release (`end_drag_at`) -- confirmed via grep.
`PyWindow.start_panel_drag`/`drop_panel_at` are purely synthetic (no
real winit event routing into `dock.rs` anywhere), the same
no-live-window-needed proof pattern `.click()`/`.hover()`/
`.right_click()` already establish.

## What happened

**`Tree::position_overlay_over(&mut self, content: NodeId, rect: Rect)`**
(`engine-core/tree.rs`, next to `open_overlay`): sets `Position::
Absolute` with `inset.left`/`top` from `rect`'s own origin and an
explicit `style.size` matching `rect`'s own width/height -- unlike
`open_overlay`, always resizes `content` to exactly cover `rect`.
Deliberately does not attach `content` anywhere or touch `self.
overlays`: a rect-covering highlight has no anchor and no outside-
click/Escape dismissal, so `OverlayMeta` doesn't fit it; attach/detach
lifecycle is the caller's own responsibility.

`engine-py`'s `dock.rs`: `DockState` gains `highlight: Option<NodeId>`.
`set_drop_zone_highlight(dock, tree, content)` registers it -- and,
since `add_rect` (like every node-creation method) already attached
`content` to root immediately, detaches it first, the same real
"alive, parentless, ready for `add_child` elsewhere later" contract
`Node.set_context_menu` already commits to for its own registered
content, for the identical reason: a highlight must start hidden.
`drag_over(dock, tree, root, position)` is the new `PointerMoved`-
during-drag step: hit-tests `position`, reuses `enclosing_zone`
unchanged to find the enclosing registered zone (if any); if found,
computes that zone's container's real absolute rect (`absolute_
position` + `layout(..).size`) and calls `Tree::position_overlay_over`,
attaching the highlight to root if not already attached; if not found,
detaches it if attached. `end_drag_at` gains one new unconditional step
right after taking `dragging`, before any of its own existing early
returns: detach the highlight if attached -- a real drag ending, for
*any* reason, must always hide it, not just on a successful move.

`engine-py`'s `window.rs`: `PyWindow.set_drop_zone_highlight(content:
PyRef<'_, Node>) -> PyResult<()>` -- the same `Rc::ptr_eq` same-tree
guard M10 Phase 2 added to `set_dock_handle`, applied here too.
`PyWindow.drag_panel_over(x: f64, y: f64)` -- computes layout fresh
(the same reasoning `.click()`/`drop_panel_at` already state), then
calls `dock::drag_over`.

**Real finding during implementation (not anticipated in `PLAN.md`):**
the first draft of the new pytest coverage used `(200.0, 50.0)` for "a
point inside the Right zone," copied from the existing, already-
passing `test_dragging_a_registered_handle_moves_its_panel_for_real`.
Direct debugging (temporary `eprintln!` instrumentation, removed
before this phase's own verification pass) revealed that point
actually lands exactly on the Right zone's own right edge, not inside
it -- three 100px-wide root children (`left_container`, `right_
container`, `handle`) don't fit `Window`'s own 300px width once
padding/gap are subtracted, so taffy's default flex-shrink compresses
`right_container` down to 68px wide (132 to 200), and `(200.0, 50.0)`
falls just outside, resolving to the root itself, not the zone. This
means the *pre-existing* test's own "the real, functional proof" claim
is weaker than its docstring states: `window.click(panel)` firing after
`drop_panel_at(200.0, 50.0)` doesn't actually distinguish "the panel
moved to Right" from "the drop was silently a same-zone no-op," since
`panel` stays clickable either way -- a real, pre-existing gap in that
test's own discriminating power, left as-is (out of this phase's own
scope; not a regression this phase introduced). This phase's own new
tests use `(160.0, 50.0)`, confirmed via the same direct debugging to
land genuinely inside the Right zone's real, computed bounds.

New pytest coverage (`test_docking.py`): dragging over a different zone
shows the highlight, real-attached and covering it (proven the only way
available from Python -- clicking the highlight's own real computed
center fires its handler); dragging outside every zone hides it; ending
a drag (`drop_panel_at`) always hides it; `drag_panel_over` with no
drag in progress or no highlight registered is a safe no-op (matching
`drop_panel_at`'s own precedent); `set_drop_zone_highlight` rejects
content from a different `Window` (the same `Rc::ptr_eq` guard's own
test pattern). New `engine-core` unit test: `Tree::position_overlay_
over` covers an arbitrary rect, independent of any anchor, and doesn't
attach the node anywhere by itself.

`examples/docking.py` registers a translucent highlight via `set_drop_
zone_highlight`; its own doc comment, which previously named this
exact gap, is corrected to state it's closed.

Full `cargo test --workspace --release` clean (81 `engine-core` tests,
up from 80 -- the new `position_overlay_over` test), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean. `maturin develop --release` + full `pytest tests/` (93 passed,
up from 87, 1 skipped) and all sixteen examples confirmed clean,
including `docking.py` with its new highlight.

M10 (Interaction & Overlay Completeness) is now complete: all 3 phases
done.
