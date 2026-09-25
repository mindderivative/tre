"""M95 Phase 1: `window.create` for `box` and `path`, and a path's own
properties -- SVG `data`, `view_box`, stroke trim -- through `set`, `get`,
and `animate`. The geometry itself (fitting, trimming, morphing) is
covered by `engine-core`'s own `path` tests; its paint, by M95 Phase 3's
pixel tests.
"""

from __future__ import annotations

from typing import Any

import pytest

import tre

TRIANGLE = "M0,0 L24,0 L24,24 Z"


def window() -> tre.Window:
    return tre.Window(300, 200, "paths")


def test_create_makes_a_detached_path_with_its_props() -> None:
    w = window()
    path = w.create("path", data=TRIANGLE, view_box=(0, 0, 24, 24), width=48, height=48)
    assert path.get("data") == TRIANGLE
    assert path.get("view_box") == (0.0, 0.0, 24.0, 24.0)
    assert (path.get("width"), path.get("height")) == (48.0, 48.0)
    assert (path.get("trim_start"), path.get("trim_end")) == (0.0, 1.0)
    w.root.add_child(path)


def test_create_makes_a_box() -> None:
    w = window()
    box = w.create("box", width=100, height="50%")
    assert (box.get("width"), box.get("height")) == (100.0, "50%")
    assert w.create("box").get("width") == "auto"


def test_path_props_set_and_clear() -> None:
    w = window()
    path = w.create("path", data=TRIANGLE)
    path.set(data="M0,0 L10,10", view_box=(-5, -5, 10, 10), trim_start=0.25, trim_end=0.75)
    assert path.get("data") == "M0,0 L10,10"
    assert path.get("view_box") == (-5.0, -5.0, 10.0, 10.0)
    assert (path.get("trim_start"), path.get("trim_end")) == (0.25, 0.75)
    path.set(view_box=None)
    assert path.get("view_box") is None


@pytest.mark.parametrize(
    ("kind", "props", "message"),
    [
        ("path", {}, 'create\\("path"\\) needs `data`'),
        ("circle", {}, "create builds: box, path"),
        ("box", {"data": TRIANGLE}, "applies only to a path node"),
        ("path", {"data": "M0,0 Xzz"}, "isn't valid SVG path data"),
        ("path", {"data": TRIANGLE, "trim_end": 1.5}, "from 0.0 to 1.0"),
        ("path", {"data": TRIANGLE, "view_box": (0, 0, 0, 10)}, "positive size"),
        ("box", {"width": "wide"}, 'a percentage like "50%"'),
    ],
)
def test_create_rejects_bad_kinds_and_props(
    kind: str, props: dict[str, Any], message: str
) -> None:
    w = window()
    with pytest.raises(ValueError, match=message):
        w.create(kind, **props)


def test_animate_accepts_path_data_and_trim() -> None:
    w = window()
    path = w.create("path", data=TRIANGLE)
    path.animate("data", "M0,0 L12,12 L0,24 Z", duration_ms=200)
    path.animate("trim_end", 0.5, duration_ms=200)
    with pytest.raises(ValueError, match="isn't valid SVG path data"):
        path.animate("data", "nonsense")
    with pytest.raises(ValueError, match="from 0.0 to 1.0"):
        path.animate("trim_start", 3.0)
    box = w.create("box")
    with pytest.raises(ValueError, match="applies only to a path node"):
        box.animate("trim_end", 0.5)
