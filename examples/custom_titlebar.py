#!/usr/bin/env python3
"""A window that draws its own title bar (0.5.0): an icon, a title, and
minimize, maximize/restore, and close buttons, on an undecorated window
with a drag region and a resize border.

- The bar is `window_region="drag"`: a press on it (or on the title)
  moves the window, and a double-click maximizes. Its buttons are
  focusable with `click` listeners, so they stay buttons.
- A 6 px resize border, and a minimum size so the buttons never crush.
- The maximize button swaps to a restore icon on the `maximized` event,
  and the title dims on `active` when the window loses focus.
- On macOS the OS keeps its own traffic lights: the bar leaves room for
  them (`titlebar_inset`) and hides its buttons (`native_controls`).

The script checks all of that headlessly with `window.simulate`, then
opens the window for 180 frames. `--watch` keeps it open, for checking the
move, resize, and double-click by hand on a real desktop; `--menu` also
turns on the OS's window menu on a right-click (`system_menu`).
Headless-CI-safe: `App.run()` returns quietly where no display is
reachable.
"""

import sys

from tre import App, Event, Window

BAR_HEIGHT = 40
BUTTON_WIDTH = 46
SURFACE = (0xFE, 0xF7, 0xFF, 0xFF)
BAR = (0xF3, 0xED, 0xF7, 0xFF)
ACCENT = (0x67, 0x50, 0xA4, 0xFF)
TEXT = (0x1D, 0x1B, 0x20, 0xFF)
TEXT_INACTIVE = (0x1D, 0x1B, 0x20, 0x80)
HOVER = (0x1D, 0x1B, 0x20, 0x14)
CLOSE_HOVER = (0xC4, 0x2B, 0x1C, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)
CLEAR = (0, 0, 0, 0)

# Icons in a 10x10 view box, stroked 1 px.
MINIMIZE = "M1 5 H9"
MAXIMIZE = "M1.5 1.5 H8.5 V8.5 H1.5 Z"
RESTORE = "M3.5 1.5 H8.5 V6.5 M1.5 3.5 H6.5 V8.5 H1.5 Z"
CLOSE = "M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5"


class TitleBar:
    """The bar: an app icon, the title, and the three window buttons."""

    def __init__(self, window, title):
        self.window = window
        self.node = window.create(
            "box", width="100%", height=BAR_HEIGHT, flex_shrink=0,
            align_items="center", gap=10, padding_left=12, fill=BAR,
            window_region="drag",
        )
        icon = window.create("box", width=16, height=16, corner_radius=4, fill=ACCENT)
        self.title = window.create(
            "text", text=title, font_size=13, font_weight=500, height=18,
            flex_grow=1, fill=TEXT,
        )
        # Pressing the icon or the title drags too: neither is interactive.
        self.node.add_child(icon)
        self.node.add_child(self.title)

        self.icons = {}  # a button's label -> its icon (Nodes take no attributes)
        self.minimize = self.button("Minimize", MINIMIZE, HOVER, window.minimize)
        self.maximize = self.button("Maximize", MAXIMIZE, HOVER, self.toggle_maximize)
        self.close = self.button("Close", CLOSE, CLOSE_HOVER, window.close)
        self.buttons = [self.minimize, self.maximize, self.close]

        window.on("maximized", self.show_maximized)
        window.on("active", lambda e: self.title.animate(
            "fill", TEXT if e.active else TEXT_INACTIVE, 150))
        window.on("titlebar_inset", lambda e: self.fit_native_controls(e.titlebar_inset))
        self.fit_native_controls()

    def button(self, label, icon_data, hover, action):
        button = self.window.create(
            "box", width=BUTTON_WIDTH, height=BAR_HEIGHT, align_items="center",
            justify_content="center", role="button", label=label, focusable=True,
        )
        icon = self.window.create(
            "path", data=icon_data, view_box=(0, 0, 10, 10), width=10, height=10,
            stroke_color=TEXT, stroke_width=1, hit_testable=False,
        )
        button.add_child(icon)
        self.icons[label] = icon
        button.on("click", action)
        # The close button turns red, and its icon white, under the pointer.
        on_red = hover == CLOSE_HOVER
        button.on("pointer_enter", lambda: (
            button.animate("fill", hover, 100),
            on_red and icon.animate("stroke_color", WHITE, 100)))
        button.on("pointer_leave", lambda: (
            button.animate("fill", CLEAR, 100),
            on_red and icon.animate("stroke_color", TEXT, 100)))
        self.node.add_child(button)
        return button

    def toggle_maximize(self):
        if self.window.get("maximized"):
            self.window.restore()
        else:
            self.window.maximize()

    def show_maximized(self, event: Event):
        self.icons["Maximize"].set(data=RESTORE if event.maximized else MAXIMIZE)
        self.maximize.set(label="Restore" if event.maximized else "Maximize")

    def fit_native_controls(self, inset=None):
        # macOS: clear the traffic lights and hide our own buttons; the
        # inset is (0, 0) and native_controls False everywhere else.
        _, width = inset or self.window.get("titlebar_inset")
        self.node.set(padding_left=max(12.0, width))
        native = self.window.get("native_controls")
        for button in self.buttons:
            button.set(visible=not native)


def main():
    watch = "--watch" in sys.argv
    window = Window(width=640, height=400, title="Custom title bar", decorations=False)
    window.set(resize_border=6, min_width=360, min_height=220,
               system_menu="--menu" in sys.argv)
    window.root.set(flex_direction="vertical", padding=0, gap=0, fill=SURFACE)
    bar = TitleBar(window, "Custom title bar")
    body = window.create("box", width="100%", flex_grow=1, padding=24)
    body.add_child(window.create(
        "text", width="100%", height=40, font_size=14, fill=TEXT,
        text="Drag the bar to move the window, double-click it to maximize, "
             "and drag an edge to resize.",
    ))
    window.root.add_child(bar.node)
    window.root.add_child(body)

    # -- headless checks ---------------------------------------------------------
    window.advance(0)  # pin the clock, for the double-click and animations
    heard = []
    bar.node.on("pointer_cancel", lambda: heard.append("drag"))
    bar.minimize.on("pointer_cancel", lambda: heard.append("button dragged"))

    # A press on the title is the window's: it moves, and nothing clicks.
    window.simulate("pointer_down", node=bar.title)
    window.simulate("pointer_up", node=bar.title)
    assert heard == ["drag"], heard

    # The buttons are buttons. Before `App.run()`, maximize and restore set
    # how the window opens.
    window.simulate("click", node=bar.maximize)
    assert window.get("maximized") is True
    window.simulate("click", node=bar.maximize)
    assert window.get("maximized") is False
    assert "button dragged" not in heard

    # On an open window the OS reports the change as the `maximized`
    # event, which swaps the icon.
    # (Path data reads back normalized, so the icon is compared with itself.)
    maximize_icon = bar.icons["Maximize"].get("data")
    window.simulate("maximized", maximized=True)
    assert bar.icons["Maximize"].get("data") != maximize_icon
    assert bar.maximize.get("label") == "Restore"
    window.simulate("maximized", maximized=False)
    assert bar.icons["Maximize"].get("data") == maximize_icon

    # A double-click on the bar maximizes, as a native title bar's does.
    window.advance(1000)
    for _ in range(2):
        window.simulate("pointer_down", node=bar.title)
        window.simulate("pointer_up", node=bar.title)
        window.advance(100)
    assert window.get("maximized") is True
    window.restore()

    # Losing focus dims the title.
    window.simulate("active", active=True)
    window.simulate("active", active=False)
    window.advance(150)
    assert bar.title.get("fill") == TEXT_INACTIVE

    # macOS's traffic lights: room is left for them (reported here by
    # hand; the live window reports its own).
    window.simulate("titlebar_inset", height=28, width=78)
    assert bar.node.get("padding_left") == 78.0
    print("custom_titlebar.py: checks passed")

    app = App()
    app.add_window(window)
    app.run(max_frames=None if watch else 180)
    print("custom_titlebar.py: exited cleanly")


if __name__ == "__main__":
    main()
