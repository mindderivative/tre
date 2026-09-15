# Input & Focus

`tre-engine`'s `input` and `focus` modules: the backend-agnostic event model every platform backend translates into, and the in-app widget focus/Tab-order tracker layered on top of it. These are two genuinely different kinds of "focus" -- kept in separate modules on purpose:

- **OS-level window focus** (`InputEvent::WindowFocused`) -- alt-tab, clicking another app. Fires even for an application with no concept of a focused *widget* at all.
- **In-app widget focus** (`FocusManager`) -- which button/field has keyboard focus, and Tab-order traversal between them. Nothing like this existed before Phase 19.

## `InputEvent` and friends

```rust
pub struct WindowId(pub u64);

pub enum MouseButton { Left, Right, Middle, Other(u16) }
pub enum ElementState { Pressed, Released }
```

`WindowId` is assigned by `tre-platform`'s `PlatformConnection` at window creation, stable for that window's lifetime, and never reused while the owning connection is alive -- safe to use directly as a map key.

```rust
#[derive(Debug, Clone, PartialEq)] // not Copy: FileDropped/FileHovered/Ime* carry owned data
pub enum InputEvent {
    PointerMoved { window: WindowId, x: f64, y: f64 },
    PointerButton { window: WindowId, button: MouseButton, state: ElementState },
    KeyboardKey { window: WindowId, key_code: u32, state: ElementState },
    CloseRequested { window: WindowId },
    Resized { window: WindowId, width: u32, height: u32 },
    WindowFocused { window: WindowId, focused: bool },
    FileDropped { window: WindowId, path: std::path::PathBuf },
    FileHovered { window: WindowId, path: std::path::PathBuf },
    FileHoverCancelled { window: WindowId },
    ImeEnabled { window: WindowId },
    ImePreedit { window: WindowId, text: String, cursor: Option<(usize, usize)> },
    ImeCommit { window: WindowId, text: String },
    ImeDisabled { window: WindowId },
}
```

Every variant carries the `WindowId` it originated from, so a multi-window application can route events without querying per-backend state.

- **`KeyboardKey::key_code`** is the raw platform key code -- the Linux evdev keycode on both Wayland and X11. Layout-aware translation (turning a key code into a character) is deliberately a UI-framework concern, out of scope here.
- **`WindowFocused`** is real OS-level window focus (winit's `WindowEvent::Focused(bool)`), distinct from `FocusManager` below.
- **`ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled`** never fire until the platform layer's `set_ime_allowed(window, true)` has been called for that window -- a real platform requirement, not a `tre` choice. `ImePreedit` carries not-yet-committed composition text (e.g. Pinyin candidates); a caller renders it as live preview text at the caret, replacing it entirely on the next `ImePreedit` or `ImeCommit`. `ImeCommit` is real, final text, inserted exactly like a directly-typed character.

### `InputEventQueue`

```rust
pub struct InputEventQueue { /* wraps tre_memory::SpscRingBuffer<InputEvent> */ }

impl InputEventQueue {
    pub fn with_capacity(capacity: usize) -> Self;
    pub fn push(&mut self, event: InputEvent);
    pub fn flush_pending_move(&mut self);
    pub fn drain(&mut self) -> Vec<InputEvent>;
}
```

The producer-side queue every platform backend pushes translated events into. **Only `PointerMoved` coalesces**: a `PointerMoved` for the same window as the currently staged pending move overwrites that staged value instead of publishing a new queue entry, so a burst of high-frequency raw OS motion events collapses to the single most recent position by the time a consumer drains the queue. Every other variant -- including two consecutive `WindowFocused` events for the same window -- survives the queue in full; this was specifically tested when `WindowFocused` was added, to confirm it doesn't accidentally fall into the same coalescing path. A full underlying ring buffer silently drops the event rather than blocking or panicking: input events are a UI convenience, never worth stalling a render frame for.

### `FrameClock`

```rust
pub struct FrameClock { /* private */ }

impl FrameClock {
    pub fn new() -> Self;
    pub fn tick(&mut self) -> f32;     // seconds since the previous tick() call; 0.0 on the first call
    pub fn elapsed(&self) -> f32;      // total seconds since FrameClock::new(), independent of tick() calls
}
```

A real, hardware-backed monotonic frame timer (`std::time::Instant` under the hood -- already wraps `clock_gettime(CLOCK_MONOTONIC)`/`QueryPerformanceCounter` internally, so no per-platform code is needed here). `tick()`'s output feeds `tre-math::spring_decay`'s `dt` parameter; `elapsed()` is the real "t" a `Tween`/`Timeline` caller samples against.

## Focus (`FocusManager`)

Reuses `AccessibilityNodeId` as its node identity, rather than inventing a second parallel id system for the same widget tree -- consistent with that type's own "the framework owns the tree, this engine just reports positions back" precedent.

```rust
pub struct FocusableNode {
    pub node_id: AccessibilityNodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub tab_index: Option<i32>,
}

pub struct FocusManager { /* private: focused: Option<AccessibilityNodeId> */ }

impl FocusManager {
    pub fn new() -> Self;
    pub fn focused(&self) -> Option<AccessibilityNodeId>;
    pub fn set_focus(&mut self, node_id: Option<AccessibilityNodeId>);
    pub fn focus_next(&mut self, nodes: &[FocusableNode]) -> Option<AccessibilityNodeId>;
    pub fn focus_previous(&mut self, nodes: &[FocusableNode]) -> Option<AccessibilityNodeId>;
}
```

`FocusManager` is **persistent** (not per-frame-cleared, unlike the `FocusableNode` list `RenderingCanvas::tag_focusable` builds each frame -- see [Canvas & Intermediate Representation](canvas-and-ir.md)), and is plain, `Sync`-free in-memory logic with no OS handle or thread affinity, unlike `tre-python`'s `unsendable` platform-connection wrappers.

**Deliberately does not track modifier keys or parse raw `key_code`s itself.** The caller recognizes Tab/Shift+Tab (and tracks Shift state) and calls `focus_next`/`focus_previous` once it has decided a tab navigation should happen -- matching `KeyboardKey`'s own "layout-aware translation is a UI framework concern" boundary, and `tre-text`'s `EditableText::move_caret_left(extend: bool)` precedent of having the *caller* track Shift state and pass a plain bool.

### Tab order

Sequential Tab-navigation order follows the real, external **HTML `tabindex` convention** (WHATWG HTML Standard §6.6.7) rather than an invented rule:

1. Nodes with a **positive** `tab_index`, first, ascending by value -- ties broken by input order (this engine's "geometry order" stand-in, since it has no widget tree of its own to derive real document order from).
2. Nodes with `tab_index` of `None` or `Some(0)`, in input order.
3. Nodes with a **negative** `tab_index` are excluded from `focus_next`/`focus_previous` entirely -- HTML's own "focusable, but skip me during sequential Tab navigation" convention -- though `set_focus` can still target them directly (programmatic/click-to-focus is a different operation from Tab traversal).

`focus_next`/`focus_previous` both wrap around (last back to first, and vice versa). If nothing is currently focused, or the focused node id no longer appears in `nodes` (removed/unmounted since it was focused), both fall back to focusing the first node in tab order rather than getting stuck on a stale id. Both return `None` only when `nodes` has no node eligible for sequential navigation at all (empty, or every node has a negative `tab_index`).
