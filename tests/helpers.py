"""Shared test helpers (M100): building nodes through the target API.

`add` is what the removed `window.add_*` factories did -- create a node and
attach it to the window's root -- so tests that just need a node in the tree
stay one line.
"""

from typing import Any

from tre import MONOSPACE_FONT_FAMILY, Node, Window

#: A text input set up the way the removed `add_code_editor` was.
CODE_EDITOR: dict[str, Any] = {
    "font_family": MONOSPACE_FONT_FAMILY,
    "multiline": True,
    "show_whitespace": True,
}


def add(window: Window, kind: str, **props: Any) -> Node:
    """Creates a `kind` node with `props` and attaches it to the root."""
    node = window.create(kind, **props)
    window.root.add_child(node)
    return node
