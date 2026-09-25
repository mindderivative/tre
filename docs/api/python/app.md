# `App`

The top-level entry point — opens and drives one or more
[`Window`](window.md)s together.

## `App`

**`App()`**

Constructs an empty `App` with no registered windows.

```python
app = App()
```

## `add_window`

**`add_window(window)`**

Registers a `Window` to be opened the next time `run()` is called.
Calling this multiple times before `run()` opens multiple windows
together.

```python
app.add_window(window)
```

## `run`

**`run(max_frames=None)`**

The single blocking call that opens every registered window and ticks,
lays out, and renders each window's own tree every frame, independently,
until every window has closed.

```python
app.run()
app.run(max_frames=60)  # each window individually stops after 60 frames
```

- `max_frames`, when given, is a **per-window** budget, not a whole-app
  one — useful for headless/CI runs with no real display loop end.
- Raises `RuntimeError` if called with zero registered windows.
- Internally drives real input dispatch (pointer press/move/release,
  theme changes, clipboard copy/cut/paste, dock drag-and-drop, text-field
  click-to-position and drag-selection) and invokes any handlers
  registered via `Node.set_on_click`/`set_on_hover_enter`/
  `set_on_hover_exit`/`set_on_change`, plus accessibility-driven actions
  from a screen reader.
- If no GPU adapter is reachable, or no display is available, the process
  exits cleanly (not treated as an error) rather than raising.
- Installs a `tracing` log subscriber as early as possible — set
  `RUST_LOG` to control verbosity (e.g. `RUST_LOG=warn python app.py`).

## `thread_handle`

**`thread_handle() -> LoopHandle`**

`App`, `Window`, and `View` may only be used from the thread that
created them — pyo3 raises `PanicException` (a `BaseException`, not an
`Exception`) if another thread touches one. `thread_handle()` returns
the one object a background thread may use: a
[`LoopHandle`](#loophandle) onto this `App`'s event loop. Every handle
from one `App` shares the same queue.

```python
handle = app.thread_handle()
threading.Thread(target=watch_files, args=(handle,), daemon=True).start()
app.run()
```

## `LoopHandle`

### `call_soon`

**`call_soon(callback)`**

Queues `callback` (called with no arguments) to run on the `App`'s
event-loop thread and wakes the loop — including an idle one waiting
for input. There it can touch `View`/`Window`/`Node` exactly like an
input handler can:

```python
# on a background thread, after detecting a file change:
text = path.read_text()                              # I/O off the UI thread
handle.call_soon(lambda: view.reconcile(source=text))
```

- Safe from any thread, before, during, or after `run()`.
- Callbacks run in the order they were queued, at the top of the next
  frame. One queued before `run()` runs on the first frame; one queued
  after `run()` returns waits for a later `run()`.
- A callback that itself calls `call_soon` doesn't run again in the
  same frame — its new entry runs on the next one.
- An exception is logged the same way as one from an input handler and
  doesn't stop the loop or later callbacks.
- A callback that changes any window's tree gets that window redrawn,
  even if another window's frame ran it.
- Raises `TypeError` if `callback` isn't callable.

`examples/threadsafe_reload.py` is a complete, runnable file watcher
built on this.
