# Working with Files

`tre`'s engine takes **data**: an image as decoded RGBA pixels, a font as
raw bytes. The rest of this guide is written around
those data forms, because that is what a framework built on `tre` hands it.

This page is for using `tre` **directly, without a framework**, where
it's convenient to point `tre` at files on disk. Every file-based API
below is a convenience layered on a data form: `tre` reads the file,
parses or decodes it, then runs exactly the same code the data form
runs.

!!! tip "Building a framework on `tre`?"
    Read files yourself and use the data forms in the right-hand column
    below. Then your framework decides the file formats, where files come
    from (disk, network, a bundle), how they're pre-processed, and when to
    reload them — `tre` never needs to know.

| File convenience | What `tre` does | Data form |
| --- | --- | --- |
| `window.add_image("logo.png", ...)` | Decodes the file | [`window.add_image_from_bytes(rgba, ...)`](../api/python/window.md#add_image_from_bytes) |
| — (fonts) | `tre` never reads font files | [`tre.register_font(bytes)`](theming-and-accessibility.md#custom-fonts) |

*Removed in 0.3.5:* view files, `include:`, component files, stylesheet
files, and `poll_reload()` went with `View`, and theme files with theming —
both belong to a framework built on `tre`.

## Images from files

```python
picture = window.add_image("logo.png", width=200, height=120, fit="cover")
```

`tre` decodes **PNG and JPEG** only. For any other format, or an image
that doesn't come from a file, decode it yourself and use the data
form, which takes straight-alpha RGBA8 pixels:

```python
from PIL import Image  # any decoder works

img = Image.open("photo.webp").convert("RGBA")
picture = window.add_image_from_bytes(
    img.tobytes(), img.width, img.height, width=200, height=120, fit="cover"
)
```

## Fonts

There's no file-based font API: `tre` accepts fonts only as bytes. Read
the file and register it:

```python
from pathlib import Path
import tre

tre.register_font(Path("fonts/Inter-Regular.ttf").read_bytes())  # ["Inter"]
```

See [Accessibility & Fonts → Custom fonts](theming-and-accessibility.md#custom-fonts).

## Hot reload inside `App.run()`

Once `App.run()` starts it owns the main thread, and a `Window` can't be
touched from any other thread. [`App.thread_handle()`](../api/python/app.md#thread_handle)
is the bridge: a background thread watches the file and reads it, then
asks the event loop to apply the change.

```python
import threading

handle = app.thread_handle()

def watch():  # runs on a background thread; only touches `handle`
    for text in changes_of("settings.json"):  # your file watcher
        handle.call_soon(lambda text=text: apply_settings(text))  # on the loop

threading.Thread(target=watch, daemon=True).start()
app.run()
```

The runnable
[`examples/threadsafe_reload.py`](https://github.com/mindderivative/tre/blob/main/examples/threadsafe_reload.py)
polls a settings file this way and restyles a live node inside
`App.run()`.
