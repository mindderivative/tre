"""A line-number gutter composed from a multiline text input, a text node, and a
`change` listener: it starts with the buffer's line count and grows and
shrinks as edits add and merge lines.
"""

from tre import Node, Window
from helpers import CODE_EDITOR, add


def _editor_and_gutter(window: Window, content: str = "") -> tuple[Node, Node]:
    editor = add(window, "text_input", **CODE_EDITOR, text=content, width=300, height=200, font_size=14)
    gutter = add(window, "text", text="", fill=(0, 0, 0, 0xFF), width=32, height=200, font_family="Roboto", font_weight=400.0, font_size=14.0)

    def sync_gutter() -> None:
        line_count = editor.get("text").count("\n") + 1
        gutter.set(text="\n".join(str(n) for n in range(1, line_count + 1)))

    editor.on("change", sync_gutter)
    sync_gutter()
    return editor, gutter


def test_a_single_line_buffer_starts_the_gutter_at_one():
    window = Window(width=400, height=300)
    _editor, gutter = _editor_and_gutter(window, content="hello")
    assert gutter.get("text") == "1"


def test_a_multi_line_buffer_seeds_the_gutter_with_every_line():
    window = Window(width=400, height=300)
    _editor, gutter = _editor_and_gutter(window, content="a\nb\nc\nd")
    assert gutter.get("text") == "1\n2\n3\n4"


def test_an_enter_keypress_grows_the_gutter_live():
    window = Window(width=400, height=300)
    editor, gutter = _editor_and_gutter(window, content="one\ntwo")
    assert gutter.get("text") == "1\n2"

    window.simulate("click", node=editor)
    window.simulate("key_down", key="end")
    window.simulate("key_down", key="enter")
    window.simulate("input", text="three")

    assert editor.get("text") == "one\ntwo\nthree"
    assert gutter.get("text") == "1\n2\n3", "an inserted line must grow the gutter live"


def test_a_backspace_that_merges_two_lines_shrinks_the_gutter():
    window = Window(width=400, height=300)
    editor, gutter = _editor_and_gutter(window, content="one\ntwo\nthree")
    assert gutter.get("text") == "1\n2\n3"

    window.simulate("click", node=editor)
    window.simulate("key_down", key="home")  # start of "three"
    window.simulate("key_down", key="backspace")  # merges "two" and "three" onto one line

    assert editor.get("text") == "one\ntwothree"
    assert gutter.get("text") == "1\n2", "merging two lines must shrink the gutter live"
