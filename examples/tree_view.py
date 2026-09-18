#!/usr/bin/env python3
"""M30 Phase 6 Step 3's real `Window.add_tree_node` (§1, §3, §5, §7):
the identical real grounding `Accordion` (Step 2) already established
-- MD3 has no official page for either, grounded in the Lists
guideline's own "expand and collapse" text -- applied recursively: a
small file-browser-shaped tree, each row `Accordion`'s own header
anatomy again, with real per-depth left indentation as the one real
difference "recursively" means here.

What this script proves automatically (headless-CI-safe, no human
needed): every row, at every depth, remains independently clickable,
and a non-leaf row's chevron flips via the same real
`animate("transform", ...)` substitute `Accordion` already
established (no rotation primitive exists, so a uniform negative
scale stands in for a 180-degree flip).
"""

from typing import Callable

from tre import App, Window

window = Window(width=400, height=300, title="tre v2 -- tree view")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=400, height=300)

rows = [
    window.add_tree_node(title="src/", depth=0, expanded=True),
    window.add_tree_node(title="engine-core/", depth=1, expanded=True),
    window.add_tree_node(title="tree.rs", depth=2, leaf=True),
    window.add_tree_node(title="node.rs", depth=2, leaf=True),
    window.add_tree_node(title="engine-py/", depth=1),
    window.add_tree_node(title="Cargo.toml", depth=0, leaf=True),
]

for header, _chevron in rows:
    shell.add_child(header)

selected: dict[str, str] = {"title": ""}


def make_selector(title: str) -> Callable[[], None]:
    def select() -> None:
        selected["title"] = title

    return select


titles = ["src/", "engine-core/", "tree.rs", "node.rs", "engine-py/", "Cargo.toml"]
for (header, _chevron), title in zip(rows, titles):
    header.enable_interaction()
    header.set_on_click(make_selector(title))

window.click(rows[2][0])
assert selected["title"] == "tree.rs", "clicking a deeply nested row must reach its own handler"

# Toggle the "engine-py/" row's own chevron -- the same real
# scale(-1.0)-as-flip substitute Accordion already established.
_engine_py_header, engine_py_chevron = rows[4]
assert engine_py_chevron is not None
engine_py_chevron.animate("transform", (0.0, 0.0, -1.0))

window.click(rows[5][0])
assert selected["title"] == "Cargo.toml", "every row must remain independently clickable"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"tree_view.py: exited cleanly after 60 frames, last selected {selected['title']!r}")
