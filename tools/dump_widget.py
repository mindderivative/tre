"""Dump a node subtree -- every node's kind, properties and computed layout
box -- as diffable JSON, and generate the reference for the widget
factories 0.3.5 removes.

    python tools/dump_widget.py add_button        # one factory, JSON
    python tools/dump_widget.py add_chip --full   # defaults included
    python tools/dump_widget.py --reference       # regenerate the docs

As a library, ``dump(node)`` returns the same dict for any node, so a
rebuilt widget can be compared with the factory it replaces:

    assert dump(rebuilt) == dump(legacy)

A property is left out when it holds a fresh ``box``'s value, unless it
belongs to the node's kind (``text``, ``fit``, ...) or ``full=True``.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
import tempfile
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import tre

ROOT = Path(__file__).resolve().parent.parent
REFERENCE_MD = ROOT / "docs" / "design" / "legacy-widgets.md"
REFERENCE_JSON = ROOT / "docs" / "design" / "legacy-widgets.json"

# Mirrors engine-py's property tables: node_layout::LAYOUT_PROPS,
# node_props::SETTABLE, node_kind_props::KIND_PROPS.
LAYOUT = [
    "width", "height", "min_width", "min_height", "max_width", "max_height",
    "aspect_ratio", "position", "x", "y", "flex_direction", "flex_wrap",
    "align_items", "justify_content", "gap", "padding", "padding_top",
    "padding_right", "padding_bottom", "padding_left", "flex_grow",
    "flex_shrink", "flex_basis", "align_self", "margin", "margin_top",
    "margin_right", "margin_bottom", "margin_left",
]  # fmt: skip
COMMON = [
    "visible", "z_index", "clip_children", "translate_x", "translate_y",
    "scale", "rotation_deg", "fill", "stroke_color", "stroke_width",
    "opacity", "corner_radius", "shadows", "role", "label", "value_min",
    "value_max", "value_step", "checked", "selected", "expanded", "disabled",
    "level", "live", "a11y_hidden", "focusable", "tab_index", "cursor",
    "hit_testable", "draw", "materialize", "size_hint",
]  # fmt: skip
KIND = [
    "data", "view_box", "trim_start", "trim_end", "placeholder",
    "placeholder_fill", "caret_color", "selection_fill", "obscured",
    "scrollbar_fill", "scrollbar_width", "palette", "text", "font_family",
    "font_weight", "font_size", "line_height", "text_align", "font_style",
    "letter_spacing", "wrap", "max_lines", "overflow", "multiline",
    "selection", "show_whitespace", "syntax_spans", "folded_ranges", "rgba",
    "pixel_width", "pixel_height", "fit", "orientation", "scroll_offset",
    "cols", "rows", "item_count", "item_extent",
]  # fmt: skip
# The pre-0.3.4 numeric reads `get` falls back to -- the removed widgets'
# animated state lives here.
LEGACY_NUMBERS = [
    "border_width", "check_progress", "elevation", "select_progress",
    "toggle_progress", "value",
]  # fmt: skip
BOX = ("layout_x", "layout_y", "layout_width", "layout_height")


def _plain(value: Any) -> Any:
    """A JSON-stable form: floats rounded, callables and pixels summarized."""
    if isinstance(value, float):
        return round(value, 4) + 0.0
    if isinstance(value, (bytes, bytearray)):
        digest = hashlib.sha256(value).hexdigest()[:12]
        return {"bytes": len(value), "sha256": digest}
    if isinstance(value, (list, tuple)):
        return [_plain(v) for v in value]
    if isinstance(value, dict):
        return {k: _plain(v) for k, v in value.items()}
    if callable(value):
        return "<callable>"
    return value


def _read(node: tre.Node, name: str) -> tuple[bool, Any]:
    try:
        return True, _plain(node.get(name))
    except (ValueError, TypeError):
        return False, None


_BOX_DEFAULTS: dict[str, Any] | None = None


def _box_defaults() -> dict[str, Any]:
    global _BOX_DEFAULTS
    if _BOX_DEFAULTS is None:
        box = tre.Window(10, 10, "defaults").create("box")
        names = LAYOUT + COMMON + LEGACY_NUMBERS
        _BOX_DEFAULTS = {n: _read(box, n)[1] for n in names}
    return _BOX_DEFAULTS


def dump(node: tre.Node, full: bool = False) -> dict[str, Any]:
    """`node` and its content subtree as a plain dict."""
    defaults = _box_defaults()
    props: dict[str, Any] = {}
    for name in LAYOUT + COMMON:
        ok, value = _read(node, name)
        if ok and (full or value != defaults[name]):
            props[name] = value
    for name in KIND:
        ok, value = _read(node, name)
        if ok:
            props[name] = value
    legacy: dict[str, Any] = {}
    for name in LEGACY_NUMBERS:
        ok, value = _read(node, name)
        if ok and name not in props and (full or value != defaults[name]):
            legacy[name] = value
    out: dict[str, Any] = {
        "kind": node.get("kind"),
        "box": [_plain(node.get(n)) for n in BOX],
        "props": props,
    }
    if legacy:
        out["legacy_numbers"] = legacy
    children = node.children()
    if children:
        out["children"] = [dump(child, full) for child in children]
    return out


def _top(node: tre.Node) -> tre.Node:
    while (parent := node.parent()) is not None:
        node = parent
    return node


def _flatten(value: Any) -> Iterator[tre.Node]:
    if isinstance(value, tre.Node):
        yield value
    elif isinstance(value, (list, tuple)):
        for item in value:
            yield from _flatten(item)


def _path(root: tre.Node, target: tre.Node) -> str | None:
    """`target`'s child-index path under `root` (`""` is root itself)."""
    if root == target:
        return ""
    for i, child in enumerate(root.children()):
        sub = _path(child, target)
        if sub is not None:
            return f"{i}/{sub}" if sub else str(i)
    return None


def dump_factory(
    window: tre.Window, returned: Any, full: bool = False
) -> dict[str, Any]:
    """Everything a factory call added: the window's content, plus any
    returned node living outside it (a layer), and where each returned
    node sits (`"content:0/1"`, `"layer0:"`)."""
    roots: list[tre.Node] = []
    trees = {"content": [dump(c, full) for c in window.root.children()]}
    returns: list[str] = []
    for node in _flatten(returned):
        top = _top(node)
        if top == window.root:
            returns.append(f"content:{_path(window.root, node)}")
            continue
        if top not in roots:
            roots.append(top)
            trees[f"layer{len(roots) - 1}"] = [dump(top, full)]
        returns.append(f"layer{roots.index(top)}:{_path(top, node)}")
    return {"returns": returns, "trees": trees}


# --- The removed factories, called at their defaults -----------------------

WHITE = (255, 255, 255, 255)
BLACK = (0, 0, 0, 255)
# examples/image.py's 4x4 PNG, so `add_image` has a file to decode.
_PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAT0lEQVR4nAFEALv/"
    "APRDNv//mAD//+s7/0yvUP8BALzU/yHaHwAeu8IAXdb7AADpHmP/eVVI/2B9i/8A"
    "AACAAf////8AwggAjAJDANx3bQC9UiChZNHhkwAAAABJRU5ErkJggg=="
)


def _png_path() -> str:
    path = Path(tempfile.gettempdir()) / "tre_dump_widget_4x4.png"
    if not path.exists():
        path.write_bytes(_PNG)
    return str(path)


def _args(w: tre.Window) -> dict[str, dict[str, Any]]:
    """The minimal call for each factory: its required arguments only."""
    return {
        "add_rect": {"background": WHITE, "width": 100, "height": 40},
        "add_text": {"content": "Label", "foreground": BLACK, "width": 100, "height": 20},
        "add_button": {"label": "Button", "width": 120, "height": 40},
        "add_icon_button": {"icon": "settings"},
        "add_fab": {"icon": "add"},
        "add_extended_fab": {"label": "Compose", "width": 140},
        "add_segmented_button": {"labels": ["Day", "Week"], "width": 200},
        "add_chip": {"label": "Chip", "width": 100},
        "add_menu_item": {"label": "Item"},
        "add_badge": {},
        "add_linear_progress": {"width": 200},
        "add_circular_progress": {},
        "add_loading_indicator": {},
        "add_time_picker_dial": {},
        "add_card": {"width": 200, "height": 120},
        "add_divider": {"length": 200},
        "add_tooltip": {"text": "Tip", "width": 80},
        "add_dialog": {"headline": "Title", "supporting_text": "Body", "width": 300, "height": 200},
        "add_snackbar": {"text": "Saved", "width": 300},
        "add_side_sheet": {},
        "add_navigation_rail": {"labels": ["Home", "Search"], "icons": ["home", "search"]},
        "add_navigation_drawer": {"labels": ["Home", "Search"], "icons": ["home", "search"]},
        "add_top_app_bar": {"title": "Title"},
        "add_toolbar": {},
        "add_split_button": {"label": "Send", "width": 120, "height": 40},
        "add_button_group": {"labels": ["A", "B"], "width": 200, "height": 40},
        "add_tabs": {"labels": ["One", "Two"]},
        "add_search_bar": {"placeholder": "Search", "width": 300},
        "add_search_view": {"width": 300, "height": 400},
        "add_list_item": {"headline": "Headline"},
        "add_list": {"items": [w.create("box", width=100, height=20)]},
        "add_accordion_header": {"title": "Section"},
        "add_tree_node": {"title": "Node"},
        "add_date_picker_day": {"day": 1},
        "add_time_input_field": {"value": "12"},
        "add_period_selector": {},
        "add_popover": {"subhead": "Subhead", "supporting_text": "Body", "width": 300, "height": 150},
        "add_link": {"content": "Link", "width": 80},
        "add_spin_box": {"value": "1"},
        "add_pagination": {"page_count": 3},
        "add_status_bar": {"text": "Ready"},
        "add_checkbox": {"background": WHITE, "width": 18, "height": 18},
        "add_radio_button": {},
        "add_switch": {},
        "add_slider": {"background": WHITE, "width": 200, "height": 20},
        "add_image": {"path": _png_path(), "width": 40, "height": 40},
        "add_image_from_bytes": {"rgba": bytes(16), "pixel_width": 2, "pixel_height": 2, "width": 40, "height": 40},
        "add_video": {"width": 160, "height": 90},
        "add_node_graph": {"width": 400, "height": 300},
        "add_graph_node": {"label": "Node", "x": 10, "y": 10, "width": 120, "height": 60},
        "add_icon": {"name": "home", "foreground": BLACK, "size": 24},
        "add_text_field": {"background": WHITE, "width": 200, "height": 56},
        "add_code_editor": {"content": "x = 1", "background": WHITE, "width": 300, "height": 200},
        "add_terminal": {"shell": "/bin/sh", "cols": 20, "rows": 4, "background": BLACK},
        "add_carousel": {"layout": "uncontained", "width": 300, "height": 200, "background": WHITE},
        "add_scroll_view": {"width": 200, "height": 200},
        "add_splitter": {"background": WHITE, "width": 300, "height": 200},
        "add_virtual_list": {"item_count": 3, "item_extent": 20, "materialize": lambda i: WHITE},
        "add_canvas": {"width": 100, "height": 100, "draw": lambda ctx: None},
    }  # fmt: skip


FACTORIES = list(_args(tre.Window(10, 10, "names")))


def build(factory: str) -> tuple[tre.Window, dict[str, Any], Any]:
    """A fresh 800x600 window holding one `factory` call at its defaults."""
    w = tre.Window(800, 600, factory)
    w.advance(0)  # pin the clock: no animation has moved
    args = _args(w)[factory]
    if factory == "add_graph_node":
        args["graph"] = w.add_node_graph(width=400, height=300)
    returned = getattr(w, factory)(**args)
    return w, args, returned


def dump_one(factory: str, full: bool = False) -> dict[str, Any]:
    w, args, returned = build(factory)
    shown = {k: _plain(v) for k, v in args.items() if not isinstance(v, tre.Node)}
    if factory == "add_list":
        shown["items"] = ["<box 100x20>"]
    if factory == "add_graph_node":
        shown["graph"] = "<add_node_graph(width=400, height=300)>"
    return {"factory": factory, "args": shown, **dump_factory(w, returned, full)}


# --- The generated reference -----------------------------------------------


_KIND_DEFAULTS: dict[str, dict[str, Any]] = {}


def _kind_defaults(kind: str) -> dict[str, Any]:
    """A fresh `kind` node's kind properties -- `{}` for a kind `create`
    can't make bare."""
    if kind not in _KIND_DEFAULTS:
        props = {"text": ""} if kind == "text" else {}
        try:
            node = tre.Window(10, 10, "defaults").create(kind, **props)
        except (ValueError, TypeError):
            _KIND_DEFAULTS[kind] = {}
        else:
            reads = {n: _read(node, n) for n in KIND if n != "text"}
            _KIND_DEFAULTS[kind] = {n: v for n, (ok, v) in reads.items() if ok}
    return _KIND_DEFAULTS[kind]


def _outline(tree: dict[str, Any], depth: int = 0) -> Iterator[str]:
    """One line per node, leaving out what the JSON form spells out:
    `x`/`y` (the box shows where it landed), per-side padding and margin
    already in the four-sided form, and kind properties at their
    defaults."""
    x, y, width, height = tree["box"]
    defaults = _kind_defaults(tree["kind"])
    props = tree["props"]
    shown = {
        k: v
        for k, v in props.items()
        if k not in ("x", "y")
        and not (k.startswith(("padding_", "margin_")) and k.split("_")[0] in props)
        and not (k in defaults and defaults[k] == v)
    }
    shown.update(tree.get("legacy_numbers", {}))
    detail = " ".join(f"{k}={json.dumps(v)}" for k, v in shown.items())
    head = f"{'  ' * depth}{tree['kind']} {width:g}x{height:g} @{x:g},{y:g}"
    yield f"{head}  {detail}".rstrip()
    for child in tree.get("children", []):
        yield from _outline(child, depth + 1)


def reference() -> tuple[str, list[dict[str, Any]]]:
    dumps = [dump_one(name) for name in FACTORIES]
    lines = [
        "# Legacy widget reference",
        "",
        "<!-- Generated by tools/dump_widget.py --reference; don't edit. -->",
        "",
        "Every widget factory 0.3.5 removes, called with only its required",
        "arguments in a fresh 800x600 window, the clock pinned at 0. Each",
        "node: its `kind`, computed box (`width x height @ x,y`), and every",
        "property that differs from a fresh node of its kind.",
        "`legacy_numbers` are the removed kinds' numeric state. `Returns` says where each",
        "node the factory returned sits: a child-index path under the window",
        "content, or under a node outside it (`layer0`).",
        "",
        "The machine-readable form is",
        "[`legacy-widgets.json`](legacy-widgets.json); `dump(node)` in",
        "`tools/dump_widget.py` produces the same shape for any node, so a",
        "rebuilt widget can be diffed against these.",
        "",
    ]
    for entry in dumps:
        call = ", ".join(f"{k}={json.dumps(v)}" for k, v in entry["args"].items())
        returns = ", ".join(entry["returns"])
        lines += [f"## `{entry['factory']}`", "", f"`{entry['factory']}({call})`", ""]
        lines += [f"Returns: `{returns}`", ""]
        for name, trees in entry["trees"].items():
            if not trees:
                continue
            lines += [f"{name}:", "", "```text"]
            for tree in trees:
                lines += list(_outline(tree))
            lines += ["```", ""]
    return "\n".join(lines), dumps


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument("factory", nargs="?", choices=FACTORIES)
    parser.add_argument("--full", action="store_true", help="include default values")
    parser.add_argument("--reference", action="store_true", help="regenerate the docs")
    args = parser.parse_args(argv)
    if args.reference:
        text, dumps = reference()
        REFERENCE_MD.write_text(text)
        REFERENCE_JSON.write_text(json.dumps(dumps, indent=1) + "\n")
        print(f"wrote {REFERENCE_MD.relative_to(ROOT)} and {REFERENCE_JSON.name}")
        return 0
    if args.factory is None:
        parser.error("name a factory, or pass --reference")
    json.dump(dump_one(args.factory, args.full), sys.stdout, indent=1)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
