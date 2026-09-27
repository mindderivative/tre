"""Shared test helpers.

`add` creates a node with `window.create` and attaches it to the window's
root, so a test that just needs a node in the tree stays one line.
`CODE_EDITOR` is the props for a monospace, multiline text input that shows
whitespace.
"""

from typing import Any

from tre import MONOSPACE_FONT_FAMILY, Node, Window

#: Props for a code-editor-style text input.
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
