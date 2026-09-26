"""M97: the migration gate's switch. With `TRE_FORBID_REMOVED=1` in the
environment when `tre` is imported -- or after `install()` -- every public
name 0.3.5 removes or renames raises `RemovedError`, naming what replaces
it, so a codebase can prove it no longer uses any of them by running its
tests with the switch on.

`RemovedError` is an `AttributeError`, so `hasattr` reports a removed name
as gone. Old property names are caught by value, in `Node.animate`, `get`,
`get_target`, and `stop_animation`, as is `animate`'s old positional
`on_complete`. `from tre import View` fails with Python's own `ImportError`,
which drops the replacement; `REMOVED["tre"]` has it.

The tables below are the migration table in `docs/design/target-api.md`,
as data; `tests/test_removed.py` keeps the two in step. This file only
uses names 0.3.4 already has, so it can be copied into a project pinned to
0.3.4 and installed from there.
"""

from __future__ import annotations

import sys
import types
from collections.abc import Callable
from typing import Any

from tre import _core

FRAMEWORK = "the framework's own"

_FACTORIES_MOVED = [
    "add_button", "add_icon_button", "add_fab", "add_extended_fab",
    "add_segmented_button", "add_chip", "add_menu_item", "add_badge",
    "add_linear_progress", "add_circular_progress", "add_loading_indicator",
    "add_time_picker_dial", "add_card", "add_divider", "add_tooltip",
    "add_dialog", "add_snackbar", "add_side_sheet", "add_navigation_rail",
    "add_navigation_drawer", "add_top_app_bar", "add_toolbar",
    "add_split_button", "add_button_group", "add_tabs", "add_search_bar",
    "add_search_view", "add_list_item", "add_list", "add_accordion_header",
    "add_tree_node", "add_date_picker_day", "add_time_input_field",
    "add_period_selector", "add_popover", "add_link", "add_spin_box",
    "add_pagination", "add_status_bar", "add_checkbox", "add_radio_button",
    "add_switch", "add_slider", "add_carousel", "add_splitter",
    "add_node_graph", "add_graph_node",
]  # fmt: skip

_SIMULATED = [
    "click", "hover", "focus", "scroll", "right_click", "press_key",
    "type_text", "press_ctrl", "copy", "cut", "paste", "select_all",
]  # fmt: skip

_WIDGET_STATE = [
    "set_carousel_index", "get_carousel_index", "get_carousel_position",
    "set_carousel_scroll", "get_carousel_scroll", "set_time_picker_dial_time",
    "get_time_picker_dial_time", "set_time_picker_dial_mode",
    "get_time_picker_dial_mode",
]  # fmt: skip

_LAYERS = "window.show_layer(node, ...) and hide_layer(node)"
_SIMULATE = "window.simulate(event, node=..., ...)"
_REACTIVITY = "Tesserae's reactivity (D5)"
_THEMING = "the framework's theming (D7)"
_DOCKING = "the dock_target and dock_drop events"
_CLIPBOARD = "read_clipboard and write_clipboard"
_STATE = "the framework's own state"

#: Removed or renamed members, by owner: `{owner: {name: replacement}}`.
REMOVED: dict[str, dict[str, str]] = {
    "tre": {
        "Signal": _REACTIVITY,
        "Computed": _REACTIVITY,
        "Effect": _REACTIVITY,
        "ViewModel": _REACTIVITY,
        "batch": _REACTIVITY,
        "untrack": _REACTIVITY,
        "View": "Tesserae's declarative layer (M98)",
        "Component": "Tesserae's declarative layer (M98)",
        "Theme": _THEMING,
        "CanvasContext": "Painter",
    },
    "Window": {
        "add_rect": 'window.create("box")',
        "add_text": 'window.create("text")',
        "add_text_field": 'window.create("text_input")',
        "add_code_editor": 'window.create("text_input")',
        "add_image_from_bytes": 'window.create("image"), frames via set(rgba=...)',
        "add_video": 'window.create("image"), frames via set(rgba=...)',
        "add_image": 'window.create("image") with decoded pixels (D6)',
        "add_icon": "window.create(\"path\") with the framework's icon data",
        "add_canvas": 'window.create("canvas")',
        "add_scroll_view": 'window.create("scroll_view")',
        "add_virtual_list": 'window.create("virtual_list")',
        "add_terminal": 'window.create("terminal")',
        **{name: f"{FRAMEWORK} widget" for name in _FACTORIES_MOVED},
        "from_view": "window.root.add_child(screen)",
        "show_view": "window.root.add_child(screen)",
        "theme": _THEMING,
        "set_theme": _THEMING,
        "build_menu": _LAYERS,
        "open_menu": _LAYERS,
        "close_menu": _LAYERS,
        "open_dialog": _LAYERS,
        "close_dialog": _LAYERS,
        "open_snackbar": _LAYERS,
        "close_snackbar": _LAYERS,
        "open_side_sheet": _LAYERS,
        "close_side_sheet": _LAYERS,
        "open_navigation_drawer": _LAYERS,
        "close_navigation_drawer": _LAYERS,
        "build_shell": f"{FRAMEWORK} shell",
        "begin_container_transform": f"{FRAMEWORK} transition, with animate",
        "end_container_transform": f"{FRAMEWORK} transition, with animate",
        "get_monospace_cell_size": "window.measure_text(...)",
        "resize_terminal": "terminal.set(cols=..., rows=...)",
        "copy_terminal_selection": 'terminal.get("selection") and write_clipboard',
        **{name: _SIMULATE for name in _SIMULATED},
        "copy_to_system_clipboard": _CLIPBOARD,
        "cut_to_system_clipboard": _CLIPBOARD,
        "paste_from_system_clipboard": _CLIPBOARD,
        "set_active_tab": "set_active_panel",
        "set_dock_handle": _DOCKING,
        "set_drop_zone_highlight": _DOCKING,
        "drag_panel_over": f"{_DOCKING}, and {_SIMULATE}",
        "drop_panel_at": f"{_DOCKING}, and {_SIMULATE}",
        "set_virtual_list_window": "nothing: tre keeps a virtual list's visible rows built",
        "redraw_canvas": "canvas.redraw()",
    },
    "Node": {
        "set_layout": "node.set(...)",
        "set_text": "node.set(text=...)",
        "set_checked": _STATE,
        "set_selected": _STATE,
        "set_clip_children": "node.set(clip_children=...)",
        "set_syntax_spans": "node.set(syntax_spans=...)",
        "set_folded_ranges": "node.set(folded_ranges=...)",
        "set_terminal_selection": "node.set(selection=...)",
        "get_text": 'node.get("text")',
        "get_checked": _STATE,
        "get_selected": _STATE,
        "is_focused": 'node.get("focused")',
        "set_on_click": 'node.on("click", ...)',
        "set_on_hover_enter": 'node.on("pointer_enter", ...)',
        "set_on_hover_exit": 'node.on("pointer_leave", ...)',
        "set_on_change": 'node.on("change", ...)',
        "set_on_focus_enter": 'node.on("focus", ...)',
        "set_on_focus_exit": 'node.on("unfocus", ...)',
        "set_context_menu": f'node.on("secondary_click", ...) and {_LAYERS}',
        "enable_interaction": "the framework's own hover and press feedback (D8)",
        "push_frame": "node.set(rgba=..., pixel_width=..., pixel_height=...)",
        **{name: f"{FRAMEWORK} widget state" for name in _WIDGET_STATE},
    },
    "Event": {
        "kind": "event.type",
        "node": "event.target",
    },
}

#: Old property names, caught by value in `animate`, `get`, `get_target`,
#: and `stop_animation`: `{name: replacement}`.
PROPERTIES: dict[str, str] = {
    "background": '"fill"',
    "foreground": '"fill"',
    "border_color": '"stroke_color"',
    "border_width": '"stroke_width"',
    "corner_radii_override": 'a 4-tuple "corner_radius"',
    "elevation": '"shadows"',
    "transform": '"translate_x", "translate_y", and "scale"',
    "rotation": '"rotation_deg"',
    "shape": "a path's \"data\"",
    "check_progress": "the framework's own animation state",
    "select_progress": "the framework's own animation state",
    "toggle_progress": "the framework's own animation state",
}

#: Migration-table entries no name check can catch, and why.
UNENFORCED: dict[str, str] = {
    "value": 'still the accessibility value, so get("value") stays legal; '
    "the legacy widgets whose numeric value it read are unreachable once "
    "their factories are gone",
    "Container": "a declarative kind, gone with View",
    "start_panel_drag": "keeps its name; only its argument changes (a panel, not a handle)",
    "remove": "keeps its name; since 0.3.4 it detaches and keeps the node alive",
    "animate": "keeps its name; the positional on_complete it used to take is caught",
    "get": "keeps its name; old property names are caught",
    "MD3 named motion curves": "had no Python name: animate's easing only takes "
    '"linear" or a cubic bezier',
}


class RemovedError(AttributeError):
    """A name 0.3.5 removes or renames, used while the switch is on."""

    def __init__(self, owner: str, name: str, replacement: str) -> None:
        subject = name if owner == "tre" else f"{owner}.{name}"
        super().__init__(f"{subject} is removed in tre 0.3.5 -- use {replacement}")


class _Forbidden:
    """Stands in for a removed class member: any access raises."""

    def __init__(self, owner: str, name: str, replacement: str) -> None:
        self.error = (owner, name, replacement)

    def __get__(self, instance: object, owner: type | None = None) -> Any:
        raise RemovedError(*self.error)


class _GuardedModule(types.ModuleType):
    """`tre`'s module type while the switch is on: a removed module-level
    name raises on access -- `tre.View` and `from tre import View` alike --
    while `tre`'s own code, which reads its globals directly, is unaffected."""

    def __getattribute__(self, name: str) -> Any:
        replacement = REMOVED["tre"].get(name)
        if replacement is not None:
            raise RemovedError("tre", name, replacement)
        return super().__getattribute__(name)


def _checked(method: Callable[..., Any], *, positional_on_complete: bool = False) -> Any:
    """`method`, refusing an old property name -- and, for `animate`, an
    `on_complete` passed where `easing` now goes."""

    def wrapper(self: Any, name: str, *args: Any, **kwargs: Any) -> Any:
        if name in PROPERTIES:
            raise RemovedError("Node", f"property {name!r}", PROPERTIES[name])
        if positional_on_complete and len(args) >= 3 and callable(args[2]):
            raise RemovedError(
                "Node",
                "animate(name, to, duration_ms, on_complete)",
                "animate(name, to, duration_ms, easing, on_complete) -- "
                "pass on_complete by keyword",
            )
        return method(self, name, *args, **kwargs)

    wrapper.__name__ = method.__name__
    wrapper.__doc__ = method.__doc__
    return wrapper


_installed = False


def install() -> None:
    """Forbids every name 0.3.5 removes or renames, in this process, from
    now on. Idempotent."""
    global _installed
    if _installed:
        return
    _installed = True
    classes: dict[str, type] = {"Window": _core.Window, "Node": _core.Node, "Event": _core.Event}
    for owner, cls in classes.items():
        for name, replacement in REMOVED[owner].items():
            setattr(cls, name, _Forbidden(owner, name, replacement))
    node: Any = _core.Node
    node.animate = _checked(node.animate, positional_on_complete=True)
    for name in ("get", "get_target", "stop_animation"):
        setattr(node, name, _checked(getattr(node, name)))
    module = sys.modules["tre"]
    exported = module.__dict__["__all__"]
    setattr(module, "__all__", [n for n in exported if n not in REMOVED["tre"]])
    module.__class__ = _GuardedModule
