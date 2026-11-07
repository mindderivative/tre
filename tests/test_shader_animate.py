"""#270 (TRE-REQ-013 step 2, part A): `Shader.animate` moves a uniform with the engine's own
animation clock, so a join can grow with no Python per frame. Pixels come from
`Window.snapshot()`; time from `Window.advance()`."""

import pytest

from tess_re import Shader, Window

LEVEL = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.level, 0.0, 0.0, 1.0);\n}\n"
TINT = "fn shade(p: Pixel) -> vec4<f32> {\n    return u.tint;\n}\n"


def scene(shader, size=20):
    window = Window(width=40, height=40)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    box = window.create("box", width=size, height=size)
    window.root.add_child(box)
    box.set(shader=shader)
    return window, box


def red_at(window, x=10, y=10):
    rgba, width, _ = window.snapshot()
    return rgba[(y * width + x) * 4]


def test_a_uniform_eases_to_its_value_and_stops_there():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window, _ = scene(shader)
    shader.animate("level", 1.0, 200)
    window.advance(1)  # starts it on the window's clock
    assert shader.uniforms["level"] == pytest.approx(0.0, abs=0.02)
    window.advance(100)
    assert shader.uniforms["level"] == pytest.approx(0.5, abs=0.02)
    assert 118 <= red_at(window) <= 138
    window.advance(500)
    assert shader.uniforms["level"] == 1.0
    assert red_at(window) == 255


def test_a_vector_uniform_animates_every_component():
    shader = Shader(TINT, uniforms={"tint": (0.0, 0.0, 0.0, 1.0)})
    window, _ = scene(shader)
    shader.animate("tint", (1.0, 0.5, 0.25, 1.0), 100, easing="linear")
    window.advance(1)
    window.advance(50)
    r, g, b, a = shader.uniforms["tint"]
    assert (r, g, b, a) == pytest.approx((0.5, 0.25, 0.125, 1.0), abs=0.03)
    window.advance(200)
    assert shader.uniforms["tint"] == (1.0, 0.5, 0.25, 1.0)


def test_asking_again_retargets_from_where_it_is():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window, _ = scene(shader)
    shader.animate("level", 1.0, 200)
    window.advance(1)
    window.advance(100)
    halfway = shader.uniforms["level"]
    shader.animate("level", 0.0, 100)
    window.advance(1)
    assert shader.uniforms["level"] == pytest.approx(halfway, abs=0.02)
    window.advance(200)
    assert shader.uniforms["level"] == 0.0


def test_set_stops_a_running_animation():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window, _ = scene(shader)
    shader.animate("level", 1.0, 200)
    window.advance(1)
    window.advance(50)
    shader.set(uniforms={"level": 0.25})
    window.advance(500)
    assert shader.uniforms["level"] == 0.25


def test_every_node_using_the_shader_follows():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window = Window(width=80, height=30)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    boxes = []
    for _ in range(2):
        box = window.create("box", width=20, height=20)
        window.root.add_child(box)
        box.set(shader=shader)
        boxes.append(box)
    shader.animate("level", 1.0, 100)
    window.advance(1)
    window.advance(300)
    for box in boxes:
        assert red_at(window, int(box.get("layout_x")) + 10, 10) == 255


def test_a_zero_duration_snaps_on_the_next_tick():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window, _ = scene(shader)
    shader.animate("level", 1.0)
    window.advance(1)
    assert shader.uniforms["level"] == 1.0


def test_a_spring_settles_on_the_target():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    window, _ = scene(shader)
    shader.animate("level", 1.0, 200, easing="spring")
    window.advance(1)
    window.advance(5000)
    assert shader.uniforms["level"] == pytest.approx(1.0, abs=1e-6)


@pytest.mark.parametrize(
    "name, to, error, match",
    [
        ("nope", 1.0, ValueError, "no uniform `nope`"),
        ("level", (1.0, 2.0), ValueError, "level"),
        ("level", "x", TypeError, "level"),
    ],
)
def test_a_bad_name_or_value_raises_and_changes_nothing(name, to, error, match):
    shader = Shader(LEVEL, uniforms={"level": 0.5})
    with pytest.raises(error, match=match):
        shader.animate(name, to, 100)
    assert shader.uniforms == {"level": 0.5}


def test_a_bad_easing_is_a_value_error():
    shader = Shader(LEVEL, uniforms={"level": 0.5})
    with pytest.raises(ValueError, match="easing"):
        shader.animate("level", 1.0, 100, easing="bouncy")


# The request's acceptance scene: a body joined to a band, the blend radius animated by the engine.
# The band is the top 10 px of a 100 x 60 node; the body hangs from it at the left, 40 px wide.
JOIN = """
fn shade(p: Pixel) -> vec4<f32> {
    let q = p.px / frame.scale;
    let band = sd_box(q - vec2<f32>(50.0, 5.0), vec2<f32>(50.0, 5.0));
    let body = sd_box(q - vec2<f32>(20.0, 35.0), vec2<f32>(20.0, 25.0));
    let d = sd_round_union(band, body, u.r);
    return vec4<f32>(1.0, 0.0, 0.0, sd_coverage(d * frame.scale));
}
"""


def join_scene(scale=None):
    shader = Shader(JOIN, uniforms={"r": 0.0})
    window = Window(width=100, height=60)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    box = window.create("box", width=100, height=60)
    window.root.add_child(box)
    box.set(shader=shader)
    return window, shader


def shot(window, scale=1.0):
    rgba, width, _ = window.snapshot(scale=scale)
    return lambda x, y: rgba[(int(y * scale) * width + int(x * scale)) * 4]


def test_a_join_grows_its_fillet_as_the_radius_animates():
    window, shader = join_scene()
    at = shot(window)
    assert at(41, 11) == 0, "no blend: a square inside corner"
    shader.animate("r", 12.0, 100, easing="linear")
    window.advance(1)
    window.advance(300)
    at = shot(window)
    assert at(41, 11) > 230, "the fillet fills the corner"
    assert at(50, 20) == 0, "but not the middle of its circle"
    assert at(20, 40) > 230 and at(70, 5) > 230, "the body and the band are as they were"
    assert at(70, 30) == 0, "and the notch beside the body stays open"


def test_the_fillet_is_smooth_at_one_and_a_half_times_the_scale():
    window, shader = join_scene()
    shader.set(uniforms={"r": 12.0})
    at = shot(window, 1.5)
    # The fillet's arc is 12 logical px from its centre (52, 22): (42.5, 12.5) is 13.4 away, on the
    # corner's side, (44.5, 14.5) is 10.6 away.
    assert at(42.5, 12.5) > 200 and at(44.5, 14.5) == 0
    assert at(20, 40) > 230 and at(70, 30) == 0


def test_two_windows_sharing_a_shader_each_end_on_the_last_value():
    shader = Shader(LEVEL, uniforms={"level": 0.0})
    first, _ = scene(shader)
    second, _ = scene(shader)
    shader.animate("level", 1.0, 100, easing="linear")
    first.advance(1)
    second.advance(1)
    first.advance(500)  # the first window takes the last step
    assert shader.uniforms["level"] == 1.0
    second.advance(1)  # the second has not looked since
    assert red_at(first) == 255 and red_at(second) == 255
