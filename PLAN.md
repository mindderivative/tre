# Plan: M10 Phase 3 — Drop-Zone Highlight Overlay for Docking (§11.4)

Corresponds to `BUILD_TRACKER.md` M10 Phase 3's own scoping: a real way
to position an overlay to cover an arbitrary target rect (a dock zone's
own real computed bounds), then wiring it into the real
drag-in-progress path so the zone currently under the pointer shows a
real, live highlight — closing the milestone.

## Investigation before writing code

- `crates/engine-py/src/dock.rs`'s own module doc comment (lines
  30-41) already names this exact gap, in its own words, from M4 Phase
  9: "Deliberately no drop-zone highlight overlay. `open_overlay`'s
  own `inset` computation is hardcoded to place content *below* its
  anchor ... it can't cover a target zone's own bounds, which is what
  a highlight needs." Confirmed by direct re-read of `Tree::
  open_overlay` (`tree.rs:417-443`): `style.inset.left = anchor_x`,
  `style.inset.top = anchor_y + anchor_height`, `right`/`bottom` left
  `auto()` — anchor-relative-below only, no width/height override, so
  content keeps whatever size its own `layout_style` already gives it.
  There is no existing way to position a node's absolute box over an
  arbitrary caller-supplied rect.
- `crates/engine-core/src/tree.rs:18-20` already imports everything a
  new "position over rect" primitive needs: `Size`, `length`, `auto`,
  `TaffyRect`, `Position` (taffy) and `Rect` (`peniko::kurbo`, used
  already at `tree.rs:105`). `style.size.width`/`.height` are already
  set via `length(..)` elsewhere (`tree.rs:597-601`, splitter geometry)
  — the same pattern this needs, so no new imports required.
- `dock.rs`'s own doc comment (lines 18-28) already names the real
  precedent for "no `PointerMoved`-time callback for docking at all,
  just press (start) and release (end)" — confirmed via grep: `dock::
  start_drag`/`end_drag_at` are the only two drag-related functions,
  called from `PyWindow.start_panel_drag`/`drop_panel_at`
  (`window.rs:681-703`), both purely synthetic (no real winit event
  routing into `dock.rs` anywhere) — the same "no-live-window-needed
  proof pattern" `.click()`/`.hover()`/`.right_click()` already
  establish (`window.rs:473-506`, `.hover()`'s own doc comment names
  this explicitly). A third synthetic entry point, `drag_panel_over(x,
  y)`, is the natural continuation of that exact vocabulary
  (`start_panel_drag` → `drag_panel_over` → `drop_panel_at`).
- `enclosing_zone` (`dock.rs:203-216`) and `container_for`
  (`dock.rs:194-201`) already exist and are exactly what's needed to
  find "which registered zone's container is under this point" — reused
  as-is, not reimplemented.
- `Tree::absolute_position`/`Tree::layout` (confirmed real, transform-
  aware since M6 Phase 4) give a container's real on-screen rect
  directly: `absolute_position(container)` for origin,
  `layout(container).size` for width/height.
- `DockState` (`dock.rs:58-74`) has no field for a registered highlight
  node yet — needs one, mirroring `handles: HashMap<NodeId, NodeId>`'s
  own simple-storage shape (a single `Option<NodeId>` is enough; unlike
  handles there's only ever one highlight per `Window`).
- `PyWindow.set_dock_handle` (`window.rs:668-674`) is the direct, just-
  shipped (M10 Phase 2) template for the same-tree `Rc::ptr_eq` guard
  the new `set_drop_zone_highlight(content: PyRef<'_, Node>)` needs too
  — one more `PyRef<'_, Node>`-taking registration method, same
  treatment.
- `examples/docking.py`'s own doc comment (lines 18-20) already names
  this gap in user-facing terms: "There is deliberately no translucent
  drop-zone highlight while dragging — a real, separate, stated gap
  this phase's own `LOG.md` names, not silently missing." This phase
  closes it; the example gets a highlight rect registered and its doc
  comment corrected to stop claiming the gap exists.

## Design

**Step 1 — `engine-core` primitive (`tree.rs`, next to `open_overlay`):**

```rust
pub fn position_overlay_over(&mut self, content: NodeId, rect: Rect) {
    let mut style = self
        .get(content)
        .expect("position_overlay_over: content NodeId not found in this Tree")
        .layout_style
        .clone();
    style.position = Position::Absolute;
    style.inset = TaffyRect {
        left: length(rect.x0 as f32),
        top: length(rect.y0 as f32),
        right: auto(),
        bottom: auto(),
    };
    style.size = Size {
        width: length(rect.width() as f32),
        height: length(rect.height() as f32),
    };
    self.set_layout_style(content, style);
}
```

Deliberately does *not* call `add_child`/touch `self.overlays` the way
`open_overlay` does — a drop-zone highlight has no anchor, is never
dismissed by outside-click/Escape (`OverlayMeta` doesn't fit it), and
its attach/detach lifecycle is driven entirely by drag state, not
click/key dispatch. `dock.rs` (Step 2) manages attach/detach itself
via the same `add_child`/`detach` primitives it already uses elsewhere
in this file — one mechanism (this method) for *where the box sits*,
reused by any future caller that needs to cover an arbitrary rect,
independent of *whether it's currently attached*.

**Step 2 — wiring into the drag path (`engine-py`):**

- `DockState` gains `highlight: Option<NodeId>`.
- `dock::set_drop_zone_highlight(dock: &SharedDockState, content:
  NodeId)` — stores it, mirroring `set_dock_handle`'s own plain-insert
  shape.
- `dock::drag_over(dock: &SharedDockState, tree: &Rc<RefCell<Tree>>,
  root: NodeId, position: Point)`:
  - No-op if no drag is in progress (`dragging.is_none()`) or no
    highlight is registered — matching `drop_panel_at`'s own "no drag
    in progress does not raise" no-op precedent.
  - Hit-tests `position`, finds the enclosing registered zone via
    `enclosing_zone` (reused as-is).
  - If a target zone is found: computes its container's real absolute
    rect (`absolute_position` + `layout(..).size`), calls `Tree::
    position_overlay_over(highlight, rect)`, and `add_child(root,
    highlight)` if not already attached (checked via `Node::parent`,
    the same "check tree parent directly" idiom `dock_panel` already
    uses at `dock.rs:121-126`).
  - If no target zone (pointer outside every registered zone): detach
    the highlight if it's currently attached — it must disappear the
    moment the pointer leaves every zone, not linger over the last one.
- `dock::end_drag_at` gains one new unconditional step, right after
  taking `dragging` (before any of its existing early returns): detach
  the highlight if attached. A real drag ending — for *any* reason
  (real move, cancelled drop outside every zone, drop back in the same
  zone) — must always hide the highlight; putting this before the
  function's existing early-return branches is what makes it
  unconditional, not just one of several exit paths.
- `PyWindow.set_drop_zone_highlight(&mut self, content: PyRef<'_,
  Node>) -> PyResult<()>` — the same `Rc::ptr_eq` same-tree guard M10
  Phase 2 just added to `set_dock_handle`, applied to this new
  registration method too.
- `PyWindow.drag_panel_over(&mut self, x: f64, y: f64)` — computes
  layout fresh (the same "nothing else does this for a `Window` with
  no render loop attached" reasoning `.click()`/`drop_panel_at`
  already state), then calls `dock::drag_over`.

**Example update:** `examples/docking.py` registers a translucent
highlight rect via the new `set_drop_zone_highlight`, and its doc
comment is corrected to state the gap is closed (matching this
project's own discipline of never leaving a stated gap's own doc
comment stale once the gap is closed — the exact class of staleness
M10 Phase 1 found and fixed for `overlay.rs`'s doc comment).

## Verification plan

- `cargo test --workspace --release` — new `engine-core` unit test(s)
  for `Tree::position_overlay_over` (positions and sizes a node to
  cover an arbitrary rect, independent of any anchor).
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
  --check`.
- `maturin develop --release` + `pytest tests/`. New `test_docking.py`
  coverage: dragging a handle and calling `drag_panel_over` with a
  point inside a *different* registered zone shows the highlight
  attached and covering that zone's real container bounds (the same
  "clicking it there proves it really moved" functional-proof
  discipline this file already uses — here, checking the highlight's
  own real computed position/size after `compute_layout`, since
  there's no other observable signal); a point outside every zone
  leaves it detached; ending the drag (`drop_panel_at`) always detaches
  it. Existing `test_docking.py` tests must keep passing unmodified.
- Run all examples (`examples/docking.py` at minimum) — confirm clean
  exit, no panic, matching every prior phase's own verification
  discipline.
