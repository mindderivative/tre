"""0.5.1 (#66): `tre.Shader` -- WGSL checked when it is made. The checking
itself (assembly, naga, positions, layout) is unit-tested in `engine-core`;
here, the Python surface."""

import pytest

from tre import Shader, ShaderError, Window

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
    photo = window.create("box", width=10, height=10)
    shader = Shader(source, inputs={"photo": photo})
    assert shader.inputs["photo"] == photo
    with pytest.raises(TypeError, match="must be a Node"):
        Shader(source, inputs={"photo": 3})
    other = Window().create("box", width=10, height=10)
    with pytest.raises(ValueError, match="one Window"):
        Shader(source, inputs={"photo": photo, "other": other})
    photo.destroy()
    with pytest.raises(ValueError, match="destroyed"):
        Shader(source, inputs={"photo": photo})


def test_equality_is_identity():
    a, b = Shader(GOOD), Shader(GOOD)
    assert a == a and a != b
    assert len({a, b, a}) == 2
