#!/usr/bin/env python3
"""tre v2's final showcase demo (M27, `BUILD_TRACKER.md`) -- one real,
substantial screen added per phase, all combined into one running app
for the first time (not just each screen proven separately). Phase 1
(this file's own current scope, §11.2/§11.10): the shell and
persistent left nav rail every later screen plugs into.

Deliberately reuses two already-real, already-proven v2 composition
patterns rather than inventing new ones: `Window.build_shell` for the
chrome, and `examples/navigation.py`'s own real remove-old/build-new
screen-swap pattern for navigation (`Node.remove()` truly deletes a
node's subtree, not a soft hide -- re-showing a screen means rebuilding
it fresh, not re-attaching a detached one).

Phase 1's own two screens are plain placeholders -- Phases 2-4 replace
`SCREENS`' own placeholder builders with real content (an MD3 component
gallery, a motion/canvas screen, a data/layout screen), reusing this
same shell and nav mechanism unchanged.

What this script proves automatically (headless-CI-safe, no human
needed): a real shell (menu bar + persistent nav rail + one screen's
worth of content) with two real, distinct screens; a real dispatched
click on the second nav button genuinely switches the active screen
(checked against the script's own tracked state, not just "didn't
crash"); and every nav button is really reachable via keyboard Tab, in
the same order it was attached -- not just mouse-clickable.
"""

from tre import App, Window

WINDOW_WIDTH = 720
WINDOW_HEIGHT = 480
MENU_BAR_HEIGHT = 32
NAV_WIDTH = 160

MENU_BAR_BG = (0x21, 0x21, 0x21, 0xFF)
NAV_BG = (0x18, 0x18, 0x18, 0xFF)
NAV_ITEM_BG = (0x33, 0x33, 0x33, 0xFF)
NAV_ITEM_ACTIVE_BG = (0x67, 0x50, 0xA4, 0xFF)
SCREEN_BG = (0x12, 0x12, 0x12, 0xFF)

SCREEN_LABELS = {
    "components": "Components",
    "motion": "Motion & Canvas",
}
SCREEN_ORDER = ["components", "motion"]


def build_placeholder_screen(window, accent):
    """Phase 1's own stand-in for a real screen -- Phases 2-4 replace
    this with real, substantial content per screen. A screen is always
    sized to fill the real remaining space next to the nav rail.
    """
    screen = window.add_rect(
        background=SCREEN_BG, width=WINDOW_WIDTH - NAV_WIDTH, height=WINDOW_HEIGHT - MENU_BAR_HEIGHT
    )
    card = window.add_rect(background=accent, width=200, height=56, x=24, y=24)
    screen.add_child(card)
    return screen


SCREENS = {
    "components": lambda window: build_placeholder_screen(window, (0x67, 0x50, 0xA4, 0xFF)),
    "motion": lambda window: build_placeholder_screen(window, (0x03, 0xDA, 0xC6, 0xFF)),
}


def build_nav_button(window, y):
    btn = window.add_rect(background=NAV_ITEM_BG, width=NAV_WIDTH - 24, height=40, x=12, y=y)
    btn.enable_interaction()
    return btn


def build_showcase(window):
    menu_bar = window.add_rect(background=MENU_BAR_BG, width=WINDOW_WIDTH, height=MENU_BAR_HEIGHT)
    content = window.build_shell(menu_bar=menu_bar)

    # `content` defaults to a real Flex Row (taffy's own `Style::default()`
    # -- confirmed directly in its source, see PLAN.md) -- the nav rail
    # (attached first) and the screen area (attached second) lay out
    # side by side with no extra style needed.
    nav = window.add_rect(background=NAV_BG, width=NAV_WIDTH, height=WINDOW_HEIGHT - MENU_BAR_HEIGHT)
    content.add_child(nav)

    screen_area = window.add_rect(
        background=SCREEN_BG, width=WINDOW_WIDTH - NAV_WIDTH, height=WINDOW_HEIGHT - MENU_BAR_HEIGHT
    )
    content.add_child(screen_area)

    state = {"current_screen": None, "current_key": None}
    buttons = {}

    def show_screen(key):
        if state["current_screen"] is not None:
            state["current_screen"].remove()
        screen = SCREENS[key](window)
        screen_area.add_child(screen)
        state["current_screen"] = screen
        state["current_key"] = key
        for k, btn in buttons.items():
            btn.animate("background", NAV_ITEM_ACTIVE_BG if k == key else NAV_ITEM_BG, duration_ms=0)

    y = 12
    for key in SCREEN_ORDER:
        btn = build_nav_button(window, y)
        nav.add_child(btn)
        btn.set_on_click(lambda k=key: show_screen(k))
        buttons[key] = btn
        y += 48

    show_screen(SCREEN_ORDER[0])
    return state, buttons, show_screen


def main():
    window = Window(width=WINDOW_WIDTH, height=WINDOW_HEIGHT, title="tre v2 -- showcase")
    state, buttons, show_screen = build_showcase(window)

    assert state["current_key"] == SCREEN_ORDER[0], "the first screen must be shown at startup"

    # Real functional proof: a dispatched click on the second nav
    # button must actually switch the active screen.
    window.click(buttons[SCREEN_ORDER[1]])
    assert state["current_key"] == SCREEN_ORDER[1], "clicking a nav button must switch the active screen"
    print(f"nav click switched the active screen to {state['current_key']!r}")

    # Real Tab-order proof: every nav button is the only interactive
    # content built so far (no other node has a click handler yet), so
    # pressing Tab once per button must visit them in the same order
    # they were attached.
    for expected_key in SCREEN_ORDER:
        window.press_key("tab")
        assert buttons[expected_key].is_focused(), (
            f"Tab order must reach the {expected_key!r} nav button next"
        )
    print("keyboard Tab reaches every nav button, in order")

    app = App()
    app.add_window(window)
    app.run(max_frames=60)
    print("demo/showcase.py: exited cleanly after 60 frames (Phase 1: shell + nav scaffold)")


if __name__ == "__main__":
    main()
