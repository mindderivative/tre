# Threading

Tesserae Engine runs on one thread: the one that created the `App`. Its windows,
nodes, and every listener, animation completion, and draw callback live and
run there, and once `App.run()` starts it owns that thread until the last
window closes. Everything else — downloads, subprocesses, file watching,
heavy computation — belongs on other threads, which hand their results back
through one object.

## Handing work to the event loop

`App.thread_handle()` returns a `LoopHandle`, the one Tesserae Engine object any thread
may use. `handle.call_soon(fn)` queues `fn` to run on the event-loop thread
and wakes the loop, even when it's idle waiting for input:

```python
import threading
from tre import App, Window

window = Window(width=400, height=120)
status = window.create("text", text="Loading...", width=360, height=24)
window.root.add_child(status)
app = App()
app.add_window(window)
handle = app.thread_handle()

def load():                                   # a background thread
    rows = fetch_report()                     # slow I/O, off the loop
    handle.call_soon(lambda: status.set(text=f"{len(rows)} rows"))

threading.Thread(target=load, daemon=True).start()
app.run()
```

Callbacks run in the order they were queued, at the top of the next frame,
where they can touch windows and nodes exactly as a listener can. A callback
queued before `run()` runs on its first frame. One that raises is logged and
doesn't stop the loop or the callbacks after it. Every handle from one `App`
shares one queue.

## Keep the work off the loop

A `call_soon` callback runs between frames, so it should only apply a
result: read files, parse, decode images, and query databases on the worker,
then hand over the finished data. Decoding an image on a worker and showing
it on the loop:

```python
from PIL import Image  # Pillow

def load_thumbnail(path, node):
    img = Image.open(path).convert("RGBA")          # on the worker
    rgba, w, h = img.tobytes(), img.width, img.height
    handle.call_soon(lambda: node.set(rgba=rgba, pixel_width=w, pixel_height=h))
```

## Hot reload

A file watcher is the same shape: watch on a worker, and on each change read
the file there and `call_soon` the rebuild. `examples/threadsafe_reload.py`
is a complete one that polls with `os.stat`; an event-driven watcher
(`watchfiles`, `watchdog`) calls `call_soon` from its own callback the same
way.

## The rule, and what enforces it

Using a `Window`, `Node`, or `App` from any thread but its own raises
`PanicException`, which derives from `BaseException`, so a bare `except
Exception` won't swallow the mistake. Dropping one on another thread is
safe: if Python's garbage collector frees a Tesserae Engine object on a worker, the
free itself happens on the object's own thread at its next frame.
