"""#271 (TRE-REQ-013 step 3): `morph="structure"` morphs a path's data number by number when
both paths have the same commands, so arcs stay arcs and no corner is smeared; the default still
resamples. Pixels come from `Window.snapshot()`, time from `Window.advance()`."""

import re

import pytest

from tess_re import Window


def rounded(w, h, r):
    return (
        f"M{r},0 H{w - r} A{r},{r} 0 0 1 {w},{r} V{h - r} A{r},{r} 0 0 1 {w - r},{h} "
        f"H{r} A{r},{r} 0 0 1 0,{h - r} V{r} A{r},{r} 0 0 1 {r},0 Z"
    )


def commands(data):
    return re.findall(r"[MLCQZ]", data)


def scene(data, **props):
    window = Window(width=120, height=80)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    path = window.create("path", data=data, width=100, height=60, fill=(255, 0, 0, 255), **props)
    window.root.add_child(path)
    return window, path


def red(window, x, y):
    rgba, width, _ = window.snapshot()
    return rgba[(y * width + x) * 4]


def start(window, path, to, ms=100):
    path.animate("data", to, ms, easing="linear")
    window.advance(1)
    window.advance(ms // 2)


def test_the_default_is_to_resample_so_nothing_changes():
    window, path = scene(rounded(100, 60, 4))
    assert path.get("morph") == "resample"
    start(window, path, rounded(100, 60, 20))
    assert len(commands(path.get("data"))) > 90, "a polygon of sampled points"


def test_a_structure_morph_keeps_the_arcs_and_the_commands():
    window, path = scene(rounded(100, 60, 4), morph="structure")
    assert path.get("morph") == "structure"
    before = commands(path.get("data"))
    start(window, path, rounded(100, 60, 20))
    assert commands(path.get("data")) == before, "four curves still, no polygon"
    # Halfway the corner is the radius 12 arc about (12, 12): 10.6 inside it, 13.4 outside.
    ox, oy = int(path.get("layout_x")), int(path.get("layout_y"))
    assert red(window, ox + 4, oy + 4) > 200, "inside the arc"
    assert red(window, ox + 2, oy + 2) == 0, "the corner is cut, not smeared"
    window.advance(500)
    assert path.get("data") == window.create("path", data=rounded(100, 60, 20)).get("data")


def test_it_falls_back_to_resampling_when_the_commands_differ():
    window, path = scene(rounded(100, 60, 4), morph="structure")
    start(window, path, "M0,0 L100,0 L100,60 L0,60 Z")
    assert len(commands(path.get("data"))) > 90
    window.advance(500)
    assert commands(path.get("data")) == ["M", "L", "L", "L", "Z"], "and ends exactly on the target"


def test_strict_refuses_a_pair_with_different_commands_and_changes_nothing():
    window, path = scene(rounded(100, 60, 4), morph="strict")
    before = path.get("data")
    with pytest.raises(ValueError, match="strict"):
        path.animate("data", "M0,0 L100,0 L100,60 Z", 100)
    assert path.get("data") == before
    start(window, path, rounded(100, 60, 20))  # equal commands are fine
    assert len(commands(path.get("data"))) < 20


def test_a_bad_mode_is_a_value_error_and_only_a_path_has_one():
    window, path = scene(rounded(100, 60, 4))
    with pytest.raises(ValueError, match="morph"):
        path.set(morph="smooth")
    box = window.create("box", width=10, height=10)
    with pytest.raises(ValueError, match="morph"):
        box.set(morph="structure")
