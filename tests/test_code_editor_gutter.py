"""M31 Phase 1 (§5, §8): real, repeatable coverage of the Line-Number
Gutter pattern -- composed entirely from existing primitives (`Window.
add_code_editor`/`add_text`, `Node.set_on_change`), no new engine-py
API added by this phase at all. The real per-line pixel-alignment
claim is proven at the Rust level
(`crates/engine-render/src/text.rs::
a_plain_texts_own_multiline_content_lines_up_with_a_matching_
multiline_textfields_own_lines`); this suite proves the real FFI-level
behavior -- a gutter built this way tracks a real, live-edited
buffer's own line count correctly.
"""

from tre import Node, Window


def _editor_and_gutter(window: Window, content: str = "") -> tuple[Node, Node]:
    editor = window.add_code_editor(
        content=content,
        background=(0xFF, 0xFB, 0xFE, 0xFF),
        width=300,
        height=200,
    )
    gutter = window.add_text(
        content="",
        background=(0, 0, 0, 0xFF),
        width=32,
        height=200,
        font_family="Roboto",
        font_weight=400.0,
        font_size=14.0,
    )

    def sync_gutter() -> None:
        line_count = editor.get_text().count("\n") + 1
        gutter.set_text("\n".join(str(n) for n in range(1, line_count + 1)))

    editor.set_on_change(sync_gutter)
    sync_gutter()
    return editor, gutter


def test_a_single_line_buffer_starts_the_gutter_at_one():
    window = Window(width=400, height=300)
    _editor, gutter = _editor_and_gutter(window, content="hello")
    assert gutter.get_text() == "1"


def test_a_multi_line_buffer_seeds_the_gutter_with_every_real_line():
    window = Window(width=400, height=300)
    _editor, gutter = _editor_and_gutter(window, content="a\nb\nc\nd")
    assert gutter.get_text() == "1\n2\n3\n4"


def test_a_real_enter_keypress_grows_the_gutter_live():
    window = Window(width=400, height=300)
    editor, gutter = _editor_and_gutter(window, content="one\ntwo")
    assert gutter.get_text() == "1\n2"

    window.click(editor)
    window.press_key("end")
    window.press_key("enter")
    window.type_text("three")

    assert editor.get_text() == "one\ntwo\nthree"
    assert gutter.get_text() == "1\n2\n3", "a real inserted line must grow the gutter live"


def test_a_real_backspace_that_merges_two_lines_shrinks_the_gutter():
    window = Window(width=400, height=300)
    editor, gutter = _editor_and_gutter(window, content="one\ntwo\nthree")
    assert gutter.get_text() == "1\n2\n3"

    window.click(editor)
    window.press_key("home")  # start of "three"
    window.press_key("backspace")  # merges "two" and "three" onto one real line

    assert editor.get_text() == "one\ntwothree"
    assert gutter.get_text() == "1\n2", "merging two real lines must shrink the gutter live"
