# `Signal` & `ViewModel`

Plain Python classes (not pyo3 bindings) implementing the reactive data
binding [declarative views](../../guide/declarative-views.md) use. See
that guide for a full walkthrough with real `view.yaml` examples.

## `Signal`

A minimal reactive value cell.

```python
count = Signal(0)
```

### `get`

**`get()`**

Returns the current value. When called during a binding's evaluation
(inside `View._attach`), also records this signal as a dependency of that
binding, so it re-evaluates whenever the signal changes.

### `set`

**`set(value)`**

Sets the value and notifies every subscriber — but only if `value`
actually differs from the current one. This change-detection is what
keeps a two-way binding's forward/reverse wiring from recursing forever.

### `update`

**`update(fn)`**

Sets the value to `fn(current_value)`, then notifies under the same
change-detection as `set`. The idiomatic "read, transform, write":

```python
clicks.update(lambda n: n + 1)
```

## `ViewModel`

Base class for wiring a [`View`](view.md)'s declared bindings/handlers to
real Python state.

```python
class CounterViewModel(ViewModel):
    def __init__(self, view):
        self.clicks = Signal(0)
        super().__init__(view)  # must run after clicks exists --
                                 # _attach evaluates every binding
                                 # immediately, so it needs to see the
                                 # real attributes it names

    def bump(self, event):
        self.clicks.update(lambda n: n + 1)


vm = CounterViewModel(View("counter.yaml"))
```

### `__init__`

**`__init__(view)`**

Stores `view` and calls `view._attach(self)`, which wires every declared
`bindings:`/`handlers:`/`two_way:` entry in the loaded YAML against this
instance. Subclasses must call `super().__init__(view)` **after** setting
up any `Signal` attributes the view's bindings reference — evaluation
happens immediately, synchronously, inside this call.
