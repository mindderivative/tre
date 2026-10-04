"""The migration gate's switch, `TRE_FORBID_REMOVED=1` (`tre/_removed.py`):
its tables kept in step with the migration table in
`docs/migrating-0.3.5.md`, and what it
forbids and leaves alone. The switch patches classes for the rest of a
process, so it's exercised in a fresh one.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

import tre
from tre import _core, _removed

SPEC = Path(__file__).resolve().parent.parent / "docs" / "migrating-0.3.5.md"
TABLE = SPEC.read_text().split("## Migration table", 1)[1]


def table_rows() -> list[tuple[list[str], str, str]]:
    """The migration table's rows: the backticked names in "Today" (with
    any argument list dropped), and the "Today" and "Target" text."""
    rows = []
    for today, target in re.findall(r"^\| (.+?) \| (.+?) \|$", TABLE, re.M):
        if today in ("Today", "---"):
            continue
        names = [re.sub(r"\(.*", "", n) for n in re.findall(r"`([^`]+)`", today)]
        rows.append((names, today, target))
    return rows


def test_every_migration_table_name_is_forbidden_kept_or_explained() -> None:
    handled = {name for members in _removed.REMOVED.values() for name in members}
    handled |= {f"Event.{name}" for name in _removed.REMOVED["Event"]}
    handled |= set(_removed.PROPERTIES) | set(_removed.UNENFORCED)
    for names, today, target in table_rows():
        if target.startswith("unchanged"):
            continue
        if not names:  # a row naming no Python name at all
            assert today in _removed.UNENFORCED, today
            continue
        # A removed class's members, listed after it, go with it.
        if names[0] in _removed.REMOVED["tre"]:
            names = [n for n in names if n in _removed.REMOVED["tre"]]
        for name in names:
            assert name in handled, f"{name!r} is in the migration table but not in _removed.py"


def test_every_forbidden_name_is_in_the_migration_table() -> None:
    table = {name for names, _, _ in table_rows() for name in names}
    for owner, members in _removed.REMOVED.items():
        for name in members:
            key = f"{owner}.{name}" if owner == "Event" else name
            assert key in table, f"{owner}.{name} is forbidden but not in the migration table"
    for name in [*_removed.PROPERTIES, *_removed.UNENFORCED]:
        assert name in table or name in TABLE, name


#: Forbidden names 0.3.5 has already removed, by milestone.
GONE = {
    # M98: the declarative and reactivity layers. M99: theming.
    "tre": {
        "View", "Component", "Signal", "Computed", "Effect", "ViewModel", "batch", "untrack",
        "Theme",
        "CanvasContext",
    },
    "Window": {
        "from_view", "show_view",
        # M99 Phase 1 Step 1: the MD3 factories, the legacy overlays and
        # menus, the shell, the container transform, and theming.
        *_removed._FACTORIES_MOVED,
        "build_menu", "open_menu", "close_menu", "open_dialog", "close_dialog",
        "open_snackbar", "close_snackbar", "open_side_sheet", "close_side_sheet",
        "open_navigation_drawer", "close_navigation_drawer", "build_shell",
        "begin_container_transform", "end_container_transform", "set_theme", "theme",
        # M99 Phase 1 Step 2: docking's bare bones.
        "set_active_tab", "set_dock_handle", "set_drop_zone_highlight", "drag_panel_over",
        "drop_panel_at",
        # M99 Phase 1 Step 3: engine-md3's icons.
        "add_icon",
        # M99 Phase 2 Step 3: the file conveniences (D6).
        "add_image",
        # M100 Phase 1 Step 1: the renames.
        "add_rect", "add_text", "add_text_field", "add_code_editor", "add_image_from_bytes",
        "add_video", "add_canvas", "add_scroll_view", "add_virtual_list", "add_terminal",
        "get_monospace_cell_size", "resize_terminal", "copy_terminal_selection",
        "set_virtual_list_window", "redraw_canvas",
        # M100 Phase 1 Step 2: the synthetic-input surface (D9).
        *_removed._SIMULATED,
        "copy_to_system_clipboard", "cut_to_system_clipboard", "paste_from_system_clipboard",
    },
    "Node": {
        "set_context_menu",
        # M99 Phase 2 Step 1: the MD3 kinds' state.
        *_removed._WIDGET_STATE,
        "set_checked", "set_selected", "get_checked", "get_selected",
        # M99 Phase 2 Step 2: the state layer and ripple (D8).
        "enable_interaction",
        # M100 Phase 1 Step 1: the renames.
        "set_layout", "set_text", "set_clip_children", "set_syntax_spans", "set_folded_ranges",
        "set_terminal_selection", "push_frame", "get_text", "is_focused", "set_on_click",
        "set_on_hover_enter", "set_on_hover_exit", "set_on_change", "set_on_focus_enter",
        "set_on_focus_exit",
    },
    "Event": {"kind", "node", "source", "position"},
}  # fmt: skip


PAGE = SPEC


def test_the_published_migration_page_names_every_forbidden_name() -> None:
    """The migration page users read covers everything the switch
    forbids."""
    page = PAGE.read_text()
    for owner, members in _removed.REMOVED.items():
        for name in members:
            assert f"`{name}" in page or f", {name}" in page or f"`{owner}.{name}`" in page, (
                f"{owner}.{name} is forbidden but not on docs/migrating-0.3.5.md"
            )
    for name in _removed.PROPERTIES:
        assert f"`{name}`" in page, f"property {name!r} is not on docs/migrating-0.3.5.md"


def test_every_forbidden_name_exists_today_or_is_already_gone() -> None:
    """A typo'd table entry would forbid nothing."""
    classes = {"tre": tre, "Window": _core.Window, "Node": _core.Node, "Event": _core.Event}
    for owner, members in _removed.REMOVED.items():
        for name in members:
            gone = name in GONE.get(owner, set())
            assert hasattr(classes[owner], name) != gone, f"{owner}.{name}"


def test_off_by_default() -> None:
    assert os.environ.get("TRE_FORBID_REMOVED") != "1", "run the suite without the switch"
    assert tre.Painter is not None
    assert callable(tre.Window(10, 10, "off").create)


SCRIPT = r"""
import json
import tre

w = tre.Window(200, 100, "switch")
results = {}

def probe(label, fn):
    try:
        fn()
        results[label] = "allowed"
    except AttributeError as e:
        results[label] = str(e)
    except ImportError:
        results[label] = "ImportError"

box = w.create("box", width=50, height=20)
w.root.add_child(box)
probe("tre.Signal", lambda: tre.Signal)
probe("from tre import View", lambda: exec("from tre import View"))
probe("Window.add_button", lambda: w.add_button)
probe("Window.theme", lambda: w.theme)
probe("Node.set_on_click", lambda: box.set_on_click)
probe("animate background", lambda: box.animate("background", (0, 0, 0, 255), 0))
probe("get elevation", lambda: box.get("elevation"))
probe("positional on_complete", lambda: box.animate("opacity", 0.5, 100, lambda: None))
seen = []
box.on("click", lambda e: seen.append(e))
w.simulate("click", node=box)
probe("Event.kind", lambda: seen[0].kind)

# What stays works.
box.animate("opacity", 0.5, 100, on_complete=lambda: None)
box.set(fill=(1, 2, 3, 255), corner_radius=4)
w.advance(100)
results["kept"] = [box.get("opacity"), list(box.get("fill")), seen[0].type, box.get("value")]
results["hasattr add_button"] = hasattr(tre.Window, "add_button")
results["__all__"] = tre.__all__
exec("from tre import *")
print(json.dumps(results))
"""


def test_the_switch_forbids_removed_names_and_keeps_the_rest() -> None:
    env = {**os.environ, "TRE_FORBID_REMOVED": "1"}
    out = subprocess.run(
        [sys.executable, "-c", SCRIPT], env=env, capture_output=True, text=True, check=True
    )
    results = json.loads(out.stdout)
    removed = "is removed in tre 0.3.5 -- use"
    assert results["tre.Signal"] == f"Signal {removed} Tesserae's reactivity (D5)"
    assert results["from tre import View"] == "ImportError"
    assert results["Window.add_button"] == f"Window.add_button {removed} the framework's own widget"
    assert results["Window.theme"].startswith(f"Window.theme {removed}")
    assert results["Node.set_on_click"] == f'Node.set_on_click {removed} node.on("click", ...)'
    assert results["animate background"] == f"Node.property 'background' {removed} \"fill\""
    assert results["get elevation"] == f"Node.property 'elevation' {removed} \"shadows\""
    assert "pass on_complete by keyword" in results["positional on_complete"]
    assert results["Event.kind"] == f"Event.kind {removed} event.type"
    assert results["kept"] == [0.5, [1, 2, 3, 255], "click", None]
    assert results["hasattr add_button"] is False
    assert results["__all__"] == [
        "App", "Painter", "CursorImage", "Event", "Gradient", "LoopHandle", "MONOSPACE_FONT_FAMILY", "Node", "Shader", "ShaderError", "Window",
        "png_bytes", "register_font", "set_system_fonts", "system_fonts", "write_png",
    ]  # fmt: skip
