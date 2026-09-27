"""The `clip_children` property: settable on any node kind, with or without
children, and never touching a text input's text. The clipping itself is
checked by `engine-render`'s `clip_children.rs`.
"""

from tre import Window
from helpers import CODE_EDITOR, add


def test_clip_children_is_settable_on_a_plain_box():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    rect.set(clip_children=True)
    rect.set(clip_children=False)


def test_clip_children_is_settable_on_a_box_with_children():
    window = Window(width=400, height=300)
    container = add(window, "box", fill=(0, 0, 0, 0), width=100, height=50)
    child = add(window, "box", fill=(255, 0, 0, 255), width=100, height=200)
    container.add_child(child)
    container.set(clip_children=True)


def test_clip_children_applies_to_every_kind_unlike_syntax_spans():
    """Unlike `syntax_spans`/`folded_ranges`, which only a text input
    takes, `clip_children` applies to every node kind."""
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(0, 255, 0, 255), width=50, height=50)
    rect.set(clip_children=True)  # must not raise for a plain box

    editor = add(window, "text_input", **CODE_EDITOR, text="def f():\n    pass", width=200, height=100, font_size=14)
    editor.set(clip_children=True)  # must not raise for a text input either
    assert editor.get("text") == "def f():\n    pass", "clipping must never touch real content"
