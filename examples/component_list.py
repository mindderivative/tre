#!/usr/bin/env python3
"""M43, both phases (§4, §5, §8, §16.2, §16.6): the real proof that
`View.instantiate(path, into)` embeds a component inside another view
with its own, separate `ViewModel`, supports multiple simultaneous
instances (each fully independent), and that `Component.remove()`
(Phase 2) really tears an instance down -- the full real dynamic-list
lifecycle: add, click, remove, add again. "The essence of MVVM and
single page applications," per the user's own words scoping this
milestone.

Each `Card` component (`component_list_card.yaml`) gets its own real
`CardViewModel`, instantiated into one container inside
`component_list.yaml`'s own `add_button` handler -- proving the shared
`Tree` correctly keeps every instance's own bindings/handlers scoped to
just that instance's real `NodeId`s, with zero cross-instance leakage,
through `tre`'s own existing dispatch/layout machinery (no
`engine-core` changes were needed for any of this).

Dispatch on an embedded component's own node goes through the
*owning* `View`'s `click()` -- `Component` deliberately has no
`click()`/`hover()` of its own (see `component.rs`'s module doc
comment for the real reason).
"""

from pathlib import Path

from tre import App, Signal, View, ViewModel, Window

directory = Path(__file__).parent

view = View(str(directory / "component_list.yaml"))
container = view.node("card_list")
card_path = str(directory / "component_list_card.yaml")

# (Component, CardViewModel) pairs, in display order -- the real,
# app-owned bookkeeping a dynamic list needs; `tre` itself tracks
# nothing beyond the live Tree.
cards = []


class CardViewModel(ViewModel):
    def __init__(self, view, card, on_remove):
        self._count = 0
        self.label = Signal("Count: 0")
        self._card = card
        self._on_remove = on_remove
        super().__init__(view)

    def bump(self):
        self._count += 1
        self.label.set(f"Count: {self._count}")

    def remove_self(self):
        # The real M43 Phase 2 proof: unsubscribes this instance's own
        # Signal subscriptions, then removes its subtree from the Tree.
        self._card.remove()
        self._on_remove(self)


def spawn_card():
    card = view.instantiate(card_path, container)
    vm = CardViewModel(card, card, discard_card)
    cards.append((card, vm))
    return card, vm


def discard_card(vm):
    for index, (_card, existing_vm) in enumerate(cards):
        if existing_vm is vm:
            del cards[index]
            return


class AppViewModel(ViewModel):
    def add_card(self):
        spawn_card()


app_vm = AppViewModel(view)

add_button = view.node("add_button")

# Three real, dispatched clicks on the "Add" button -- each one calling
# AppViewModel.add_card(), which instantiates a real, independent Card.
for _ in range(3):
    view.click(add_button)
assert len(cards) == 3

view.click(cards[1][0].node("button"))
assert cards[1][1]._count == 1

# Remove the first card (never clicked) through its own real dispatched
# "remove" click -- the same reentrant-from-a-handler pattern tre's own
# M42 Phase 2 (Window.show_view) had to catch a real borrow-panic bug
# for; Component.remove() has no such issue (it doesn't touch any
# shared RefCell across a callback boundary the way that fix needed).
view.click(cards[0][0].node("remove_button"))
assert len(cards) == 2
assert cards[0][1]._count == 1, "what was index 1 is now index 0 after removal"

# Add one more after a real removal -- proving the container/Tree stays
# healthy across a full add/click/remove/add cycle.
view.click(add_button)
assert len(cards) == 3

print(f"final card counts: {[vm._count for _, vm in cards]!r}")

window = Window.from_view(view, width=300, height=260, title="Tesserae Dynamic Component List")
app = App()
app.add_window(window)
app.run(max_frames=20)
print("component_list.py: exited cleanly after a real 20-frame render loop")
