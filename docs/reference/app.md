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

**Since 0.5.6**, it also works while `run()` is going: from a listener, a
`frame` handler or `LoopHandle.call_soon`. The window opens on the loop's next
turn. Closing it leaves the other windows running, and closing the last one
ends the run (`App.run` returns). Adding a window that is already open raises
`ValueError`; a window that was closed may be added again. If the new window's
GPU can't be set up it closes by itself and the error is logged (the call that
added it has already returned). A window still waiting to open when the last
open one closes is lost with the run. `max_frames` applies to the new window
on its own.

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
  one — useful for headless/CI runs that must end on their own.
- Raises `RuntimeError` if called with zero registered windows.
- Delivers real input — pointer, wheel, keyboard, text and input-method
  composition, window events, and screen-reader requests — through the
  same pipeline `window.simulate` uses, invoking the listeners it reaches.
- If no display is available, returns `None` without running, so headless
  scripts and CI can call it unconditionally.
- Raises `RuntimeError` if a window's GPU can't be set up -- no GPU
  adapter, no device, or a surface the adapter can't drive. (Before 0.4.0
  this exited the process with status 0.)
- Installs a `tracing` log subscriber as early as possible — set
  `RUST_LOG` to control verbosity (e.g. `RUST_LOG=warn python app.py`).

## `outputs`

**`outputs() -> list[Output]`** (0.5.8) — the screens the compositor has, top to bottom and left
to right, read now, so it works before `run()` (a shell decides how many bars to make first). Each
is an [`Output`](outputs.md). The list is empty where the platform cannot say: on anything but
Wayland, and where the compositor does not report connector names.

```python
for out in app.outputs():
    app.add_window(make_bar(output=out))
```

## `on`, `off` and `simulate`

**`on(event, handler)`**, **`off(event)`**, **`simulate(event, **fields)`** (0.5.8) — listeners for
changes in the set of screens: `output_added`, `output_removed` and `output_updated`. A handler
takes the [`Output`](outputs.md) (or nothing). There is one handler per event and a second `on`
replaces it. See [Outputs](outputs.md#events). `simulate` delivers an event as the compositor
would, for tests with no display.

## GPU memory

Windows share one GPU device (0.5.9, #220), and a device asks the driver's allocator for memory in
blocks. tre asks for small blocks (8 to 64 MiB, 4 to 32 MiB for host-visible ones), so a small
window holds a few tens of MiB of video memory (31 MiB for a 200 x 100 window on the machine it
was measured on). `wgpu`'s own default reserves 192 MiB the moment a device exists, whatever is
then put in it, and that was tre's behaviour before 0.5.8's follow-up (#219): **set
`TRE_GPU_MEMORY=performance` in the environment before the app starts to get it back** (any other
value keeps the small blocks). A large image or a big window still gets the memory it needs
either way; the setting only changes how much is reserved in advance. See
[GPU memory per window](../design/vram-per-window.md) for the measurements.

**One device for every window (0.5.9, #220).** The first window makes the `wgpu` instance, adapter
and device, and every later window (ordinary, layer surface or popup) draws with them, so it holds
only its own swapchain, render target and renderer: five 200 x 100 windows hold the 31 MiB one does
(they held 83 MiB), and open a little faster. Two things follow. A GPU error is reported to every
window on the device, once each, because `wgpu` does not say which window's draw caused it; and a
lost device is lost for all of them, so every window's `gpu_lost` listener hears it before the run
ends. A window whose surface the shared adapter cannot present to (a second GPU) gets a device of its
own. **Set `TRE_GPU_DEVICES=per-window` before the app starts for a device per window again** (`shared`,
or anything else, is the default). `frame_stats()["memory"]` reports the device, so with a shared device
every window reads the same numbers (`shared_device` says which case it is).

## `thread_handle`

**`thread_handle() -> LoopHandle`**

`App`, `Window`, `Node`, and every other Tesserae Engine object may only be used
from the thread that created them — using one from another thread
raises `PanicException` (a `BaseException`, not an `Exception`).
*Dropping* one there is safe: if Python's garbage collector frees a Tesserae Engine
object on a background thread, the actual free happens on its own thread
at the next frame or Tesserae Engine call. `thread_handle()` returns the one object a
background thread may use: a [`LoopHandle`](#loophandle) onto this
`App`'s event loop. Every handle from one `App` shares the same queue.

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
for input. There it can touch a `Window` and its `Node`s exactly like an
input handler can:

```python
# on a background thread, after detecting a file change:
text = path.read_text()                              # I/O off the UI thread
handle.call_soon(lambda: rebuild_screen(text))           # on the loop
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
