#!/usr/bin/env python3
"""A shell that stays up with no window (0.5.9, #238): `run(keep_alive=True)` and `LoopHandle.stop()`.

A desktop shell has to stay up when every monitor is set to show nothing, or the last output is
unplugged, and put a bar on each screen as it appears. This is the skeleton: one thin bar per
output (added from `output_added`, so a screen plugged in later gets one), the loop kept alive with
no window open, and a quit on SIGTERM or Ctrl-C through `signal.set_wakeup_fd` and a watcher
thread (a Python signal handler does not run on an idle loop).

It ends by itself after 3 seconds so it is safe to run anywhere; `--seconds 0` runs until a signal.
Without a display `run()` returns at once, and without a layer shell there are no bars to add, so
it still runs headless in CI. See docs/reference/app.md ("Staying up with no window").
"""

import signal
import socket
import sys
import threading

import tre
from tre import App, LayerWindow

seconds = 3.0
if "--seconds" in sys.argv:
    seconds = float(sys.argv[sys.argv.index("--seconds") + 1])

app = App()
handle = app.thread_handle()
bars = {}


def add_bar(output):
    bar = LayerWindow(f"example.keepalive.{output.name}", anchor=("top", "left", "right"),
                      size=(0, 24), exclusive_zone=0, output=output)
    bar.root.set(fill=(0x1D, 0x1B, 0x20, 0xF0))
    bar.on("closed", lambda e, name=output.name: (bars.pop(name, None), print(f"bar on {name} closed: {e.reason}")))
    bars[output.name] = bar
    app.add_window(bar)
    print(f"bar added on {output.name}")


if tre.supports("layer_shell"):
    app.on("output_added", add_bar)
    app.on("output_removed", lambda output: print(f"output {output.name} removed"))
    for output in app.outputs():
        add_bar(output)

# Quit on SIGTERM or Ctrl-C: the C-level handler writes to this socket at once, even though the
# main thread is idle in the event loop and would not run a Python handler.
reader, writer = socket.socketpair()
writer.setblocking(False)
signal.set_wakeup_fd(writer.fileno())
for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, lambda *_: None)  # a handler must exist for the fd to be written


def quit_on_signal():
    reader.recv(8)
    print("signal received: stopping")
    handle.stop()


threading.Thread(target=quit_on_signal, daemon=True).start()
if seconds > 0:
    threading.Timer(seconds, handle.stop).start()

app.run(keep_alive=True)
print("keep_alive.py: exited cleanly")
