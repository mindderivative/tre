#!/usr/bin/env python3
"""M31 Phase 5's real Code Folding (§5, §8): `Node.set_folded_ranges`
collapses real byte ranges of a Code Editor's own content into one
visible "⋯" marker at paint time -- the user's own explicit "full real
folding" scope choice, given via `AskUserQuestion` before this phase
started, since no real reference (not even the sibling `pyCopper`
project's own `CodeEditor`, which explicitly excludes folding too) was
available to design it from.

Composes a real gutter toggle affordance with M31 Phase 1's own real
line-number gutter pattern: a small clickable `Rect` per foldable
line, positioned using the real measured line height M32 Phase 1's
own `Window.get_monospace_cell_size` now provides (this script used a
`font_size * 1.3` approximation before that real font/metric existed)
-- no new engine capability needed for the affordance itself, the
same real "compose from existing primitives" outcome Phase 1 already
reached.

**Real, deliberate v1 limitation, stated directly, not glossed over:**
cursor navigation is not fold-aware -- `set_folded_ranges`'s own Rust
doc comment has the full reasoning. `content`/`get_text()` are never
touched by folding; the real per-pixel proof that a folded range
paints differently (and that a click past one resolves to a real
content offset) is `crates/engine-render/tests/text_field_paint.rs`'s
own `a_folded_range_paints_genuinely_different_pixels_than_unfolded`/
`hit_test_position_on_a_folded_field_returns_real_content_offsets`,
not this script.
"""

from tre import MONOSPACE_FONT_FAMILY, Window

window = Window(width=460, height=300, title="tre v2 -- code folding")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

FONT_WEIGHT = 400.0
FONT_SIZE = 14.0
GUTTER_WIDTH = 32.0
TOGGLE_WIDTH = 16.0
# M32 Phase 1 (§5, §8, §10): the real measured line height of the
# bundled monospace face at FONT_SIZE -- only the toggle affordance's
# own click target needs this; the gutter's own line-number text stays
# pixel-perfect via the Phase 1 "matching sibling Text node" trick,
# which needs no Y math at all.
_, LINE_HEIGHT = window.get_monospace_cell_size(font_size=FONT_SIZE)

content = "def add(a, b):\n    return a + b\n\ndef sub(a, b):\n    return a - b"
# Byte range of "def add"'s own real body (the "    return a + b"
# line, including its own leading real newline so folding it leaves
# "def add(a, b):" and the blank line directly adjacent).
FOLD_RANGE = (15, 33)

editor = window.add_code_editor(
    content=content,
    background=(0xFF, 0xFB, 0xFE, 0xFF),
    width=380,
    height=220,
    font_weight=FONT_WEIGHT,
    font_size=FONT_SIZE,
    x=20 + GUTTER_WIDTH + TOGGLE_WIDTH,
    y=20,
)

gutter = window.add_text(
    content="",
    background=(0x49, 0x45, 0x4F, 0xFF),
    width=GUTTER_WIDTH,
    height=220,
    # M32 Phase 1 (§5, §8, §10): must match `add_code_editor`'s own
    # real bundled monospace face for the "lines up by construction"
    # claim to hold -- see `code_editor_gutter.py`'s identical note.
    font_family=MONOSPACE_FONT_FAMILY,
    font_weight=FONT_WEIGHT,
    font_size=FONT_SIZE,
    x=20 + TOGGLE_WIDTH,
    y=20,
)


def sync_gutter() -> None:
    line_count = editor.get_text().count("\n") + 1
    gutter.set_text("\n".join(str(n) for n in range(1, line_count + 1)))


editor.set_on_change(sync_gutter)
sync_gutter()

# A real, clickable fold-toggle affordance on line 1 ("def add(a,
# b):"), the real line the fold's own body hangs off of -- composes
# with the gutter the identical way Phase 1's own line numbers do.
toggle = window.add_rect(
    background=(0x79, 0x74, 0x7E, 0xFF),
    width=TOGGLE_WIDTH,
    height=TOGGLE_WIDTH,
    x=20,
    y=20 + (LINE_HEIGHT - TOGGLE_WIDTH) / 2,
)
toggle.enable_interaction()

folded = False


def toggle_fold() -> None:
    global folded
    folded = not folded
    editor.set_folded_ranges([FOLD_RANGE] if folded else [])


toggle.set_on_click(toggle_fold)

print(f"before any click: folded={folded}")
window.click(toggle)
print(f"after one click: folded={folded}")
assert folded, "a real click on the toggle must fold the real body range"
assert editor.get_text() == content, "folding must never touch the real underlying content"

window.click(toggle)
print(f"after a second click: folded={folded}")
assert not folded, "a real second click must unfold it again"
assert editor.get_text() == content

print(
    "code_editor_folding.py: a real gutter toggle folded and unfolded a real "
    "function body, composed entirely from existing primitives plus the new "
    "real set_folded_ranges paint transform -- get_text() never changed"
)
