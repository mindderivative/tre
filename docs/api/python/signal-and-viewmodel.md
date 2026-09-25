# Reactivity: `Signal`, `Computed`, `Effect` & `ViewModel`

Plain Python (not pyo3 bindings) implementing the reactive data binding
[declarative views](../../guide/declarative-views.md) use. See that
guide for a full walkthrough.

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

## `Computed`

**`Computed(fn)`**

A derived, cached value. `fn` runs once immediately, recording every
`Signal`/`Computed` it reads; after that it re-runs only when one of
those dependencies actually changes, and notifies its own subscribers
only when its result changes.

```python
first = Signal("Ada")
last = Signal("Lovelace")
full = Computed(lambda: f"{first.get()} {last.get()}")
full.get()  # "Ada Lovelace"
```

Read it with `.get()`. It behaves like a read-only `Signal` everywhere,
so a `{{ }}` binding, another `Computed`, or an `Effect` can depend on
it. Dependencies are re-discovered on every run, so a conditional read
(`a.get() if flag.get() else b.get()`) tracks whichever branch ran.

## `Effect`

**`Effect(fn)`**

Runs `fn` once immediately, then again every time a `Signal`/`Computed`
it read last time changes. For side effects — logging, pushing data
elsewhere, or driving a [`View.reconcile`](view.md#reconcile):

```python
effect = Effect(lambda: view.reconcile(spec=build_spec(items.get())))
```

**`dispose()`** unsubscribes from every dependency and stops future
runs — call it when whatever owns the `Effect` goes away.

## `batch`

**`batch(fn)`**

Runs `fn()` with every `Signal` write inside it deferred, then notifies
each affected subscriber once — a `Computed` or `Effect` depending on
two Signals both written inside the batch re-runs once, not twice.
Returns `fn`'s result. Batches nest; notifications fire when the
outermost one finishes.

```python
batch(lambda: (first.set("Grace"), last.set("Hopper")))
```

## `untrack`

**`untrack(fn)`**

Runs `fn()` without its `.get()` reads being recorded by the enclosing
`Computed`/`Effect`/binding — read a value without depending on it.
Returns `fn`'s result.

```python
Effect(lambda: log(f"{count.get()} (theme: {untrack(theme.get)})"))
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

    def bump(self):
        self.clicks.update(lambda n: n + 1)


vm = CounterViewModel(View(spec=counter_spec))
```

### `__init__`

**`__init__(view)`**

Stores `view` and calls `view._attach(self)`, which wires every declared
`bindings:`/`handlers:`/`two_way:` entry in the view against this
instance. Subclasses must call `super().__init__(view)` **after** setting
up any `Signal` attributes the view's bindings reference — evaluation
happens immediately, synchronously, inside this call.
