# Accessibility

Accessibility is split cleanly across two crates: `tre-engine` tags spatial nodes during a frame (no OS dependency, no rendering-loop hook), and `tre-a11y` publishes those tagged nodes to the real Linux AT-SPI2 bus. Neither crate depends on any RHI backend.

## Tagging (`tre-engine`)

### Node types (`tre_engine::{AccessibilityNodeId, AccessibilityRole, AccessibilityNode}`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccessibilityNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityRole {
    Generic,
    Button,
    TextLabel,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccessibilityNode {
    pub node_id: AccessibilityNodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub role: AccessibilityRole,
}
```

`AccessibilityNodeId` is an opaque, caller-assigned stable key -- the UI framework already owns the real widget tree and its hierarchy; the engine only reports each tagged node's *rendered* spatial position back, keyed by whatever id the framework itself already tracks. `AccessibilityRole` is a small, real, useful starter set rather than an attempt at AT-SPI2's own roughly 130-role taxonomy -- trivially extensible once a real OS bridge reveals which additional roles are actually needed. `AccessibilityNode`'s `x`/`y`/`width`/`height` are the axis-aligned bounding box after the active transform has been applied -- stored as `f32`, not yet rounded to whatever integer convention a real OS accessibility bridge ultimately needs.

### `RenderingCanvas::tag_accessibility_node` / `accessibility_nodes`

```rust
impl RenderingCanvas {
    pub fn tag_accessibility_node(
        &mut self,
        node_id: AccessibilityNodeId,
        x: f32, y: f32, width: f32, height: f32,
        role: AccessibilityRole,
    );

    pub fn accessibility_nodes(&self) -> &[AccessibilityNode];
}
```

`x`/`y`/`width`/`height` are in the canvas's *local* space, matching every other drawing primitive's convention. **All four corners** are transformed by the active `Affine2` (not just the top-left) and the stored bounds are the real axis-aligned bounding box of those four transformed corners -- correct even when the active transform includes a rotation, unlike a naive reuse of the local width/height at a transformed origin. The result is a genuine axis-aligned rect an OS accessibility API can consume directly (AT-SPI2's `Component::GetExtents`, UIA's `BoundingRectangle`).

Two disclosed, deliberate simplifications:

- **Does not intersect against the active clip stack** -- a node partially scrolled out of view still reports its full transformed bounds.
- **Not threaded through multi-threaded `SubCanvas` recording** -- `accessibility_nodes()` is read directly off the root `RenderingCanvas`, *before* `render_canvas()` consumes it. No real caller records accessibility nodes off the main thread today, so this plumbing (which exists for the vertex/index/command buffers) wasn't extended here.

A real caller publishes `accessibility_nodes()`'s result to a real `tre_a11y::A11yBridge` once per rendered frame.

### `RenderingCanvas::tag_focusable` / `focusable_nodes`

```rust
impl RenderingCanvas {
    pub fn tag_focusable(
        &mut self,
        node_id: AccessibilityNodeId,
        x: f32, y: f32, width: f32, height: f32,
        tab_index: Option<i32>,
    );

    pub fn focusable_nodes(&self) -> &[FocusableNode];
}
```

The same transform-correct bounding-box logic as `tag_accessibility_node` (both share a private `transform_bounds` helper), tagging a widget for `FocusManager::focus_next`/`focus_previous` to traverse instead (see [Input & Focus](input-and-focus.md) for `FocusableNode` and the tab-order rule). `node_id` reuses the same caller-assigned identifier `tag_accessibility_node` uses for this widget tree -- deliberately not a second, parallel id system. Same ordering rule as `accessibility_nodes()`: read `focusable_nodes()` before `render_canvas()` consumes the canvas.

## Publishing (`tre-a11y`)

`tre-a11y` publishes `tre-engine`'s tagged nodes to the real Linux AT-SPI2 accessibility bus via [`accesskit`](https://docs.rs/accesskit)/`accesskit_unix`. It has no dependency on any RHI backend and no rendering-loop hook of its own -- fully decoupled from rendering.

```rust
pub struct A11yBridge { /* private */ }

impl A11yBridge {
    #[must_use]
    pub fn connect(
        app_name: impl Into<String>,
        toolkit_name: impl Into<String>,
        toolkit_version: impl Into<String>,
    ) -> Self;

    pub fn publish(&self, nodes: &[AccessibilityNode]);
}
```

That's the entire public surface -- two methods, no public fields, no submodules.

- **`connect` is infallible.** `accesskit_unix::Adapter::new` never returns a `Result` -- it gracefully degrades to a permanently-inactive adapter when no AT-SPI2 registry is reachable, and `publish` becomes a real no-op in that case rather than an error.
- **`publish` never blocks on D-Bus I/O.** `accesskit_unix::Adapter` already owns a real, dedicated background thread and an unbounded internal channel; this method only mutates in-process state and, if a real assistive technology is already connected, enqueues an update via that thread's own channel.
- **Rebuilds the entire accessibility tree from scratch on every call** -- no incremental diffing. A synthesized `Role::Window` root (reserved id `u64::MAX`) is inserted as the parent of every published node, since `accesskit::TreeUpdate` requires exactly one root but tagged nodes are a flat list with no such concept. `TreeUpdate::focus` is hard-coded to that synthesized root always -- no real per-node focus is reported to AT-SPI2 yet, a disclosed simplification independent of `FocusManager`'s own in-app tracking.
- **Maps only role + bounds** -- no accessible name, description, value, or state flags are published; `AccessibilityRole::Generic` maps to `Role::Unknown` (deliberately not `Role::GenericContainer`, since `accesskit_atspi_common`'s own filter excludes `GenericContainer` from the platform tree entirely).

!!! warning "Single-writer only (REVIEW.md finding #133)"
    `publish`'s internal node-list update and the live AT-SPI2 tree update happen via two separate lock acquisitions, not one atomic step -- two threads calling `publish` concurrently with different node sets could leave the two out of sync with each other. Every real caller today publishes from one thread only, so this is latent, not exercised, but unlike `SwmrSlotTable`/`MpscRingBuffer` (see [Math & Memory](math-and-memory.md)), nothing in this type enforces the single-writer contract. A future multi-window/multi-render-thread caller must serialize its own `publish` calls.

`publish` panics if either internal lock is poisoned (i.e. a prior call panicked while holding it) -- not expected in normal operation. Internally, `A11yBridge` talks to AT-SPI2 entirely through `accesskit_unix::Adapter`; it has no `zbus`/D-Bus code of its own (`zbus` appears only as a dev-dependency, used by the crate's own integration test to independently verify what gets published).
