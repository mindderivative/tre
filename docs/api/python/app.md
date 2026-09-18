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

`App` has no other public methods.
