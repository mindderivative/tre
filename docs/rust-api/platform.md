# Platform

`tre-platform` provides real OS windowing (via [`winit`](https://docs.rs/winit)), clipboard (`arboard`), native file dialogs (`rfd`), and system tray (`tray-icon`) integration -- the platform layer `tre-python`'s `WindowedRenderer`/`Clipboard`/`TrayIcon`/file-dialog bindings wrap directly. **Linux only** (Wayland primary, X11 fallback) -- an explicit scope decision, not merely "unverified": no Windows/macOS code paths exist in this crate at all. It needs no `unsafe` of its own (`#![forbid(unsafe_code)]`) -- `winit`'s `Window`/`EventLoop` already implement `raw-window-handle` 0.6's traits directly.

## Crate-root re-exports

```rust
pub use clipboard::Clipboard;
pub use file_dialog::{pick_file, pick_files, pick_folder, save_file, FileFilter};
pub use tray::{
    init as tray_init, poll_events as tray_poll_events, pump_events as tray_pump_events, Menu,
    TrayEvent, TrayIcon,
};
pub use tre_engine::{ElementState, InputEvent, MouseButton, WindowId};
```

That last line is a single blanket re-export straight from `tre_engine` -- any new `InputEvent` variant added upstream (like Phase 19's `WindowFocused`) is automatically visible through `tre_platform::InputEvent` with no changes needed in this crate.

## `PlatformConnection`

```rust
pub enum PlatformConnection {
    Wayland(/* private */),
    X11(/* private */),
}
```

One shared display-server connection, owning every window created through it. Pick a backend once per process and create all of an application's windows from the same `PlatformConnection` -- a second one opens a second, independent connection.

| Method | Signature | Notes |
|---|---|---|
| `new` | `pub fn new() -> Result<Self, PlatformError>` | Wayland if `WAYLAND_DISPLAY` is set, else X11. |
| `new_wayland` / `new_x11` | `pub fn new_wayland() -> Result<Self, PlatformError>` | Force a specific backend. |
| `create_window` | `pub fn create_window(&mut self, title: &str, width: u32, height: u32) -> Result<WindowId, PlatformError>` | |
| `poll_events` | `pub fn poll_events(&mut self) -> Vec<InputEvent>` | Drains pending events for every window on this connection. Call once per frame; never blocks. |
| `scale_factor` | `pub fn scale_factor(&self, window: WindowId) -> f64` | Real per-window DPI scale, at full `f64` precision. |
| `window_handle` | `pub fn window_handle(&self, window: WindowId) -> Result<WindowHandle<'_>, HandleError>` | |
| `set_title` | `pub fn set_title(&self, window: WindowId, title: &str) -> Result<(), PlatformError>` | |
| `set_minimized` / `is_minimized` | `pub fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError>` / `pub fn is_minimized(&self, window: WindowId) -> Option<bool>` | See caveats below. |
| `set_maximized` / `is_maximized` | `pub fn set_maximized(&self, window: WindowId, maximized: bool) -> Result<(), PlatformError>` / `pub fn is_maximized(&self, window: WindowId) -> bool` | |
| `set_icon` | `pub fn set_icon(&self, window: WindowId, icon: Option<WindowIcon>) -> Result<(), PlatformError>` | `None` clears it. |
| `set_cursor` | `pub fn set_cursor(&self, window: WindowId, icon: CursorIcon) -> Result<(), PlatformError>` | |
| `set_ime_allowed` | `pub fn set_ime_allowed(&self, window: WindowId, allowed: bool) -> Result<(), PlatformError>` | Required before winit emits any IME event. |

Every method taking a `WindowId` this connection didn't create returns `PlatformError::UnknownWindow`.

### Real platform caveats (disclosed directly in doc comments)

- **`winit::EventLoop` can be constructed only once per process, ever** -- a permanent, process-global restriction. Harmless today (every demo is its own `fn main()` process) but relevant to any future in-process test building two.
- **`set_minimized`/`set_maximized` are fire-and-forget requests**, not immediately reflected by `is_minimized`/`is_maximized` -- both are one-way protocol requests to the compositor/window manager; call `poll_events` (possibly across real wall-clock time) before the confirmation is observed.
- **Wayland cannot un-minimize** (`minimized: false` has no visible effect -- a real protocol-level limitation winit can't lift); minimizing works on both backends.
- **Wayland has no way to query minimized state at all** -- `is_minimized` always returns `None` there.
- **`set_icon` is a no-op on Wayland** (no client-side icon protocol; icons come from desktop-file `app_id` metadata); works on X11 subject to the window manager's own icon-size conventions.
- **IME events require explicit opt-in** via `set_ime_allowed(window, true)` -- winit never emits `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` otherwise.

## `Clipboard`

Text-only (v1 scope -- `arboard`'s `image-data` feature is disabled; image clipboard is disclosed future work). Construct once, reuse across calls -- opening a real connection to the platform clipboard service is not cheap to repeat per call.

```rust
impl Clipboard {
    pub fn new() -> Result<Self, PlatformError>;
    pub fn get_text(&mut self) -> Result<String, PlatformError>;
    pub fn set_text(&mut self, text: &str) -> Result<(), PlatformError>;
}
```

## File dialogs

Plain, one-shot functions (no persistent connection to hold, unlike `Clipboard`) -- each call opens a fresh native dialog and **blocks the calling thread** until the user responds. On Linux, backed by the XDG desktop portal (`ashpd`), verified against a real, live KDE Plasma portal implementation before being added.

```rust
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,  // no leading dot, e.g. ["txt", "md"]
}

pub fn pick_file(title: Option<&str>, filters: &[FileFilter], starting_directory: Option<&Path>) -> Option<PathBuf>;
pub fn pick_files(title: Option<&str>, filters: &[FileFilter], starting_directory: Option<&Path>) -> Option<Vec<PathBuf>>;
pub fn pick_folder(title: Option<&str>, starting_directory: Option<&Path>) -> Option<PathBuf>;
pub fn save_file(title: Option<&str>, filters: &[FileFilter], starting_directory: Option<&Path>, default_file_name: Option<&str>) -> Option<PathBuf>;
```

`None` means the user cancelled -- not an error, matching every real OS file dialog's own convention. `pick_folder` takes no `filters` (folders aren't filtered).

## System tray

**Real, required integration constraint** (confirmed in isolated feasibility testing, not assumed): on Linux, `tray-icon` needs a real GTK event loop pumped on the *same thread* that creates and owns it -- `tre`'s own windowed rendering uses `winit`'s `EventLoop`, not GTK's. A real caller must call `tray_pump_events()` once per frame, alongside `PlatformConnection::poll_events()`, to keep the tray icon and its menu responsive. `tray_init()` must be called exactly once, before creating any `TrayIcon`/`Menu`, on that same thread.

```rust
pub fn init() -> Result<(), PlatformError>;         // re-exported as tray_init
pub fn pump_events();                                // re-exported as tray_pump_events

pub struct Menu { /* ... */ }
impl Menu {
    pub fn new() -> Self;
    pub fn add_item(&self, label: &str, enabled: bool) -> Result<String, PlatformError>; // returns item id
    pub fn add_separator(&self) -> Result<(), PlatformError>;
}

pub struct TrayIcon { /* ... */ }
impl TrayIcon {
    pub fn new(rgba: Vec<u8>, width: u32, height: u32, tooltip: Option<&str>, menu: Option<Menu>) -> Result<Self, PlatformError>;
    pub fn set_tooltip(&self, tooltip: Option<&str>) -> Result<(), PlatformError>;
    pub fn set_icon(&self, rgba: Vec<u8>, width: u32, height: u32) -> Result<(), PlatformError>;
    pub fn set_temp_dir_path(&self, path: Option<&std::path::Path>); // Linux-only, no-op elsewhere
}

pub enum TrayEvent {
    IconClick,
    MenuItemClick { item_id: String },
}
pub fn poll_events() -> Vec<TrayEvent>;              // re-exported as tray_poll_events
```

`rgba` buffers must always be exactly `width * height * 4` bytes (both `TrayIcon::new` and `set_icon`). `TrayEvent::IconClick` does not distinguish left/right/double click -- `tray-icon` itself doesn't distinguish these consistently cross-platform, so this is disclosed v1 scope: click detection, not click-*kind* detection. `set_temp_dir_path` exists because Linux's `AppIndicator` backend has no in-memory icon API at all -- it reads each new icon from a real file on disk (default `$XDG_RUNTIME_DIR/tray-icon` or `/tmp/tray-icon`); this method redirects where that file is written, mainly so `set_icon`'s real effect can be verified automatically by reading the PNG back.

## `WindowIcon` / `CursorIcon` / `PlatformError`

```rust
pub struct WindowIcon {
    pub rgba: Vec<u8>,   // must equal width * height * 4
    pub width: u32,
    pub height: u32,
}

pub enum CursorIcon {
    Default, ContextMenu, Help, Pointer, Progress, Wait, Cell, Crosshair, Text, VerticalText,
    Alias, Copy, Move, NoDrop, NotAllowed, Grab, Grabbing,
    EResize, NResize, NeResize, NwResize, SResize, SeResize, SwResize, WResize, EwResize,
    NsResize, NeswResize, NwseResize, ColResize, RowResize, AllScroll, ZoomIn, ZoomOut,
}
// 34 variants total -- the real, complete CSS3/`cursor-icon` set, bound directly
// rather than a smaller invented subset. `Default` is `#[default]`.

pub enum PlatformError {
    ConnectionFailed,
    ProtocolMissing(&'static str),
    UnknownWindow,   // `window` wasn't created by this connection, or was already closed
    Other(String),
}
// implements std::error::Error + Display
```

## How `WindowEvent`s become `InputEvent`s

The internal (private) `winit_backend` module's `Handler::window_event` is the exact translation every `InputEvent` a caller ever sees from `PlatformConnection::poll_events` passes through:

| `winit::event::WindowEvent` | `InputEvent` produced |
|---|---|
| `CloseRequested` | `CloseRequested { window }` |
| `Resized(size)` | `Resized { window, width, height }` |
| `Focused(focused)` | `WindowFocused { window, focused }` |
| `CursorMoved { position, .. }` | `PointerMoved { window, x, y }` |
| `MouseInput { state, button, .. }` | `PointerButton { window, button, state }` |
| `KeyboardInput { event, .. }` | `KeyboardKey { window, key_code, state }` -- only if `event.physical_key.to_scancode()` returns `Some` (evdev keycode); a truly unidentified key is silently dropped |
| `DroppedFile` / `HoveredFile` / `HoveredFileCancelled` | `FileDropped` / `FileHovered` / `FileHoverCancelled` |
| `Ime(Enabled\|Preedit\|Commit\|Disabled)` | `ImeEnabled` / `ImePreedit { text, cursor }` / `ImeCommit { text }` / `ImeDisabled` |
| everything else (`Moved`, `Destroyed`, `ScaleFactorChanged`, `ThemeChanged`, `Occluded`, `CursorEntered`/`Left`, `MouseWheel`, touch/gesture events, the newer pointer-event set, ...) | **silently dropped** -- a trailing `_ => {}` arm |

Events for a `winit::window::WindowId` this connection doesn't recognize (not created here, or already closed and removed) are dropped even before this match runs.
