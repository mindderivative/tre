# Input Events

`WindowedRenderer.poll_events()` returns a list of `InputEvent` values once per frame. Call it every frame; it never blocks.

## `InputEvent`

A "complex enum" -- each variant is a real, distinct class under `tre.InputEvent`, so you dispatch with `isinstance`:

```python
for event in renderer.poll_events():
    if isinstance(event, tre.InputEvent.CloseRequested):
        raise SystemExit
    elif isinstance(event, tre.InputEvent.PointerMoved):
        print(event.window, event.x, event.y)
```

| Variant | Fields |
|---|---|
| `PointerMoved` | `window: WindowId`, `x: float`, `y: float` |
| `PointerButton` | `window: WindowId`, `button: MouseButton`, `state: ElementState` |
| `KeyboardKey` | `window: WindowId`, `key_code: int`, `state: ElementState` |
| `CloseRequested` | `window: WindowId` |
| `Resized` | `window: WindowId`, `width: int`, `height: int` |
| `WindowFocused` | `window: WindowId`, `focused: bool` |
| `FileDropped` | `window: WindowId`, `path: str` |
| `FileHovered` | `window: WindowId`, `path: str` |
| `FileHoverCancelled` | `window: WindowId` |
| `ImeEnabled` | `window: WindowId` |
| `ImePreedit` | `window: WindowId`, `text: str`, `cursor: tuple[int, int] \| None` |
| `ImeCommit` | `window: WindowId`, `text: str` |
| `ImeDisabled` | `window: WindowId` |

Notes:

- **`key_code`** is the raw platform (Linux evdev) keycode -- a fixed physical key position, not a layout-aware character. Translating it into a character or a named key ("the Tab key", "the letter A") is left to you; the engine deliberately doesn't do layout-aware translation.
- **`WindowFocused`** is real, OS-level window focus (alt-tab, clicking another app) -- distinct from in-app *widget* focus, which is `tre.FocusManager`'s job (see [Accessibility & Focus](accessibility-and-focus.md)). It fires even for an app with no concept of a focused widget at all.
- **`FileDropped`/`FileHovered`**'s `path` is a plain `str` (lossy-converted from the OS path) -- simpler than round-tripping through `os.PathLike`, at the cost of lossy display-only handling for a real but rare non-UTF-8 path.
- IME events only fire after you call `renderer.set_ime_allowed(window, True)` (see [Rendering](rendering.md)).

## `WindowId`

Frozen, equatable, and hashable -- safe to use as a dict key when routing events across multiple windows.

```python
WindowId(id: int)  # normally handed to you by WindowedRenderer; useful for synthesizing test events
```

## `MouseButton`

Variants: `Left()`, `Right()`, `Middle()`, `Other(code: int)` -- a raw platform button code for buttons beyond the three common ones. (The empty-tuple call syntax on the first three is a real PyO3 constraint: unit variants can't mix with data-carrying variants in one complex enum.)

## `ElementState`

Variants: `Pressed`, `Released`.