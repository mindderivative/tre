#!/usr/bin/env python3
"""M87 (tre issue #6): hot reload *inside* `App.run()`, driven by a
background file-watcher thread.

`examples/hot_reload.py` calls `poll_reload()` from a loop the script
controls -- which is impossible once `App.run()` takes over the main
thread. And `App`/`Window`/`View` may only be touched from the thread
that created them. `App.thread_handle()` is the bridge: a background
thread holds the handle, and `handle.call_soon(fn)` runs `fn` on the
event-loop thread, waking the loop even when it's idle.

The watcher does the file I/O on its own thread and hands the loop only
the part that must run there: `view.reconcile(source=text)`. It polls
`os.stat` to stay dependency-free; a real framework would use an
event-driven watcher (`watchfiles`, `watchdog`) in exactly the same
shape -- one `call_soon` per detected change.

What this script proves automatically (headless-CI-safe): a callable
queued before `run()` makes an edit on the first frame, standing in for
a developer saving the file; the watcher thread notices, reads it, and
queues the reconcile; `box`'s `corner_radius` changes in the live tree
while `App.run()` is still running. It works on a temporary copy of
`hot_reload.yaml`, so repeat runs leave no diff.

To try it by hand, drop `max_frames` from `app.run(...)` and edit the
temp file the script prints while the window is open.
"""

import shutil
import tempfile
import threading
from pathlib import Path

from tre import App, View, Window

source = Path(__file__).parent / "hot_reload.yaml"
workdir = Path(tempfile.mkdtemp(prefix="tre_threadsafe_reload_"))
view_path = workdir / "hot_reload.yaml"
shutil.copy(source, view_path)
print(f"watching {view_path}")

view = View(str(view_path))
window = Window.from_view(view, width=200, height=80, title="threadsafe_reload")
app = App()
app.add_window(window)
handle = app.thread_handle()

stop = threading.Event()
queued = threading.Event()
saved = threading.Event()
reloads = []


def signature(path):
    stat = path.stat()
    return (stat.st_mtime_ns, stat.st_size)


def watch(path, last):
    """Runs on a background thread. Never touches `view`/`window`
    directly -- only `handle`, the one thread-safe object."""
    while not stop.is_set():
        current = signature(path)
        if current != last:
            last = current
            text = path.read_text()  # file I/O stays off the UI thread

            def reload(text=text):
                view.reconcile(source=text)
                reloads.append(view.node("box").get("corner_radius"))

            handle.call_soon(reload)
            queued.set()
        stop.wait(0.01)


watcher = threading.Thread(target=watch, args=(view_path, signature(view_path)), daemon=True)
watcher.start()


def simulated_editor_save():
    view_path.write_text(view_path.read_text().replace("corner_radius: 0", "corner_radius: 16"))
    saved.set()
    # Demo-only: wait until the watcher has queued its reload. A
    # max_frames-bounded run counts idle frames too, and those take
    # microseconds, so without this the run could end before the
    # watcher's next poll. A real app never blocks here -- its run lasts
    # until the window closes.
    queued.wait(timeout=5)


handle.call_soon(simulated_editor_save)
app.run(max_frames=600)
stop.set()
watcher.join()

print(f"before reload: corner_radius=0.0, reloads seen inside App.run(): {reloads}")
if saved.is_set():
    assert reloads == [16.0], f"the edit ran on the first frame but never reloaded: {reloads}"
    print("threadsafe_reload.py: a file edit reached the live tree inside App.run()")
else:
    # No display reachable: `App.run()` returned before any frame.
    print("threadsafe_reload.py: no frames rendered (no display), nothing to reload")

shutil.rmtree(workdir)
