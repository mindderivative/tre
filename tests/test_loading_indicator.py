"""M39 Phase 2 (§5, §7): real, repeatable coverage of `Window.
add_loading_indicator` -- MD3 Expressive's own real, perpetually-
looping shape-morph spinner (a real, simplified v1: four real,
procedurally-generated shapes -- Pentagon, Pill, Cookie, Oval -- with
plain eased morphing rather than genuine spring physics, scoped via
`AskUserQuestion`). There is no Python getter for the raw `shape`
animation target or `current_shape` cycle position -- the same real
verification-surface limit already established repeatedly this whole
project. The real looping mechanism itself is proven directly at the
Rust level (`crates/engine-core/src/tree.rs`'s own `tick_all_*_
loading_indicator*` tests) and via a real, live, multi-frame render
loop (`examples/loading_indicator.py`); this file proves the real FFI
construction surface.
"""

from tre import Node, Window


def test_add_loading_indicator_returns_a_node():
    window = Window(width=400, height=200)
    indicator = window.add_loading_indicator()
    assert isinstance(indicator, Node)


def test_a_themed_loading_indicator_does_not_raise():
    window = Window(width=400, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    indicator = window.add_loading_indicator()
    assert isinstance(indicator, Node)


def test_a_custom_size_does_not_raise():
    window = Window(width=400, height=200)
    indicator = window.add_loading_indicator(size=96.0)
    assert isinstance(indicator, Node)


def test_a_custom_color_does_not_raise():
    window = Window(width=400, height=200)
    indicator = window.add_loading_indicator(color=(0xB0, 0x00, 0x20, 0xFF))
    assert isinstance(indicator, Node)


def test_multiple_indicators_in_the_same_window_do_not_raise():
    window = Window(width=400, height=200)
    first = window.add_loading_indicator(x=0.0, y=0.0)
    second = window.add_loading_indicator(size=24.0, x=100.0, y=0.0)
    assert first is not second
