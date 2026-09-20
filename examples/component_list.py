#!/usr/bin/env python3
"""M43 Phase 1 (§4, §5, §8, §16.2, §16.6): the real, first proof that
`View.instantiate(path, into)` embeds a component inside another view
with its own, separate `ViewModel` -- and that it supports multiple
simultaneous instances, each fully independent. "The essence of MVVM
and single page applications," per the user's own words scoping this
milestone.

Three `Card` components (`component_list_card.yaml`), each its own real
`CardViewModel`, are instantiated into one container inside
`component_list.yaml`. Each card's own `on_click` handler bumps *only*
its own `Signal`-bound label -- proving the shared `Tree` correctly
keeps every instance's own bindings/handlers scoped to just that
instance's real `NodeId`s, with zero cross-instance leakage, through
`tre`'s own existing dispatch/layout machinery (no `engine-core`
changes were needed for this at all).

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


class CardViewModel(ViewModel):
    def __init__(self, view):
        self._count = 0
        self.label = Signal("Count: 0")
        super().__init__(view)

    def bump(self):
        self._count += 1
        self.label.set(f"Count: {self._count}")


card_path = str(directory / "component_list_card.yaml")
cards = []
viewmodels = []
for _ in range(3):
    card = view.instantiate(card_path, container)
    vm = CardViewModel(card)
    cards.append(card)
    viewmodels.append(vm)

print(f"before any click: labels={[c.node('label').get_text() for c in cards]!r}")
assert [c.node("label").get_text() for c in cards] == ["Count: 0"] * 3

# Click only the second card's own button, twice -- through the owning
# View's real click() (Component has none of its own).
view.click(cards[1].node("button"))
view.click(cards[1].node("button"))

labels = [c.node("label").get_text() for c in cards]
print(f"after 2 clicks on card 1: labels={labels!r}")
assert labels == ["Count: 0", "Count: 2", "Count: 0"], (
    "only the clicked instance's own label should have changed -- multi-instance independence"
)

window = Window.from_view(view, width=260, height=200, title="Tesserae-style Component List")
app = App()
app.add_window(window)
app.run(max_frames=20)
print("component_list.py: exited cleanly after a real 20-frame render loop")
