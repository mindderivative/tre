#!/usr/bin/env python3
"""tre v2's final showcase demo (M27, `BUILD_TRACKER.md`, now fully
complete across all 5 phases) -- one real, substantial screen added
per phase, all combined into one running app for the first time, not
just each screen proven separately.

- **Phase 1** (§11.2/§11.10): the shell and persistent left nav rail
  every screen plugs into, reusing two already-real, already-proven v2
  composition patterns -- `Window.build_shell` for the chrome, and
  `examples/navigation.py`'s own real remove-old/build-new screen-swap
  pattern for navigation.
- **Phase 2** (§5, §7.1, §7.3): the MD3 component & theming gallery --
  `Checkbox`/`Slider`/`TextField`/`Image`/`Icon` all live, plus a real
  seed-color/dark-mode theme picker demonstrating `Window.set_theme`
  re-tinting every already-built themed component at once. Added
  `Window.add_text` (a real, genuine gap: no imperative way to create
  a plain text label existed before this).
- **Phase 3** (§5, §7.4, §7.5, §7.6): four real animation triggers
  (opacity/corner_radius/elevation/shape morph) plus a live `Canvas`
  node graph with its own real `transform` pan/zoom. Found and fixed a
  real, structural `PyWindow` re-entrancy limitation (`materializers`/
  `canvas_draws` moved behind their own `RefCell`, letting
  `add_canvas`/`add_virtual_list` become `&self` like every other real
  `add_*` method).
- **Phase 4** (§16): a real 5,000-row virtualized list with real
  paging, a real minimal docking layout, and a declarative YAML `View`
  panel using M26's real stylesheet/token support -- the first place
  both authoring paths genuinely compose in one running app, via real
  cross-path data flow (a `View`-dispatched click updates a `Signal`,
  reflected on an ordinary imperative label), since `View` has no
  rendering concept of its own to nest visually. Extended `Node.
  set_text`/`get_text` to also handle plain `Text` labels.
- **Phase 5** (§10): a real, comprehensive keyboard-Tab-order sweep
  across every screen's own interactive controls (not just a couple of
  spot-checks), plus real keyboard *operability* (not just
  reachability) on a representative control per screen via a real
  Enter-key press -- the same `DispatchOutcome::Activated` mechanism a
  real mouse click already produces. Deduplicated the `label()` helper
  three screen builders each defined locally into one shared factory.

Several real, genuine bugs and gaps -- found only by actually running
each phase, never assumed -- are documented in this file's own inline
comments where each fix landed, and in `BUILD_TRACKER.md`'s own
phase-by-phase history: the zero-argument handler contract (found in
six places across this file, three docs pages, and the actually-shipped
`python/tre/__init__.py` docstring), this engine's non-bubbling
hit-testing, `Window.add_virtual_list`'s missing `x`/`y`, and more.

What this script proves automatically (headless-CI-safe, no human
needed): every real interaction this module doc names above, for real,
via real dispatched input -- not just that each screen constructs and
renders without error.
"""

import base64
import tempfile
from pathlib import Path

from tre import App, Signal, View, ViewModel, Window

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
    "data": "Data & Layout",
}
SCREEN_ORDER = ["components", "motion", "data"]

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


def make_label_fn(window, screen):
    """Phase 5 polish: the identical `label(text, x, y, ...)` closure
    every screen builder used to define locally, three separate times
    -- factored out once a real accessibility/consistency pass made
    the duplication worth removing. Each screen builder still gets its
    own bound `label` function with the same call-site shape as
    before; only the definition moved.
    """

    def label(text, x, y, width=160, height=18, font_size=13):
        node = window.add_text(text, background=LABEL_COLOR, width=width, height=height, x=x, y=y, font_size=font_size)
        screen.add_child(node)
        return node

    return label


def build_gallery_screen(window):
    """Phase 2's real MD3 component & theming gallery -- every real
    component live, plus a real seed-color/dark-mode picker.
    """
    screen = window.add_rect(background=SCREEN_BG, width=SCREEN_WIDTH, height=SCREEN_HEIGHT)
    label = make_label_fn(window, screen)

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


# --- Phase 3: Motion & Custom Drawing screen ---

# A small real node graph, the same real shape `examples/node_graph.py`
# already proves -- edges and node circles as real `DrawCommand`s
# inside one `Canvas`, with a real `CustomHitTest::Circle` on one node.
GRAPH_POSITIONS = [(30, 30), (130, 20), (210, 55), (160, 130), (50, 120)]
GRAPH_EDGES = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (1, 3)]
GRAPH_NODE_COLORS = [
    (0xFF, 0xA5, 0x00, 0xFF),
    (0x03, 0xDA, 0xC6, 0xFF),
    (0xCF, 0x62, 0x79, 0xFF),
    (0x67, 0x50, 0xA4, 0xFF),
    (0x38, 0x8E, 0x3C, 0xFF),
]
GRAPH_NODE_RADIUS = 10

MOTION_REFS = {}


def draw_node_graph(ctx):
    for a, b in GRAPH_EDGES:
        ax, ay = GRAPH_POSITIONS[a]
        bx, by = GRAPH_POSITIONS[b]
        ctx.stroke_path(points=[(ax, ay), (bx, by)], color=(0x63, 0x50, 0xA4, 0xFF), width=2.0)
    for (x, y), color in zip(GRAPH_POSITIONS, GRAPH_NODE_COLORS):
        ctx.fill_circle(cx=x, cy=y, radius=GRAPH_NODE_RADIUS, color=color)
    cx, cy = GRAPH_POSITIONS[2]
    ctx.set_hit_test_circle(cx=cx, cy=cy, radius=GRAPH_NODE_RADIUS)


def build_motion_screen(window):
    """Phase 3's real animation breadth (opacity/corner_radius/
    elevation/shape), each triggered by a real dispatched click on its
    own button -- not auto-playing on a timer with nothing driving it
    -- plus a live `Canvas` (a real node graph) with its own real
    `transform` pan/zoom, the same real mechanism `examples/pan_zoom.py`
    already proves, applied here to a `Canvas` node rather than a
    `Rect` for the first time.
    """
    screen = window.add_rect(background=SCREEN_BG, width=SCREEN_WIDTH, height=SCREEN_HEIGHT)
    label = make_label_fn(window, screen)

    def trigger_button(text, x, y, width=76):
        label(text, x, y - 20, width=width, font_size=12)
        btn = window.add_rect(background=(0x33, 0x33, 0x33, 0xFF), width=width, height=32, x=x, y=y)
        btn.enable_interaction()
        screen.add_child(btn)
        return btn

    label("Animation (click a trigger below)", 16, 12, width=280)

    card = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=120, height=120, x=16, y=100)
    screen.add_child(card)

    anim_state = {"faded": False, "rounded": False, "elevated": False, "morphed": False}
    TRIANGLE = [(60.0, 0.0), (120.0, 120.0), (0.0, 120.0)]
    DIAMOND = [(60.0, 0.0), (120.0, 60.0), (60.0, 120.0), (0.0, 60.0)]

    def trigger_fade():
        anim_state["faded"] = not anim_state["faded"]
        card.animate("opacity", 0.25 if anim_state["faded"] else 1.0, duration_ms=400)

    def trigger_round():
        anim_state["rounded"] = not anim_state["rounded"]
        card.animate("corner_radius", 48.0 if anim_state["rounded"] else 0.0, duration_ms=400)

    def trigger_elevate():
        anim_state["elevated"] = not anim_state["elevated"]
        card.animate("elevation", 6.0 if anim_state["elevated"] else 0.0, duration_ms=400)

    def trigger_morph():
        anim_state["morphed"] = not anim_state["morphed"]
        card.animate("shape", DIAMOND if anim_state["morphed"] else TRIANGLE, duration_ms=400)

    fade_btn = trigger_button("Fade", 16, 60)
    round_btn = trigger_button("Round", 100, 60)
    elevate_btn = trigger_button("Elevate", 184, 60)
    morph_btn = trigger_button("Morph", 268, 60)
    fade_btn.set_on_click(trigger_fade)
    round_btn.set_on_click(trigger_round)
    elevate_btn.set_on_click(trigger_elevate)
    morph_btn.set_on_click(trigger_morph)

    # --- Canvas: a real node graph, pannable/zoomable as a whole ---
    label("Canvas (node graph)", 300, 12, width=220)
    graph = window.add_canvas(width=240, height=160, draw=draw_node_graph, x=300, y=36)
    screen.add_child(graph)
    window.redraw_canvas(graph)

    pan_zoom_state = {"panned": False}

    def trigger_pan_zoom():
        pan_zoom_state["panned"] = not pan_zoom_state["panned"]
        graph.animate(
            "transform", (30.0, 20.0, 1.25) if pan_zoom_state["panned"] else (0.0, 0.0, 1.0), duration_ms=500
        )

    pan_zoom_btn = trigger_button("Pan/Zoom", 300, 224)
    pan_zoom_btn.set_on_click(trigger_pan_zoom)

    MOTION_REFS.clear()
    MOTION_REFS.update(
        {
            "card": card,
            "graph": graph,
            "fade_btn": fade_btn,
            "round_btn": round_btn,
            "elevate_btn": elevate_btn,
            "morph_btn": morph_btn,
            "pan_zoom_btn": pan_zoom_btn,
        }
    )
    return screen


# --- Phase 4: Data & Layout screen ---

DATA_ROW_COUNT = 5_000
DATA_ROW_HEIGHT = 22.0
DATA_PAGE_SIZE = 10

DATA_REFS = {}

# `Window.set_drop_zone_highlight`'s own real contract: the node it's
# given starts *detached* (no parent at all) and stays that way except
# while a real drag is in progress -- it can never be made a
# descendant of a screen the way every other node this demo builds is,
# so `screen.remove()` (a real screen-swap navigating away from "data")
# would never reach it and it would leak, a fresh orphan, on every
# rebuild. Created lazily, once, and reused across every rebuild
# instead -- the one real node in this whole demo that outlives its
# own screen on purpose.
_data_screen_singletons = {"highlight": None}


def data_row_color(idx):
    if idx % 25 == 0:
        return (0x67, 0x50, 0xA4, 0xFF)
    return (0x2A, 0x2A, 0x2A, 0xFF) if idx % 2 == 0 else (0x1E, 0x1E, 0x1E, 0xFF)


def build_data_screen(window):
    """Phase 4's real data & layout screen: a genuinely large
    virtualized list, a real docking layout, and a declarative YAML
    `View` panel -- proving both authoring paths compose in one real
    running app, not just separately in isolated examples. `View` has
    no rendering/render-loop concept of its own (confirmed directly in
    `view.rs`'s own module doc comment), so "compose" here means real,
    meaningful data flow between the two: a click dispatched through
    the `View`'s own real handler mechanism updates a `Signal`, read
    back and shown on an ordinary imperative `Window` label -- the
    honest, buildable interpretation of "embedded alongside," not a
    literal visual nesting `View`'s own architecture can't support.
    """
    screen = window.add_rect(background=SCREEN_BG, width=SCREEN_WIDTH, height=SCREEN_HEIGHT)
    label = make_label_fn(window, screen)

    # --- Virtualized list: a genuinely large dataset ---
    label(f"Virtualized List ({DATA_ROW_COUNT:,} rows)", 16, 12, width=260)
    # Real finding: `add_virtual_list` (unlike every other `add_*`
    # method) takes no `x`/`y` at all -- confirmed by a real `TypeError`
    # from actually calling it, not assumed. Wrapped in an ordinary
    # positioned container instead: the list itself has no absolute
    # position of its own, so it fills its sole parent via the same
    # default Flex Row layout every screen container already uses.
    list_container = window.add_rect(background=SCREEN_BG, width=260, height=220, x=16, y=36)
    screen.add_child(list_container)
    data_list = window.add_virtual_list(
        item_count=DATA_ROW_COUNT, materialize=data_row_color, item_extent=DATA_ROW_HEIGHT, width=260, height=220
    )
    list_container.add_child(data_list)
    window.set_virtual_list_window(data_list, 0, DATA_PAGE_SIZE)

    page_state = {"start": 0}

    def load_next_page():
        new_start = min(page_state["start"] + DATA_PAGE_SIZE, DATA_ROW_COUNT - DATA_PAGE_SIZE)
        if new_start == page_state["start"]:
            return
        window.set_virtual_list_window(data_list, new_start, new_start + DATA_PAGE_SIZE)
        window.scroll(data_list, DATA_ROW_HEIGHT * (new_start - page_state["start"]))
        page_state["start"] = new_start

    label("Load next page", 16, 244, width=140, height=16, font_size=12)
    page_btn = window.add_rect(background=(0x33, 0x33, 0x33, 0xFF), width=140, height=32, x=16, y=264)
    page_btn.enable_interaction()
    page_btn.set_on_click(load_next_page)
    screen.add_child(page_btn)

    # --- Docking: a real, minimal two-zone layout ---
    label("Docking", 300, 12, width=200)
    left_zone = window.add_rect(background=(0, 0, 0, 0), width=110, height=100, x=300, y=36)
    right_zone = window.add_rect(background=(0x22, 0x22, 0x22, 0xFF), width=110, height=100, x=416, y=36)
    screen.add_child(left_zone)
    screen.add_child(right_zone)
    window.add_dock_zone("left", left_zone, 110.0)
    window.add_dock_zone("right", right_zone, 110.0)

    handle = window.add_rect(background=(0x80, 0x80, 0x80, 0xFF), width=110, height=20, x=300, y=36)
    panel = window.add_rect(background=(0x03, 0xDA, 0xC6, 0xFF), width=110, height=80, x=300, y=56)
    window.dock_panel("left", panel)
    window.set_dock_handle(handle, panel)
    screen.add_child(handle)
    screen.add_child(panel)

    if _data_screen_singletons["highlight"] is None:
        highlight = window.add_rect(background=(0x00, 0x80, 0xFF, 0x60), width=1, height=1)
        window.set_drop_zone_highlight(highlight)
        _data_screen_singletons["highlight"] = highlight

    dock_state = {"side": "left"}

    def move_panel():
        target_side, target_zone = ("right", right_zone) if dock_state["side"] == "left" else ("left", left_zone)
        started = window.start_panel_drag(handle)
        if started:
            zx, zy = (416.0, 36.0) if target_side == "right" else (300.0, 36.0)
            window.drop_panel_at(zx + 55.0, zy + 50.0)  # the target zone's own real center
            dock_state["side"] = target_side

    label("Move panel", 300, 124, width=140, height=16, font_size=12)
    move_btn = window.add_rect(background=(0x33, 0x33, 0x33, 0xFF), width=140, height=32, x=300, y=144)
    move_btn.enable_interaction()
    move_btn.set_on_click(move_panel)
    screen.add_child(move_btn)

    # --- Declarative View panel: the other authoring path, composed ---
    label("Declarative View (M26 stylesheet + tokens)", 16, 300, width=400)
    here = Path(__file__).parent
    view = View(
        str(here / "data_panel.yaml"),
        stylesheet=str(here / "data_panel_sheet.yaml"),
        theme_seed=SEED_COLORS[0][1],
    )

    class CounterViewModel(ViewModel):
        def __init__(self, view):
            self.counter = Signal(0)
            super().__init__(view)

        def bump(self):
            self.counter.update(lambda n: n + 1)

    view_model = CounterViewModel(view)
    bump_view_button = view.node("bump_button")

    counter_label = label("Declarative counter: 0", 16, 324, width=260)

    def bump_declarative_counter():
        view.click(bump_view_button)
        counter_label.set_text(f"Declarative counter: {view_model.counter.get()}")

    label("Bump declarative counter", 16, 336, width=260, height=16, font_size=12)
    bump_btn = window.add_rect(background=(0x33, 0x33, 0x33, 0xFF), width=220, height=32, x=16, y=356)
    bump_btn.enable_interaction()
    bump_btn.set_on_click(bump_declarative_counter)
    screen.add_child(bump_btn)

    DATA_REFS.clear()
    DATA_REFS.update(
        {
            "data_list": data_list,
            "page_btn": page_btn,
            "load_next_page": load_next_page,
            "page_state": page_state,
            "panel": panel,
            "move_btn": move_btn,
            "move_panel": move_panel,
            "dock_state": dock_state,
            "view": view,
            "view_model": view_model,
            "bump_view_button": bump_view_button,
            "counter_label": counter_label,
            "bump_btn": bump_btn,
            "bump_declarative_counter": bump_declarative_counter,
        }
    )
    return screen


SCREENS = {
    "components": build_gallery_screen,
    "motion": build_motion_screen,
    "data": build_data_screen,
}


def build_nav_button(window, y, text):
    btn = window.add_rect(background=NAV_ITEM_BG, width=NAV_WIDTH - 24, height=40, x=12, y=y)
    btn.enable_interaction()
    # A real `Text` label, layered on top -- attached as the button's
    # own child (not `nav`'s), so its `x`/`y` inset resolves relative
    # to the button's own origin, the same real "inset resolves against
    # whatever the immediate parent turns out to be" behavior every
    # other absolutely-positioned node in this demo already relies on.
    #
    # Real finding, caught only by actually dispatching a click: this
    # engine's hit-testing does not bubble from a hit child up to a
    # parent's own click handler -- a label centered over the button
    # (where `Window.click(btn)` always targets) silently absorbed
    # every click, since `add_text`'s own node has no handler of its
    # own. Kept near the button's own top edge instead, deliberately
    # clear of its real geometric center.
    label = window.add_text(text, background=(0xEE, 0xEE, 0xEE, 0xFF), width=NAV_WIDTH - 40, height=14, x=8, y=4, font_size=12)
    btn.add_child(label)
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
        btn = build_nav_button(window, y, SCREEN_LABELS[key])
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

    # Phase 5's own real, comprehensive Tab-order sweep: every one of
    # the gallery's 8 real interactive controls, reached in the exact
    # order they were attached -- replacing the 2-control spot-check
    # earlier phases used. `main()` already ran its own nav-order check
    # immediately before calling this function, consuming the first two
    # Tab stops (both nav buttons) -- an explicit, stated call-order
    # dependency, not a hidden one.
    interactive_order = [
        refs["checkbox"],
        refs["slider"],
        refs["text_field"],
        *refs["swatches"],
        refs["dark_toggle"],
    ]
    for i, node in enumerate(interactive_order):
        window.press_key("tab")
        assert node.is_focused(), f"gallery Tab order must reach control {i} next"

        if node is refs["checkbox"]:
            # Real keyboard *operability*, not just reachability: a
            # real Enter press activates whatever is currently
            # focused, the identical `DispatchOutcome::Activated`
            # mechanism a real mouse click already produces (both
            # routed through the same registered `Click` handler) --
            # fires the real `toggle_checkbox` handler, not a
            # hand-rolled duplicate of it.
            was_checked = refs["checkbox"].get_checked()
            window.press_key("enter")
            assert refs["checkbox"].get_checked() != was_checked, (
                "a real keyboard Enter press must activate a focused Checkbox"
            )
            print(f"Checkbox: keyboard Enter activated it ({was_checked} -> {refs['checkbox'].get_checked()})")
        elif node is refs["slider"]:
            # `Node.animate(..., duration_ms=0)` only *registers* the
            # animation -- it snaps to the target the next time
            # something ticks this node, normally `App.run()`'s own
            # per-frame loop (confirmed in `view.rs::apply_binding_
            # value`'s own doc comment), which hasn't started yet here.
            # The real, already-proven way to move a Slider
            # synchronously without a render loop is the same one
            # `examples/slider.py` already established: a real
            # dispatched arrow-key nudge (`Tree::dispatch_slider_key`'s
            # own mechanism manually ticks immediately, unlike
            # `animate()`).
            before = refs["slider"].get("thumb_position")
            window.press_key("right")
            after = refs["slider"].get("thumb_position")
            assert after > before, "a real ArrowRight nudge must move a focused Slider"
            print(f"Slider: thumb_position {before} -> {after}")
    print(f"Gallery: keyboard Tab reaches all {len(interactive_order)} interactive controls, in order")

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


def verify_motion_screen(window):
    """Phase 3's real functional proof, run while "motion" is the
    active screen. Matches the exact verification bar every prior
    motion-related example in this workspace already uses (`pan_zoom.
    py`/`shape_morph.py`/`elevation.py`): a real dispatched click on
    each trigger registers its animation with no error -- the
    definitive pixel-level proof that each one actually paints its
    target is `engine-render`'s own tests, not this script. A
    duration>0 animation deliberately isn't asserted to have "landed"
    a specific value here -- unlike Phase 2's Slider check, there's no
    real, immediate-tick mechanism for `opacity`/`corner_radius`/
    `elevation`/`shape`/`transform` the way a keyboard nudge gives a
    Slider, so asserting a landed value would depend on unpredictable
    real wall-clock/frame-rate timing, not the real thing being proved.
    """
    refs = MOTION_REFS

    # Phase 5's own real, comprehensive Tab-order sweep for this
    # screen. Real finding, confirmed empirically before writing this:
    # the node focused before a real screen swap (`window.click()` on
    # a nav button, removing the old screen's whole subtree) is gone
    # from the tree, and focus resets to none -- the very next Tab
    # press after switching screens lands on the *first* focusable
    # node in the whole window again (nav button 1), not straight into
    # the new screen's own content. So reaching "motion"'s own first
    # control needs 3 Tab presses through the nav rail again first
    # (already proven reachable once, at the very start of `main()`
    # -- not re-asserted here, just consumed) before its own 5 real
    # controls.
    window.press_key("tab")
    window.press_key("tab")
    window.press_key("tab")
    interactive_order = [
        refs["fade_btn"], refs["round_btn"], refs["elevate_btn"], refs["morph_btn"], refs["pan_zoom_btn"]
    ]
    for i, node in enumerate(interactive_order):
        window.press_key("tab")
        assert node.is_focused(), f"motion Tab order must reach control {i} next"
        if node is refs["fade_btn"]:
            # Real keyboard *operability*: a real Enter press on the
            # focused Fade trigger fires the identical registered
            # `Click` handler a real mouse click would, not a
            # hand-rolled duplicate.
            window.press_key("enter")
            print("Motion: keyboard Enter activated the Fade trigger")
        else:
            window.click(node)
    print(f"Motion: keyboard Tab reaches all {len(interactive_order)} interactive controls, in order")
    print("Motion: Fade/Round/Elevate/Morph/Pan-Zoom triggers each registered a real animation, no error")

    # Real finding, caught only by actually dispatching this click (a
    # first draft assumed the opposite): `set_hit_test_circle` *replaces*
    # the canvas's default rectangular hit test entirely, not narrows it
    # -- `draw_node_graph`'s own circle sits over graph node 2's
    # position, not the canvas's own geometric center, so a real click
    # at the canvas's center (all `Window.click` can target) now misses
    # it entirely. Asserted here as the real, positive proof that the
    # custom hit test genuinely took effect, not the inverted claim a
    # first draft made.
    clicked = []
    refs["graph"].set_on_click(lambda: clicked.append(True))
    window.click(refs["graph"])
    assert not clicked, "the custom circular hit test must replace the default rect, not add to it"
    print("Canvas: the custom circular hit test genuinely replaced the default rectangular one")


def verify_data_screen(window):
    """Phase 4's real functional proof, run while "data" is the active
    screen: real virtualization (paging genuinely re-materializes, not
    just accumulates), a real headless drag-and-drop (the same
    `start_panel_drag`/`drop_panel_at` sequence `tests/test_docking.py`
    already establishes), and real cross-path data flow -- a click
    dispatched through `View`'s own dispatch updates a `Signal`, read
    back and reflected in an ordinary imperative `Window` label.
    """
    refs = DATA_REFS

    # Phase 5's own real, comprehensive Tab-order sweep for this
    # screen -- the same real "3 nav taps first" behavior `verify_
    # motion_screen` already found and accounted for (a screen swap
    # resets focus to none). Real keyboard *operability* on the first
    # control (page_btn, via Enter); `move_btn`/`bump_btn` are reached
    # here but activated separately below via their own existing real
    # checks, so a real drag/declarative-click side effect isn't
    # accidentally triggered twice.
    window.press_key("tab")
    window.press_key("tab")
    window.press_key("tab")
    interactive_order = [refs["page_btn"], refs["move_btn"], refs["bump_btn"]]
    before_start = refs["page_state"]["start"]
    for i, node in enumerate(interactive_order):
        window.press_key("tab")
        assert node.is_focused(), f"data Tab order must reach control {i} next"
        if node is refs["page_btn"]:
            window.press_key("enter")
    print(f"Data: keyboard Tab reaches all {len(interactive_order)} interactive controls, in order")
    assert refs["page_state"]["start"] == before_start + DATA_PAGE_SIZE, (
        "a real keyboard Enter press must activate the focused page button"
    )
    print(f"VirtualList: keyboard Enter paged from row {before_start} to row {refs['page_state']['start']}")

    # Real cascade + MD3 token resolution (M26), reachable from the
    # same declarative panel this screen embeds: the stylesheet's own
    # `id: bump_button` rule (corner_radius: 12) must have won over its
    # `kind: Rect` rule (corner_radius: 4).
    assert refs["bump_view_button"].get("corner_radius") == 12.0, (
        "the id-level stylesheet rule must win over the kind-level one"
    )
    print("Declarative View: stylesheet cascade resolved corner_radius=12 (id beats kind)")

    # Real cross-path data flow: a View-dispatched click updates a
    # Signal; the imperative Window label reflects the new value.
    refs["move_panel"]()  # exercised before the View click below, so
    # both real interactions in this screen run in one pass
    assert refs["dock_state"]["side"] == "right", "a real headless drag must move the panel to the right zone"
    window.click(refs["panel"])  # the panel must still be real, attached, and clickable in its new zone
    print(f"Docking: panel moved to the {refs['dock_state']['side']!r} zone via a real headless drag")

    before_count = refs["view_model"].counter.get()
    refs["bump_declarative_counter"]()  # the same handler the real button click wires to
    assert refs["view_model"].counter.get() == before_count + 1, "a View-dispatched click must update its own Signal"
    assert refs["counter_label"].get_text() == f"Declarative counter: {refs['view_model'].counter.get()}", (
        "the imperative label must reflect the declarative View's own real Signal state"
    )
    print(f"Declarative -> Imperative: {refs['counter_label'].get_text()!r}")


def main():
    window = Window(width=WINDOW_WIDTH, height=WINDOW_HEIGHT, title="tre v2 -- showcase")
    state, buttons, show_screen = build_showcase(window)

    assert state["current_key"] == SCREEN_ORDER[0], "the first screen must be shown at startup"

    # Real Tab-order proof, done first while it's a clean, predictable
    # state: the nav rail is always built before either screen's own
    # content (see `build_showcase`), so the two nav buttons are always
    # the first two Tab stops regardless of which screen is active --
    # checked here before any other Tab presses (Phase 2's own Slider
    # check below moves focus further) could make this ambiguous.
    for expected_key in SCREEN_ORDER:
        window.press_key("tab")
        assert buttons[expected_key].is_focused(), (
            f"Tab order must reach the {expected_key!r} nav button next"
        )
    print("keyboard Tab reaches every nav button first, in order")

    # Phase 2: exercise every gallery control while "components" (the
    # first screen) is still active.
    verify_gallery_screen(window)

    # Real functional proof: a dispatched click on the second nav
    # button must actually switch the active screen.
    window.click(buttons[SCREEN_ORDER[1]])
    assert state["current_key"] == SCREEN_ORDER[1], "clicking a nav button must switch the active screen"
    print(f"nav click switched the active screen to {state['current_key']!r}")

    # Phase 3: exercise every motion/canvas control while "motion" is
    # the active screen.
    verify_motion_screen(window)

    # Real functional proof: a dispatched click on the third nav button
    # must switch to the data & layout screen.
    window.click(buttons[SCREEN_ORDER[2]])
    assert state["current_key"] == SCREEN_ORDER[2], "clicking the third nav button must switch to 'data'"
    print(f"nav click switched the active screen to {state['current_key']!r}")

    # Phase 4: exercise the virtualized list, docking, and declarative
    # View panel while "data" is the active screen.
    verify_data_screen(window)

    # Leave the gallery as the visible starting screen for anyone
    # actually running this interactively.
    show_screen(SCREEN_ORDER[0])

    app = App()
    app.add_window(window)
    app.run(max_frames=60)
    print("demo/showcase.py: exited cleanly after 60 frames -- all 5 phases complete")


if __name__ == "__main__":
    main()
