#!/usr/bin/env python3
"""M31 Phase 1's real Line-Number Gutter (§5, §8): a real per-line
gutter for `Window.add_code_editor`, composed entirely from existing
primitives -- no new engine capability at all.

`BUILD_TRACKER.md`'s own Phase 1 scoping note flagged a real open
question: aligning each gutter number to its own real painted editor
line needs either a new `engine-py` per-line-position read-back, or a
fixed line-height assumed in the app (stated as fragile). Investigated
directly before writing any code: `TextRenderer::draw` (plain `Text`)
and `TextRenderer::draw_field` (`TextField`) both build their real
`parley::Layout` through the exact same `shaped_layout` method
(confirmed by direct source read) -- so a gutter composed as an
ordinary sibling `Text` node, with the identical `font_family`/
`font_weight`/`font_size` as the editor and wide enough never to wrap,
lines up with the editor's own real per-line Y positions *by
construction*. Proven precisely at the Rust level
(`crates/engine-render/src/text.rs::
a_plain_texts_own_multiline_content_lines_up_with_a_matching_
multiline_textfields_own_lines`, comparing real `parley::Layout::
lines()` geometry directly) -- this script proves the real end-to-end
FFI behavior: the gutter's own content tracks the editor's real line
count live, on every real edit.
"""

from tre import MONOSPACE_FONT_FAMILY, App, Window

window = Window(width=460, height=280, title="tre v2 -- code editor gutter")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

FONT_WEIGHT = 400.0
FONT_SIZE = 14.0
GUTTER_WIDTH = 32.0

initial_content = "def add(a, b):\n    return a + b"

editor = window.add_code_editor(
    content=initial_content,
    background=(0xFF, 0xFB, 0xFE, 0xFF),
    width=380,
    height=200,
    font_weight=FONT_WEIGHT,
    font_size=FONT_SIZE,
    x=20 + GUTTER_WIDTH,
    y=20,
)

gutter = window.add_text(
    content="",
    background=(0x49, 0x45, 0x4F, 0xFF),  # on-surface-variant-ish grey
    width=GUTTER_WIDTH,
    height=200,
    # M32 Phase 1 (§5, §8, §10): `add_code_editor` now always shapes
    # with the real bundled monospace face -- the gutter's own sibling
    # `Text` node must match that exact `font_family`, not `Roboto`,
    # or the "lines up by construction" claim above (identical
    # `shaped_layout` inputs) no longer holds.
    font_family=MONOSPACE_FONT_FAMILY,
    font_weight=FONT_WEIGHT,
    font_size=FONT_SIZE,
    x=20,
    y=20,
)


def sync_gutter() -> None:
    line_count = editor.get_text().count("\n") + 1
    gutter.set_text("\n".join(str(n) for n in range(1, line_count + 1)))


editor.set_on_change(sync_gutter)
sync_gutter()  # seed the gutter for the editor's own initial content

print(f"initial gutter:\n{gutter.get_text()!r}")
assert gutter.get_text() == "1\n2", "the seeded 2-line buffer must start the gutter at '1\\n2'"

window.click(editor)
window.press_key("up")
window.press_key("end")
window.press_key("enter")
window.type_text("    # adds two numbers")

print(f"after a real Enter mid-buffer:\n{editor.get_text()}")
print(f"gutter now:\n{gutter.get_text()!r}")
assert editor.get_text() == "def add(a, b):\n    # adds two numbers\n    return a + b"
assert gutter.get_text() == "1\n2\n3", "a real inserted line must grow the gutter to match"

app = App()
app.add_window(window)
app.run(max_frames=30)
print(
    "code_editor_gutter.py: exited cleanly after 30 frames -- a real gutter "
    "tracked a real live-edited buffer's own line count, composed from "
    "existing primitives with zero new engine capability"
)
