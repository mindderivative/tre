"""0.5.6.2 (#179): corner and centre placements, `offset` and `flip` for layers.

`placement` also names a *point* of the anchor -- `top_start`, `top_end`,
`bottom_start`, `bottom_end`, `center` -- and the layer's centre goes there, for a
badge or a marker; a point is never flipped or shifted to fit. `offset=(dx, dy)`
moves the point the layer attaches to, in the anchor's own coordinates, and
`flip=False` keeps a side placement from flipping or shifting as well.
"""

import pytest

import tre

W, H = 400, 300


def scene(anchor_x=100, anchor_y=100, anchor_w=60, anchor_h=30, **layer_args):
    window = tre.Window(W, H, "layer points")
    window.root.set(padding=0, gap=0)
    anchor = window.create(
        "box", position="absolute", x=anchor_x, y=anchor_y, width=anchor_w, height=anchor_h
    )
    window.root.add_child(anchor)
    layer = window.create("box", width=20, height=10)
    window.show_layer(layer, anchor=anchor, **layer_args)
    window.advance(16)
    return window, anchor, layer


def at(layer):
    return (layer.get("layout_x"), layer.get("layout_y"))


@pytest.mark.parametrize(
    "placement, expected",
    [
        # The layer is 20 x 10, so its top-left is its centre less (10, 5).
        ("top_start", (100 - 10, 100 - 5)),
        ("top_end", (160 - 10, 100 - 5)),
        ("bottom_start", (100 - 10, 130 - 5)),
        ("bottom_end", (160 - 10, 130 - 5)),
        ("center", (130 - 10, 115 - 5)),
    ],
)
def test_a_point_places_the_layers_centre_on_it(placement, expected):
    _, _, layer = scene(placement=placement)
    assert at(layer) == expected


def test_the_four_sides_are_unchanged():
    assert at(scene(placement="below")[2]) == (100, 130)
    assert at(scene(placement="above")[2]) == (100, 90)
    assert at(scene(placement="start")[2]) == (80, 100)
    assert at(scene(placement="end")[2]) == (160, 100)


def test_a_point_is_never_flipped_or_shifted_to_fit():
    # Centred on the window's top-left corner: half of it is off the window, and
    # it stays there. A side placement would have been shifted back inside.
    _, _, layer = scene(anchor_x=0, anchor_y=0, placement="top_start")
    assert at(layer) == (-10, -5)
    _, _, side = scene(anchor_x=0, anchor_y=0, placement="above")
    assert at(side)[1] >= 0


def test_offset_moves_the_attachment_point():
    _, _, layer = scene(placement="top_end", offset=(-8, 6))
    assert at(layer) == (160 - 10 - 8, 100 - 5 + 6)
    _, _, below = scene(placement="below", offset=(0, 4))
    assert at(below) == (100, 134)


def test_offset_is_in_the_anchors_coordinates_so_a_scaled_anchor_scales_it():
    window = tre.Window(W, H, "scaled")
    window.root.set(padding=0, gap=0)
    anchor = window.create(
        "box", position="absolute", x=100, y=100, width=60, height=30, scale=2.0
    )
    window.root.add_child(anchor)
    plain = window.create("box", width=20, height=10)
    shifted = window.create("box", width=20, height=10)
    window.show_layer(plain, anchor=anchor, placement="top_start")
    window.show_layer(shifted, anchor=anchor, placement="top_start", offset=(5, 5))
    window.advance(16)
    # The anchor draws twice as big, so a 5 px offset in its coordinates moves
    # the layer 10 px, wherever the scale is centred.
    assert (at(shifted)[0] - at(plain)[0], at(shifted)[1] - at(plain)[1]) == (10, 10)


def test_flip_false_keeps_a_side_where_it_was_put():
    # `below` an anchor at the bottom of the window would flip above it.
    _, _, flipped = scene(anchor_y=H - 30, placement="below")
    assert at(flipped)[1] < H - 30
    _, _, pinned = scene(anchor_y=H - 30, placement="below", flip=False)
    assert at(pinned) == (100, H)


def test_the_placement_used_is_reported_by_name():
    _, _, layer = scene(placement="bottom_end")
    assert layer.get("layer_placement") == "bottom_end"


def test_a_bad_placement_or_offset_is_refused():
    window = tre.Window(W, H, "bad")
    anchor = window.create("box", width=10, height=10)
    window.root.add_child(anchor)
    layer = window.create("box", width=5, height=5)
    with pytest.raises(ValueError, match="placement"):
        window.show_layer(layer, anchor=anchor, placement="middle")
    with pytest.raises(ValueError, match="offset"):
        window.show_layer(layer, anchor=anchor, offset=(float("nan"), 0))
