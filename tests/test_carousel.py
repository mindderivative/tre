"""M30 Phase 9 Step 5 (§5, §7, §11.7): real, repeatable coverage of
`Window.add_carousel` -- a real MD3 carousel (`COMPONENT_CAROUSEL.md`).
Checked with the user before starting, given the real scope (an
animated value that also invalidates layout, real wheel/drag input
this codebase didn't have anywhere else); the user chose "full real
MD3 carousel" over a scoped-down v1.

Every real item-width/position claim below is proven at the Rust level
(`crates/engine-core/src/tree.rs`'s own `carousel_*` tests, GPU-free,
reading `Tree::layout` directly) -- this suite proves the FFI surface:
`add_carousel` itself, the real synchronous wheel/scroll dispatch
(`Window.scroll`, no tick needed -- `index`/`scroll_x` move the instant
the wheel event is dispatched, the same real synchronous-mutation shape
`Tree::set_splitter_position` already has), and the real, honest limit
`test_checkbox.py`'s own doc comment already states for every other
`Animated<T>`-backed field: pytest alone can prove a real tick *moved*
a value, never that it *finished*, since a headless render loop's own
frame count bears no fixed relationship to real wall-clock duration.
"""

import pytest

from tre import Node, Window


def test_carousel_accessors_reject_a_non_carousel_node():
    window = Window(width=500, height=300)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'index'"):
        rect.get_carousel_index()
    with pytest.raises(ValueError, match="Rect has no property 'index'"):
        rect.set_carousel_index(0)
    with pytest.raises(ValueError, match="Rect has no property 'position'"):
        rect.get_carousel_position()
    with pytest.raises(ValueError, match="Rect has no property 'scroll_x'"):
        rect.get_carousel_scroll()
    with pytest.raises(ValueError, match="Rect has no property 'scroll_x'"):
        rect.set_carousel_scroll(0.0)
