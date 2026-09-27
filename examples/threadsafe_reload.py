#!/usr/bin/env python3
"""Hot reload inside `App.run()`, driven by a background file-watcher
thread.

`App` and `Window` may only be touched from the thread that created them,
and once `App.run()` takes over the main thread, nothing else runs there.
`App.thread_handle()` is the bridge: a background thread holds the
handle, and `handle.call_soon(fn)` runs `fn` on the event-loop thread,
waking the loop even when it's idle.

The watcher does the file I/O on its own thread and hands the loop only
the part that must run there: applying the new settings to the tree. It
polls `os.stat` to stay dependency-free; a framework would use an
event-driven watcher (`watchfiles`, `watchdog`) in exactly the same shape
-- one `call_soon` per detected change, and a framework's screen
rebuild goes in the same `call_soon`.

What this script proves automatically (headless-CI-safe): a callable
queued before `run()` edits the settings file on the first frame,
standing in for a developer saving it; the watcher thread notices, reads
it, and queues the change; `box`'s `corner_radius` changes in the live
tree while `App.run()` is still running.

To try it by hand, drop `max_frames` from `app.run(...)` and edit the
file the script prints while the window is open.
"""

import json
import shutil
import tempfile
import threading
from pathlib import Path

from tre import App, Window

workdir = Path(tempfile.mkdtemp(prefix="tre_threadsafe_reload_"))
settings_path = workdir / "settings.json"
settings_path.write_text(json.dumps({"corner_radius": 0}))
print(f"watching {settings_path}")

window = Window(200, 80, "threadsafe_reload")
box = window.create("box", width=120, height=40, fill=(0x67, 0x50, 0xA4, 0xFF))
window.root.add_child(box)
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
    """Runs on a background thread. Never touches `window` or `box`
    directly -- only `handle`, the one thread-safe object."""
    while not stop.is_set():
        current = signature(path)
        if current != last:
            last = current
            settings = json.loads(path.read_text())  # file I/O stays off the UI thread

            def reload(settings=settings):
                box.set(**settings)
                reloads.append(box.get("corner_radius"))

            handle.call_soon(reload)
            queued.set()
        stop.wait(0.01)


watcher = threading.Thread(
    target=watch, args=(settings_path, signature(settings_path)), daemon=True
)
watcher.start()


def simulated_editor_save():
    settings_path.write_text(json.dumps({"corner_radius": 16}))
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
