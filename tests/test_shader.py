"""0.5.1 (#66): `tre.Shader` -- WGSL checked when it is made. The checking
itself (assembly, naga, positions, layout) is unit-tested in `engine-core`;
here, the Python surface."""

import pytest

from tre import Shader, ShaderError, Window

PIXELS = {"rgba": bytes([255, 0, 0, 255] * 4), "pixel_width": 2, "pixel_height": 2}


def image(window):
    return window.create("image", width=10, height=10, **PIXELS)


GOOD = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(p.uv, 0.0, 1.0);\n}\n"


def test_a_valid_shader_reads_back():
    shader = Shader(GOOD)
    assert shader.wgsl == GOOD
    assert shader.mode == "fill"
    assert shader.animated is False
    assert shader.uniforms == {}
    assert shader.inputs == {}
    assert "fill" in repr(shader)


def test_a_mistake_raises_shader_error_with_a_position():
    source = "fn shade(p: Pixel) -> vec4<f32> {\n    let x = ;\n    return vec4<f32>(1.0);\n}\n"
    with pytest.raises(ShaderError) as caught:
        Shader(source)
    error = caught.value
    assert isinstance(error, ValueError)
    assert error.line == 2
    assert error.source_line == "    let x = ;"
    assert error.column is not None
    assert "(line 2, column" in str(error)


def test_an_error_without_a_position_has_none():
    with pytest.raises(ShaderError) as caught:
        Shader("fn other() {}\n")
    assert (caught.value.line, caught.value.column, caught.value.source_line) == (None, None, None)
    assert "fn shade(p: Pixel) -> vec4<f32>" in str(caught.value)


def test_uniforms_take_numbers_and_vectors():
    source = (
        "fn shade(p: Pixel) -> vec4<f32> {\n"
        "    return vec4<f32>(u.a * u.b.x, u.c.y, u.d.z, u.e.w);\n}\n"
    )
    shader = Shader(source, uniforms={"a": 1, "b": (0.5, 2), "c": [1.0, 2.0, 3.0], "d": (1, 2, 3), "e": (1, 2, 3, 4)})
    assert shader.uniforms == {"a": 1.0, "b": (0.5, 2.0), "c": (1.0, 2.0, 3.0), "d": (1.0, 2.0, 3.0), "e": (1.0, 2.0, 3.0, 4.0)}


@pytest.mark.parametrize(
    "value, error",
    [
        (True, TypeError),
        ("x", TypeError),
        (None, TypeError),
        ((1.0,), ValueError),
        ((1, 2, 3, 4, 5), ValueError),
        ((1, "a"), TypeError),
        (float("nan"), ValueError),
        (float("inf"), ValueError),
    ],
)
def test_bad_uniform_values_are_refused(value, error):
    with pytest.raises(error):
        Shader(GOOD, uniforms={"a": value})


def test_bad_names_and_modes_are_refused():
    with pytest.raises(ShaderError, match="isn't a WGSL identifier"):
        Shader(GOOD, uniforms={"1a": 1})
    with pytest.raises(ShaderError, match="keyword"):
        Shader(GOOD, uniforms={"var": 1})
    with pytest.raises(TypeError, match="uniform names must be str"):
        Shader(GOOD, uniforms={1: 1})
    with pytest.raises(ValueError, match='"fill" or "effect"'):
        Shader(GOOD, mode="blend")


def test_an_effect_reads_content_and_a_fill_cannot():
    effect = "fn shade(p: Pixel) -> vec4<f32> {\n    return content(p.uv);\n}\n"
    assert Shader(effect, mode="effect").mode == "effect"
    with pytest.raises(ShaderError):
        Shader(effect)


def test_set_replaces_uniforms_atomically():
    source = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.amount);\n}\n"
    shader = Shader(source, uniforms={"amount": 0.25})
    shader.set(uniforms={"amount": 0.75})
    assert shader.uniforms == {"amount": 0.75}
    with pytest.raises(ShaderError):
        shader.set(uniforms={"other": 1.0})  # `u.amount` no longer exists
    assert shader.uniforms == {"amount": 0.75}, "a failed set changes nothing"
    with pytest.raises(TypeError):
        shader.set(uniforms={"amount": "x"})
    assert shader.uniforms == {"amount": 0.75}


def test_inputs_are_nodes_of_one_window():
    source = "fn shade(p: Pixel) -> vec4<f32> {\n    return input_photo(p.uv);\n}\n"
    window = Window()
    photo = image(window)
    shader = Shader(source, inputs={"photo": photo})
    assert shader.inputs["photo"] == photo
    with pytest.raises(TypeError, match="must be a Node"):
        Shader(source, inputs={"photo": 3})
    other = image(Window())
    with pytest.raises(ValueError, match="one Window"):
        Shader(source, inputs={"photo": photo, "other": other})
    photo.destroy()
    with pytest.raises(ValueError, match="destroyed"):
        Shader(source, inputs={"photo": photo})


def test_equality_is_identity():
    a, b = Shader(GOOD), Shader(GOOD)
    assert a == a and a != b
    assert len({a, b, a}) == 2


# --- the node property -------------------------------------------------------------


def test_every_node_kind_takes_a_shader():
    window = Window()
    shader = Shader(GOOD)
    kinds = ["box", "text", "text_input", "image", "path", "canvas", "scroll_view"]
    for kind in kinds:
        try:
            node = window.create(kind, width=20, height=20)
        except ValueError:
            continue  # a kind that needs arguments is covered below
        assert node.get("shader") is None
        node.set(shader=shader)
        assert node.get("shader") == shader, kind
        node.set(shader=None)
        assert node.get("shader") is None, kind


def test_shader_is_a_create_property_and_is_shared():
    window = Window()
    shader = Shader(
        "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.amount);\n}\n",
        uniforms={"amount": 0.5},
    )
    a = window.create("box", width=10, height=10, shader=shader)
    b = window.create("box", width=10, height=10, shader=shader)
    assert a.get("shader") == b.get("shader") == shader
    a.get("shader").set(uniforms={"amount": 1.0})
    assert shader.uniforms == {"amount": 1.0}
    assert b.get("shader").uniforms == {"amount": 1.0}


def test_a_bad_shader_value_changes_nothing():
    window = Window()
    node = window.create("box", width=10, height=10)
    shader = Shader(GOOD)
    node.set(shader=shader)
    with pytest.raises(ValueError, match="a tre.Shader or None"):
        node.set(shader="void main() {}", opacity=0.5)
    assert node.get("shader") == shader
    assert node.get("opacity") == 1.0, "atomic: the valid half wasn't applied"


def test_inputs_must_be_in_the_nodes_own_window():
    source = "fn shade(p: Pixel) -> vec4<f32> {\n    return input_photo(p.uv);\n}\n"
    window, other = Window(), Window()
    photo = image(window)
    shader = Shader(source, inputs={"photo": photo})
    window.create("box", width=10, height=10, shader=shader)
    foreign = other.create("box", width=10, height=10)
    with pytest.raises(ValueError, match="different Window"):
        foreign.set(shader=shader)
    assert foreign.get("shader") is None
    assert window.create("box", shader=shader).get("shader").inputs["photo"] == photo


def test_the_property_is_listed_in_the_unknown_property_error():
    node = Window().create("box", width=10, height=10)
    with pytest.raises(ValueError, match="shader"):
        node.set(colour=1)


# --- inputs (#68) --------------------------------------------------------------------

READ = "fn shade(p: Pixel) -> vec4<f32> {\n    return input_x(p.uv);\n}\n"


def test_an_input_is_an_image_node_or_a_node_with_a_shader():
    window = Window()
    img = image(window)
    plain = window.create("box", width=10, height=10)
    assert Shader(READ, inputs={"x": img}).inputs["x"] == img
    with pytest.raises(ValueError, match="image or video node, or a node that has a shader"):
        Shader(READ, inputs={"x": plain})
    plain.set(shader=Shader(GOOD))
    assert Shader(READ, inputs={"x": plain}).inputs["x"] == plain


def test_a_shader_that_would_read_itself_is_refused():
    window = Window()
    a = window.create("box", width=10, height=10, shader=Shader(GOOD))
    b = window.create("box", width=10, height=10, shader=Shader(READ, inputs={"x": a}))
    # a reading b, which reads a.
    with pytest.raises(ValueError, match="would read itself"):
        a.set(shader=Shader(READ, inputs={"x": b}))
    assert a.get("shader").inputs == {}, "nothing changed"
    # A node reading itself.
    with pytest.raises(ValueError, match="would read itself"):
        a.set(shader=Shader(READ, inputs={"x": a}))


def test_an_image_node_may_read_its_own_pixels():
    window = Window()
    img = image(window)
    img.set(shader=Shader(READ, inputs={"x": img}))
    assert img.get("shader").inputs["x"] == img


# --- animation (#70) -----------------------------------------------------------------

CLOCK = "fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(fract(frame.time), 0.0, 0.0, 1.0);\n}\n"


def test_a_live_window_runs_with_an_animated_shader():
    """A smoke test of the real loop: an animated shader draws, with its time
    from the window clock, through `App.run`. (`max_frames` runs frames
    whether or not the loop would sleep, so it can't show the keep-awake
    rule itself; that is `has_animated_shader` in `engine-render`'s tests.)"""
    import os
    import subprocess
    import sys
    import textwrap

    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        pytest.skip("no display reachable")
    script = textwrap.dedent(f"""
        from tre import App, Shader, Window
        w = Window(width=120, height=80)
        box = w.create("box", width=60, height=60, shader=Shader({CLOCK!r}, animated=True))
        w.root.add_child(box)
        app = App()
        app.add_window(w)
        app.run(max_frames=30)
        print("RAN")
    """)
    done = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stderr
    assert done.stdout.strip().splitlines()[-1] == "RAN"
