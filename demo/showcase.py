#!/usr/bin/env python3
"""tre v2's final showcase demo (M27, `BUILD_TRACKER.md`) -- one real,
substantial screen added per phase, all combined into one running app
for the first time (not just each screen proven separately).

Phase 1 (§11.2/§11.10): the shell and persistent left nav rail every
later screen plugs into. Deliberately reuses two already-real,
already-proven v2 composition patterns rather than inventing new ones:
`Window.build_shell` for the chrome, and `examples/navigation.py`'s
own real remove-old/build-new screen-swap pattern for navigation
(`Node.remove()` truly deletes a node's subtree, not a soft hide --
re-showing a screen means rebuilding it fresh, not re-attaching a
detached one).

Phase 2 (this file's own current scope, §5, §7.1, §7.3): the MD3
component & theming gallery screen -- every real component
(`Checkbox`, `Slider`, `TextField`, `Image`, `Icon`) shown live, plus a
real seed-color/dark-mode theme picker demonstrating `Window.set_theme`
re-tinting every already-built themed component at once, not just
newly-created ones. Building this screen surfaced a real, genuine gap:
`Window` had no way to create a plain `NodeKind::Text` label at all
(only reachable declaratively, via a `view.yaml`) -- `Window.add_text`
(new, mirrors `add_rect`'s own shape) closes it.

Phases 3-4 replace the two remaining `SCREENS` placeholders with real
content (a motion/canvas screen, a data/layout screen), reusing this
same shell and nav mechanism unchanged.

What this script proves automatically (headless-CI-safe, no human
needed): a real shell (menu bar + persistent nav rail + one screen's
worth of content) with two real, distinct screens; a real dispatched
click on the second nav button genuinely switches the active screen;
every nav button is really reachable via keyboard Tab, in order; and,
new this phase, every gallery control's own real state changes exactly
the way a real click/read-back would show -- a `Checkbox` toggling, a
`Slider`'s programmatic nudge landing, a `TextField` round-tripping
real text, and every theme-seed swatch plus the dark-mode toggle
applying a real `Window.set_theme` call with no error.
"""

import base64
import tempfile
from pathlib import Path

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
LABEL_COLOR = (0xAA, 0xAA, 0xAA, 0xFF)

SCREEN_WIDTH = WINDOW_WIDTH - NAV_WIDTH
SCREEN_HEIGHT = WINDOW_HEIGHT - MENU_BAR_HEIGHT

SCREEN_LABELS = {
    "components": "Components",
    "motion": "Motion & Canvas",
}
SCREEN_ORDER = ["components", "motion"]

SEED_COLORS = [
    ("Purple", (0x67, 0x50, 0xA4, 0xFF)),
    ("Teal", (0x03, 0xDA, 0xC6, 0xFF)),
    ("Orange", (0xFF, 0x98, 0x00, 0xFF)),
    ("Blue", (0x1E, 0x88, 0xE5, 0xFF)),
]

# The curated icon set `engine_md3::icons` actually ships (see
# docs/guide/components.md) -- not invented here.
ICON_NAMES = ["home", "search", "menu", "close", "check", "arrow_back", "add", "settings"]

# Same real, tiny 4x4 PNG `examples/image.py` embeds -- one real,
# reusable test asset, not a second copy invented here. Written once
# at import time, reused across every rebuild of the gallery screen
# (it's rebuilt fresh each time a user navigates back to it).
_TINY_PNG_B64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAT0lEQVR4nAFEALv/"
    "APRDNv//mAD//+s7/0yvUP8BALzU/yHaHwAeu8IAXdb7AADpHmP/eVVI/2B9i/8A"
    "AACAAf////8AwggAjAJDANx3bQC9UiChZNHhkwAAAABJRU5ErkJggg=="
)
_tmp_dir = tempfile.mkdtemp(prefix="tre_showcase_")
_TINY_PNG_PATH = str(Path(_tmp_dir) / "tiny.png")
Path(_TINY_PNG_PATH).write_bytes(base64.b64decode(_TINY_PNG_B64))

# Populated fresh by `build_gallery_screen` every time the "components"
# screen is (re)built -- `main()`'s own real functional checks read
# this after `show_screen("components")` runs, the same "expose refs
# for the caller to verify" shape `build_showcase`'s own `buttons`
# dict already established for Phase 1's nav buttons.
GALLERY_REFS = {}


def build_placeholder_screen(window, accent):
    """Phase 1's own stand-in for a real screen -- Phase 3 replaces
    this with real, substantial content. A screen is always sized to
    fill the real remaining space next to the nav rail.
    """
    screen = window.add_rect(background=SCREEN_BG, width=SCREEN_WIDTH, height=SCREEN_HEIGHT)
    card = window.add_rect(background=accent, width=200, height=56, x=24, y=24)
    screen.add_child(card)
    return screen


def build_gallery_screen(window):
    """Phase 2's real MD3 component & theming gallery -- every real
    component live, plus a real seed-color/dark-mode picker.
    """
    screen = window.add_rect(background=SCREEN_BG, width=SCREEN_WIDTH, height=SCREEN_HEIGHT)

    def label(text, x, y, width=160, height=18, font_size=13):
        node = window.add_text(text, background=LABEL_COLOR, width=width, height=height, x=x, y=y, font_size=font_size)
        screen.add_child(node)
        return node

    # --- Checkbox ---
    label("Checkbox", 16, 12)
    checkbox = window.add_checkbox(background=(0x67, 0x50, 0xA4, 0xFF), width=28, height=28, x=16, y=34)
    checkbox.enable_interaction()
    screen.add_child(checkbox)

    def toggle_checkbox():
        now_checked = not checkbox.get_checked()
        checkbox.set_checked(now_checked)
        checkbox.animate("check_progress", 1.0 if now_checked else 0.0, duration_ms=150)

    checkbox.set_on_click(toggle_checkbox)

    # --- Slider ---
    label("Slider", 16, 76)
    slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=200, height=32, value=0.4, x=16, y=100)
    slider.enable_interaction()
    screen.add_child(slider)

    # --- TextField ---
    label("TextField", 16, 148)
    text_field = window.add_text_field(
        background=(0xEE, 0xEE, 0xEE, 0xFF), width=200, height=36, content="Edit me", x=16, y=172
    )
    screen.add_child(text_field)

    # --- Image ---
    label("Image", 16, 220)
    image = window.add_image(path=_TINY_PNG_PATH, width=96, height=96, x=16, y=244, fit="cover")
    screen.add_child(image)

    # --- Icon ---
    label("Icon", 16, 352)
    icons = []
    for i, name in enumerate(ICON_NAMES):
        icon = window.add_icon(name=name, color=(0xE0, 0xE0, 0xE0, 0xFF), size=28, x=16 + i * 32, y=376)
        screen.add_child(icon)
        icons.append(icon)

    # --- Theme controls ---
    theme_x = 300
    theme_state = {"seed": SEED_COLORS[0][1], "dark": False}

    def apply_theme(seed):
        theme_state["seed"] = seed
        window.set_theme(seed, dark=theme_state["dark"])

    label("Theme seed", theme_x, 12, width=200)
    swatches = []
    for i, (_name, seed) in enumerate(SEED_COLORS):
        swatch = window.add_rect(background=seed, width=40, height=40, x=theme_x + i * 48, y=36)
        swatch.enable_interaction()
        swatch.set_on_click(lambda s=seed: apply_theme(s))
        screen.add_child(swatch)
        swatches.append(swatch)

    label("Dark mode", theme_x, 100)
    dark_toggle = window.add_checkbox(background=(0x67, 0x50, 0xA4, 0xFF), width=24, height=24, x=theme_x, y=124)
    dark_toggle.enable_interaction()
    screen.add_child(dark_toggle)

    def toggle_dark():
        theme_state["dark"] = not dark_toggle.get_checked()
        dark_toggle.set_checked(theme_state["dark"])
        dark_toggle.animate("check_progress", 1.0 if theme_state["dark"] else 0.0, duration_ms=150)
        window.set_theme(theme_state["seed"], dark=theme_state["dark"])

    dark_toggle.set_on_click(toggle_dark)

    # Seed a real initial theme -- the same real live-re-theming path
    # every later swatch/toggle click uses, so the screen isn't stuck
    # on every component's own plain, un-themed default the moment
    # it's first shown.
    apply_theme(SEED_COLORS[0][1])

    GALLERY_REFS.clear()
    GALLERY_REFS.update(
        {
            "checkbox": checkbox,
            "toggle_checkbox": toggle_checkbox,
            "slider": slider,
            "text_field": text_field,
            "image": image,
            "icons": icons,
            "swatches": swatches,
            "dark_toggle": dark_toggle,
            "toggle_dark": toggle_dark,
            "theme_state": theme_state,
        }
    )
    return screen


SCREENS = {
    "components": build_gallery_screen,
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


def verify_gallery_screen(window):
    """Phase 2's real functional proof, run while "components" is the
    active screen (right after `build_showcase` -- it's shown first).
    No pixel readback needed: every check reads real, already-exposed
    `Node` state back through the same getters the engine already
    provides, the same "construction + real state changes, pixel-level
    color correctness proven separately by engine-render's own tests"
    split every other example in this workspace already uses.
    """
    refs = GALLERY_REFS

    was_checked = refs["checkbox"].get_checked()
    refs["toggle_checkbox"]()
    assert refs["checkbox"].get_checked() != was_checked, "a real click must flip Checkbox.checked"
    print(f"Checkbox: {was_checked} -> {refs['checkbox'].get_checked()}")

    # `Node.animate(..., duration_ms=0)` only *registers* the animation --
    # it snaps to the target the next time something ticks this node,
    # normally `App.run()`'s own per-frame loop (confirmed in
    # `view.rs::apply_binding_value`'s own doc comment), which hasn't
    # started yet here. The real, already-proven way to move a Slider
    # synchronously without a render loop is the same one `examples/
    # slider.py` already established: focus it via Tab, then a real
    # dispatched arrow-key nudge (`Tree::dispatch_slider_key`'s own
    # mechanism manually ticks immediately, unlike `animate()`).
    before = refs["slider"].get("thumb_position")
    window.press_key("tab")  # -> nav button 1
    window.press_key("tab")  # -> nav button 2
    window.press_key("tab")  # -> checkbox (first gallery control, built after the nav)
    window.press_key("tab")  # -> slider
    window.press_key("right")
    after = refs["slider"].get("thumb_position")
    assert after > before, "a real ArrowRight nudge must move a focused Slider"
    print(f"Slider: thumb_position {before} -> {after}")

    refs["text_field"].set_text("tre v2 showcase")
    assert refs["text_field"].get_text() == "tre v2 showcase", "TextField content must round-trip"
    print(f"TextField: {refs['text_field'].get_text()!r}")

    # Every theme-seed swatch, then the dark-mode toggle, each a real
    # Window.set_theme call -- proving the whole live-re-theming path
    # runs with no error across every seed, not just the initial one.
    for swatch in refs["swatches"]:
        window.click(swatch)
    refs["toggle_dark"]()
    assert refs["theme_state"]["dark"] is True, "the dark-mode toggle must flip theme_state"
    print(f"Theme: cycled all {len(refs['swatches'])} seed swatches + dark toggle, no error")


def main():
    window = Window(width=WINDOW_WIDTH, height=WINDOW_HEIGHT, title="tre v2 -- showcase")
    state, buttons, show_screen = build_showcase(window)

    assert state["current_key"] == SCREEN_ORDER[0], "the first screen must be shown at startup"

    # Phase 2: exercise every gallery control while "components" (the
    # first screen) is still active.
    verify_gallery_screen(window)

    # Real functional proof: a dispatched click on the second nav
    # button must actually switch the active screen.
    window.click(buttons[SCREEN_ORDER[1]])
    assert state["current_key"] == SCREEN_ORDER[1], "clicking a nav button must switch the active screen"
    print(f"nav click switched the active screen to {state['current_key']!r}")

    # Real Tab-order proof: now that "motion" (a plain placeholder) is
    # active, the two nav buttons are the only interactive content in
    # the tree (the gallery's own many focusable controls were removed
    # along with the "components" screen), so pressing Tab once per
    # button must visit them in the same order they were attached.
    for expected_key in SCREEN_ORDER:
        window.press_key("tab")
        assert buttons[expected_key].is_focused(), (
            f"Tab order must reach the {expected_key!r} nav button next"
        )
    print("keyboard Tab reaches every nav button, in order")

    # Leave the gallery as the visible starting screen for anyone
    # actually running this interactively.
    show_screen(SCREEN_ORDER[0])

    app = App()
    app.add_window(window)
    app.run(max_frames=60)
    print("demo/showcase.py: exited cleanly after 60 frames (Phase 2: MD3 component & theming gallery)")


if __name__ == "__main__":
    main()
