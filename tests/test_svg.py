"""0.5.4 (#141): the `svg` node -- an SVG document painted as one scene."""

import gzip

import pytest

from tre import Window

RED = (255, 0, 0, 255)
BLUE = (0, 0, 255, 255)

# 20 x 10 user units: a red square on the left, a blue one on the right.
TWO_SQUARES = """<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10">
  <rect x="0" y="0" width="10" height="10" fill="#ff0000"/>
  <rect x="10" y="0" width="10" height="10" fill="#0000ff"/>
</svg>"""


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def shown(source, width=100, height=100, **props):
    window = Window(width=width, height=height)
    node = window.create(
        "svg", svg=source, width=width, height=height, position="absolute", x=0, y=0, **props
    )
    window.root.add_child(node)
    return window, node


def test_a_document_paints_fitted_into_the_box():
    # 20x10 into 100x100: scale 5, centred vertically -- 25..75 rows.
    window, node = shown(TWO_SQUARES)
    assert node.get("kind") == "svg"
    assert pixel(window, 25, 50) == RED
    assert pixel(window, 75, 50) == BLUE
    # The letterbox bars stay empty.
    assert pixel(window, 25, 10)[3] == 0


def test_it_reports_the_documents_own_size():
    _, node = shown(TWO_SQUARES)
    assert node.get("svg_size") == (20.0, 10.0)


def test_bytes_and_svgz_are_accepted():
    window, _ = shown(TWO_SQUARES.encode())
    assert pixel(window, 25, 50) == RED
    window, _ = shown(gzip.compress(TWO_SQUARES.encode()))
    assert pixel(window, 75, 50) == BLUE


def test_setting_a_new_document_replaces_the_old_one():
    window, node = shown(TWO_SQUARES)
    node.set(
        svg='<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">'
        '<rect width="10" height="10" fill="#00ff00"/></svg>'
    )
    assert node.get("svg_size") == (10.0, 10.0)
    assert pixel(window, 50, 50) == (0, 255, 0, 255)


def test_a_linear_gradient_ramps_across_the_shape():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#000"/>
      <stop offset="1" stop-color="#fff"/></linearGradient></defs>
      <rect width="100" height="100" fill="url(#g)"/></svg>"""
    window, _ = shown(svg)
    left, middle, right = (pixel(window, x, 50)[0] for x in (5, 50, 95))
    assert left < 40 and 100 < middle < 160 and right > 215


def test_a_clip_path_and_group_opacity_apply():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <defs><clipPath id="c"><rect width="50" height="100"/></clipPath></defs>
      <g clip-path="url(#c)" opacity="0.5">
        <rect width="100" height="100" fill="#000"/></g></svg>"""
    window, _ = shown(svg)
    inside = pixel(window, 25, 50)
    assert inside[3] in range(120, 136)
    assert pixel(window, 75, 50)[3] == 0


def test_a_stroke_is_drawn_with_its_width():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <line x1="0" y1="50" x2="100" y2="50" stroke="#0000ff" stroke-width="10"/></svg>"""
    window, _ = shown(svg)
    assert pixel(window, 50, 50) == BLUE
    assert pixel(window, 50, 40)[3] == 0
    assert pixel(window, 50, 60)[3] == 0


def test_the_box_background_shows_behind_the_document():
    window, _ = shown(TWO_SQUARES, fill=(0, 255, 0, 255))
    assert pixel(window, 25, 10) == (0, 255, 0, 255)
    assert pixel(window, 25, 50) == RED


def test_a_bad_document_is_a_value_error_and_creates_nothing():
    window = Window(width=50, height=50)
    with pytest.raises(ValueError, match="valid SVG"):
        window.create("svg", svg="not svg", width=10, height=10)
    with pytest.raises(ValueError, match="str or bytes"):
        window.create("svg", svg=3, width=10, height=10)
    with pytest.raises(ValueError, match="needs `svg`"):
        window.create("svg", width=10, height=10)


def test_svg_properties_apply_only_to_an_svg_node():
    window, node = shown(TWO_SQUARES)
    box = window.create("box")
    with pytest.raises(ValueError, match="applies only to an svg"):
        box.set(svg=TWO_SQUARES)
    with pytest.raises(ValueError, match="applies only to an svg"):
        box.get("svg_size")
    with pytest.raises(ValueError, match="read-only"):
        node.set(svg_size=(1, 1))


# 0.5.4 (#143): text, nested clips, patterns, a blur filter, nested SVG images,
# and what is still dropped without an error.

def svg(body, size=100):
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}">{body}</svg>'


def test_text_is_drawn_from_the_engines_fonts():
    window, _ = shown(svg('<text x="10" y="80" font-size="80" font-family="Roboto" fill="#ff0000">H</text>'))
    rgba, width, height = window.snapshot()
    red = sum(1 for i in range(0, width * height * 4, 4) if rgba[i] > 200 and rgba[i + 3] > 200)
    assert red > 300, f"the glyph should cover many pixels, got {red}"


def test_a_clip_inside_a_clip_keeps_only_the_overlap():
    window, _ = shown(svg(
        '<defs><clipPath id="a"><rect width="60" height="100"/></clipPath>'
        '<clipPath id="b" clip-path="url(#a)"><rect x="40" width="60" height="100"/></clipPath></defs>'
        '<rect width="100" height="100" fill="#ff0000" clip-path="url(#b)"/>'
    ))
    assert pixel(window, 50, 50) == RED
    assert pixel(window, 20, 50)[3] == 0
    assert pixel(window, 80, 50)[3] == 0


def test_a_pattern_tiles_across_the_shape():
    window, _ = shown(svg(
        '<defs><pattern id="p" width="20" height="20" patternUnits="userSpaceOnUse">'
        '<rect width="10" height="20" fill="#ff0000"/></pattern></defs>'
        '<rect width="100" height="100" fill="url(#p)"/>'
    ))
    assert pixel(window, 5, 50) == RED
    assert pixel(window, 15, 50)[3] == 0
    assert pixel(window, 25, 50) == RED
    assert pixel(window, 85, 50) == RED


def test_a_gaussian_blur_filter_softens_the_edge():
    window, _ = shown(svg(
        '<defs><filter id="b" x="-50%" y="-50%" width="200%" height="200%">'
        '<feGaussianBlur stdDeviation="5"/></filter></defs>'
        '<g filter="url(#b)"><rect x="30" y="30" width="40" height="40" fill="#000000"/></g>'
    ))
    assert pixel(window, 50, 50)[3] > 240
    assert 40 < pixel(window, 30, 50)[3] < 215, "the edge is half-covered"
    assert 0 < pixel(window, 22, 50)[3] < 120, "the blur spreads past the shape"


def test_a_nested_svg_image_is_drawn_scaled_into_its_box():
    import base64
    inner = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>'
    href = "data:image/svg+xml;base64," + base64.b64encode(inner.encode()).decode()
    window, _ = shown(svg(f'<image x="0" y="0" width="50" height="50" href="{href}"/>'))
    assert pixel(window, 25, 25) == RED
    assert pixel(window, 75, 75)[3] == 0


def test_a_raster_image_and_a_mask_are_dropped_without_an_error():
    import base64
    png = base64.b64decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
    )
    href = "data:image/png;base64," + base64.b64encode(png).decode()
    window, _ = shown(svg(
        f'<image width="20" height="20" href="{href}"/>'
        '<defs><mask id="m"><rect width="100" height="100" fill="#fff"/></mask></defs>'
        '<rect x="60" width="40" height="100" fill="#0000ff" mask="url(#m)"/>'
        '<rect x="0" y="60" width="40" height="40" fill="#ff0000"/>'
    ))
    assert pixel(window, 20, 80) == RED, "the rest of the document still draws"


def test_a_pattern_covers_the_whole_shape_when_the_document_is_scaled_and_rotated():
    # 50 user units shown at 100px (scale 2), tiles rotated 30 degrees: tiles
    # must still reach the far corner.
    window, _ = shown(svg(
        '<defs><pattern id="p" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(30)">'
        '<rect width="6" height="6" fill="#ff0000"/></pattern></defs>'
        '<rect width="50" height="50" fill="url(#p)"/>', size=50
    ))
    for x, y in ((5, 5), (95, 5), (5, 95), (95, 95), (50, 50)):
        assert pixel(window, x, y) == RED, (x, y)
