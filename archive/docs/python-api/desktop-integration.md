# Desktop Integration

System clipboard, native file dialogs, and a system tray icon with a native menu. See **Platform Integration** for OS-specific setup notes and gotchas -- this page is the API reference.

## `Clipboard`

**Must stay on its constructing thread.** Construct once and reuse -- opening the underlying platform clipboard connection isn't cheap to repeat per call. Real, disclosed v1 scope: plain text only (no images, no rich text).

```python
clipboard = tre.Clipboard()
clipboard.set_text("hello")
clipboard.get_text()  # -> "hello"
```

| Method | Signature | Raises |
|---|---|---|
| `__init__` | `()` | `RuntimeError` if no real clipboard service is reachable |
| `get_text` | `() -> str` | `RuntimeError` if the clipboard is empty, holds non-text content, or the platform service failed |
| `set_text` | `(text: str)` | `RuntimeError` if the platform service rejected the write |

Also used directly by `EditableText.copy`/`cut`/`paste` (see [Text Editing](text-editing.md)).

## Native file dialogs

Four free functions, each releasing the GIL for its own real, blocking wait -- a native dialog can block on user interaction for an unbounded amount of wall-clock time, and other Python threads keep running while it does.

```python
path = tre.pick_file(title="Open", filters=[("Images", ["png", "jpg"])])
if path is not None:
    ...
```

| Function | Signature | Notes |
|---|---|---|
| `pick_file` | `(title: str \| None = None, filters: list[tuple[str, list[str]]] = [], starting_directory: str \| None = None) -> str \| None` | `None` on cancel -- not an error, matching every real OS dialog's own convention |
| `pick_files` | same params | Multi-select variant — returns `list[str] \| None` |
| `pick_folder` | `(title=None, starting_directory=None) -> str \| None` | No `filters` -- doesn't apply to folder selection |
| `save_file` | `(title=None, filters=[], starting_directory=None, default_file_name: str \| None = None) -> str \| None` | `default_file_name` pre-fills the file-name field, e.g. `"untitled.svg"` |

`filters` is a list of `(display_name, extensions)` pairs, e.g. `[("Images", ["png", "jpg"])]` -- extensions have no leading dot.

## System tray

`Menu` and `TrayIcon` **must stay on their constructing thread** (they wrap real GTK state on Linux, which has its own thread-affinity requirement). You must also drive GTK's own event loop yourself, since `tre`'s windowed event loop doesn't do it for you.

```python
tre.tray_init()  # once, before creating any Menu/TrayIcon

menu = tre.Menu()
copy_id = menu.add_item("Copy", enabled=True)
menu.add_separator()

icon = tre.TrayIcon(rgba_bytes, width, height, tooltip="My App", menu=menu)

while True:
    tre.tray_pump_events()  # required every frame
    for event in tre.tray_poll_events():
        if isinstance(event, tre.TrayEvent.MenuItemClick) and event.item_id == copy_id:
            ...
```

### `Menu`

| Method | Signature | Raises |
|---|---|---|
| `__init__` | `()` | |
| `add_item` | `(label: str, enabled: bool) -> str` | Returns the item's own id, matched against a later `TrayEvent.MenuItemClick.item_id`. `RuntimeError` if the platform backend rejects it |
| `add_separator` | `()` | `RuntimeError` on backend rejection |

Ownership genuinely transfers into a `TrayIcon` once you construct one with `menu=...` -- a `Menu` already attached elsewhere raises `ValueError` if you try to attach it again.

### `TrayIcon`

```python
TrayIcon(rgba: bytes, width: int, height: int, tooltip: str | None = None, menu: Menu | None = None)
```

`rgba` must be exactly `width*height*4` bytes. `tray_init()` must be called once, on this same thread, before the first `TrayIcon` is created. Raises `ValueError` if `menu` is already attached to another icon; `RuntimeError` if `rgba`'s size is wrong or the backend rejects creation (e.g. no tray-watcher service is running on this desktop).

| Method | Signature | Notes |
|---|---|---|
| `set_tooltip` | `(tooltip: str \| None)` | `None` clears it. `RuntimeError` on backend rejection |
| `set_icon` | `(rgba: bytes, width: int, height: int)` | Same size contract as the constructor |
| `set_temp_dir_path` | `(path: str \| None)` | **Linux only**, a real no-op elsewhere: redirects where the real GTK/`appindicator` backend writes each new icon as a temporary PNG. `None` reverts to the default (`$XDG_RUNTIME_DIR/tray-icon` or `/tmp/tray-icon`) |

### `TrayEvent`

Variants: `IconClick()`, `MenuItemClick(item_id: str)`.

### Free functions

| Function | Signature | Notes |
|---|---|---|
| `tray_init` | `()` | Call once, before any `Menu`/`TrayIcon`, on the thread that will own them. `RuntimeError` if GTK fails to initialize |
| `tray_pump_events` | `()` | Required once per frame for the tray icon/menu to register, receive clicks, or update at all |
| `tray_poll_events` | `() -> list[TrayEvent]` | Drains every tray event queued since the last call -- call once per frame, alongside `tray_pump_events()` |