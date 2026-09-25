# Working with Files

`tre`'s engine takes **data**: a view as a spec (a `dict`), a theme or
stylesheet as a `dict`, an image as decoded RGBA pixels, a font as raw
bytes. The rest of this guide is written around those data forms,
because that is what a framework built on `tre` hands it.

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
| `View("view.yaml")` | Reads and parses the YAML, resolves `include:`/`image.src:`, watches the file | [`View(spec=...)`](declarative-views.md#building-a-view) |
| `include: other.yaml` in a view | Splices in another YAML file | Build the combined `dict` yourself, or [`instantiate("", into, spec=...)`](../api/python/view.md#instantiate) |
| `image: {src: logo.png}` in a view | Decodes the file while building | A blank `kind: Image` + [`Node.push_frame`](../api/python/node.md) |
| `window.add_image("logo.png", ...)` | Decodes the file | [`window.add_image_from_bytes(rgba, ...)`](../api/python/window.md#add_image_from_bytes) |
| `View(stylesheet="sheet.yaml")` | Reads the stylesheet YAML | `stylesheet_spec=` |
| `default_theme=`/`custom_theme=` paths | Reads the theme YAML | [`default_theme_spec=`/`custom_theme_spec=`](theming-and-accessibility.md#theme-documents) |
| `view.instantiate("row.yaml", into)` | Reads the component YAML | `view.instantiate("", into, spec=...)` |
| `view.poll_reload()` | Re-reads the file when it changes on disk | [`view.reconcile(spec=...)`](declarative-views.md#updating-a-live-view) |
| — (fonts) | `tre` never reads font files | [`tre.register_font(bytes)`](theming-and-accessibility.md#custom-fonts) |

## Loading a view from a file

```python
from tre import View

view = View("counter.yaml")
node = view.node("my_widget")
```

`View(path)` reads and parses the file, builds the tree, and starts a
filesystem watcher for [hot reload](#hot-reload). The file holds the
same structure as a [`spec=` dict](declarative-views.md#the-view-schema),
written as YAML. Raises `RuntimeError` if the file can't be read, and
`ValueError` if the YAML fails to parse or build.

`path` also sets the **base directory**: `include:` entries and
`image.src:` paths in the view resolve relative to the view file's own
directory.

### Pre-processed content for a real file

`source=` passes YAML text you've already read (and perhaps rewritten)
while keeping `path=` as the real file:

```python
text = Path("counter.yaml").read_text().replace("$ACCENT", "#6750A4")
view = View("counter.yaml", source=text)
```

`path` is required alongside `source=` — it still supplies the base
directory and the file `poll_reload()` watches. `poll_reload(source=...)`
works the same way on a change.

## Composing with `include:`

Split a view across files. An `include:` entry splices the target
file's widget tree in as an ordinary child, indistinguishable from an
inline one once loaded:

```yaml
id: root
kind: Container
style: {flex_direction: Vertical, width: 240, height: 120, gap: 8, padding: 8}
children:
  - id: header
    kind: Rect
    style: {width: 200, height: 16, background: "#6750A4"}
  - include: confirm_dialog.yaml
```

Included paths resolve relative to the including file's directory and
must stay within it (no `../` escape). Cycles are detected, and includes
nest up to 8 deep.

`include:` exists only on the file path — it's resolved while parsing
YAML text, before the view becomes a spec. A `dict` passed to `spec=`
must already be complete.

## Components from files

```python
row = view.instantiate("row.yaml", into=view.node("list_container"))
```

Reads `row.yaml` and embeds it as an independent
[`Component`](../api/python/view.md#component). The data form is
`view.instantiate("", into, spec=row_spec)`.

## Images from files

In a view:

```yaml
- id: logo
  kind: Image
  image: {src: logo.png, fit: Contain}
  style: {width: 120, height: 60}
```

Imperatively:

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

A view's `src:` resolves relative to the view file's directory, confined
the same way `include:` is.

## Stylesheet and theme files

```python
view = View(
    "gallery.yaml",
    stylesheet="gallery_sheet.yaml",
    custom_theme="brand_theme.yaml",
    theme_seed=(0x67, 0x50, 0xA4, 0xFF),
)
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme="brand_theme.yaml")
view.set_theme(custom_theme="brand_theme.yaml")
```

- `stylesheet=` takes a stylesheet file (`{styles: [...]}`) — the
  schema is in [Declarative Views → Stylesheets](declarative-views.md#stylesheets-md3-color-tokens).
- `default_theme=`/`custom_theme=` take a theme file (`seed`, `dark`,
  `colors`, `styles`, `components`, `typography`) — the schema is in
  [Theming & Accessibility → Theme documents](theming-and-accessibility.md#theme-documents).
  Omitting `default_theme` uses the theme shipped inside `tre`.

Each path parameter has a `*_spec` twin taking the same content as a
`dict`; passing both raises `ValueError`.

## Fonts

There's no file-based font API: `tre` accepts fonts only as bytes. Read
the file and register it:

```python
from pathlib import Path
import tre

tre.register_font(Path("fonts/Inter-Regular.ttf").read_bytes())  # ["Inter"]
```

See [Theming & Accessibility → Custom fonts](theming-and-accessibility.md#custom-fonts).

## Hot reload

A `View` built from a file watches it. `poll_reload()` checks whether
the file changed since the last call and, if so, re-reads it and
reconciles it into the live tree:

```python
if view.poll_reload():
    print("view changed on disk, reconciled in place")
```

An unchanged widget (same `id`, same `kind`) keeps its runtime identity,
preserving focus, scroll position, and in-flight animations. The
stylesheet and theme the view was built with are reused on every reload.
**`bindings:`/`handlers:`/`two_way:` are not re-resolved** — if a reload
adds a new binding or handler, call `_attach` again (construct a fresh
`ViewModel`, or call `view._attach(vm)`).

A `View` built with `spec=`/`json=` and no `path=` has no watcher, so
`poll_reload()` always returns `False` — use
[`reconcile()`](declarative-views.md#updating-a-live-view) instead.

### Hot reload inside `App.run()`

`poll_reload()` has to be called from somewhere, but once `App.run()`
starts it owns the main thread, and a `View` can't be touched from any
other thread. Use [`App.thread_handle()`](../api/python/app.md#thread_handle):
a background thread asks the event loop to run the check.

```python
import threading
import time

handle = app.thread_handle()

def reload():            # runs on the event-loop thread
    view.poll_reload()

def ticker():            # runs on a background thread; only touches `handle`
    while True:
        time.sleep(0.25)
        handle.call_soon(reload)

threading.Thread(target=ticker, daemon=True).start()
app.run()
```

`poll_reload()` is cheap when nothing changed — it checks a flag its
filesystem watcher sets. For an event-driven version that reads the file
on the watcher thread instead, see the runnable
[`examples/threadsafe_reload.py`](https://github.com/mindderivative/tre/blob/main/examples/threadsafe_reload.py).
