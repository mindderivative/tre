#!/usr/bin/env python3
"""M45 (§16.2): the real, end-to-end proof that `Computed`/`Effect`/
`batch()` compose correctly with the existing `Signal`/`View`/binding
stack, through a genuine render loop -- not just in isolated pytest
coverage.

`price`/`quantity` are plain `Signal`s; `total` is a `Computed` that
derives `price.get() * quantity.get()`; `total_label` is a *second*
`Computed`, formatting `total`'s own value as a display string -- a real
`Computed`-of-`Computed` chain. The YAML `{{ total_label.get() }}`
binding points directly at that second `Computed`, proving the same
real duck-typed composition `tests/test_reactivity.py` already proves:
`View._attach`'s own subscribe call site never type-checks what it
subscribes to, so a `Computed` needs no special Rust-side support to
back a live binding.

Each click on the button bumps *both* `price` and `quantity` inside one
`batch()` call -- this script wraps `total`'s own `_recompute` to prove
it only fires *once* per click, not twice (one per Signal), the real
bug `batch()`'s own implementation caught and fixed by actually running
its own test suite (see `BUILD_TRACKER.md`'s M45 section).

`max_frames=30` matches this workspace's own headless-CI-safe
convention, same as `live_view.py`.
"""

from pathlib import Path

from tre import App, Computed, Effect, Signal, View, ViewModel, Window, batch

directory = Path(__file__).parent
view = View(str(directory / "reactivity.yaml"))


class CartViewModel(ViewModel):
    def __init__(self, view):
        self.price = Signal(10)
        self.quantity = Signal(1)
        self.total = Computed(lambda: self.price.get() * self.quantity.get())
        self.total_label = Computed(lambda: f"Total: ${self.total.get()}")

        # Proves `total` recomputes exactly once per batched click, not
        # once per Signal it reads -- wraps the real `_recompute`
        # rather than re-implementing it.
        self.recompute_count = 0
        original_recompute = self.total._recompute

        def counting_recompute():
            self.recompute_count += 1
            original_recompute()

        self.total._recompute = counting_recompute
        for dependency in list(self.total._dependencies):
            dependency._unsubscribe(original_recompute)
            dependency._subscribe(counting_recompute)

        # A real Effect, for its own side effect only (no downstream
        # binding reads it) -- logs every genuine total change.
        self.log = []
        self.total_effect = Effect(lambda: self.log.append(self.total.get()))

        super().__init__(view)

    def bump_both(self):
        def writes():
            self.price.update(lambda p: p + 5)
            self.quantity.update(lambda q: q + 1)

        batch(writes)


vm = CartViewModel(view)
button = view.node("bump_button")
label = view.node("total_label")

print(f"before any click: {label.get_text()!r}, total={vm.total.get()}, log={vm.log}")
assert label.get_text() == "Total: $10"
assert vm.log == [10]

for _ in range(3):
    view.click(button)

print(
    f"after 3 clicks: {label.get_text()!r}, total={vm.total.get()}, "
    f"recompute_count={vm.recompute_count}, log={vm.log}"
)
assert vm.price.get() == 25 and vm.quantity.get() == 4
assert vm.total.get() == 100
assert label.get_text() == "Total: $100"
# The wrapper is installed *after* Computed.__init__'s own initial
# recompute already ran (using the real, unwrapped method), so it only
# ever counts the 3 batched-click recomputes, not that first one too.
assert vm.recompute_count == 3, "exactly 1 recompute per batched click, not 2 (one per Signal)"
assert vm.log == [10, 30, 60, 100], "the Effect must see every genuine total change, in order"

window = Window.from_view(view, width=260, height=140, title="Tesserae Reactivity")
app = App()
app.add_window(window)
app.run(max_frames=30)
print("reactivity.py: exited cleanly after a real 30-frame render loop")
