"""M97 Phase 2 Step 3: `docs/design/md3-handover.json` and its page -- current,
and matching what the live engine resolves. The engine's half is checked
for staleness by `crates/engine-md3/tests/handover.rs`; this checks the
half `tools/md3_handover.py` adds, and cross-checks the whole file against
`window.theme`.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

import tre

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import md3_handover  # noqa: E402

DATA: dict[str, Any] = json.loads(md3_handover.HANDOVER_JSON.read_text())


def rgba(hex_: str) -> tuple[int, int, int, int]:
    return (int(hex_[1:3], 16), int(hex_[3:5], 16), int(hex_[5:7], 16), 255)


def test_the_file_and_page_are_current() -> None:
    data = md3_handover.handover()
    assert DATA == data, "run `python tools/md3_handover.py`"
    assert md3_handover.HANDOVER_MD.read_text() == md3_handover.page(data)


@pytest.mark.parametrize("seed", sorted(DATA["color"]["seeds"]))
@pytest.mark.parametrize("dark", [False, True])
def test_every_role_is_what_the_engine_resolves(seed: str, dark: bool) -> None:
    w = tre.Window(10, 10, "theme")
    w.set_theme(rgba(seed), dark=dark)
    scheme = DATA["color"]["seeds"][seed]["dark" if dark else "light"]
    assert {role: w.theme.role(role) for role in scheme} == {
        role: rgba(hex_) for role, hex_ in scheme.items()
    }


def test_the_type_scale_is_what_the_engine_resolves() -> None:
    w = tre.Window(10, 10, "type")
    w.set_theme((0x67, 0x50, 0xA4, 0xFF))
    for role, style in DATA["typography"].items():
        resolved = w.theme.typography(role)
        assert resolved is not None, role
        family, weight, size, line_height = resolved
        assert (family, weight, size) == (
            style["font_family"],
            style["font_weight"],
            style["font_size"],
        )
        assert line_height == pytest.approx(style["line_height"])


def test_the_default_theme_is_what_the_engine_resolves() -> None:
    w = tre.Window(10, 10, "components")
    w.set_theme((0x67, 0x50, 0xA4, 0xFF))
    for key, fields in DATA["default_theme"]["components"].items():
        component, _, variant = key.partition(".")
        if "corner_radius" in fields:
            assert w.theme.shape(component, variant or None) == fields["corner_radius"], key
        if "elevation" in fields:
            assert w.theme.elevation(component, variant or None) == fields["elevation"], key


def test_every_icon_and_shape_is_path_data() -> None:
    w = tre.Window(10, 10, "paths")
    icons = DATA["icons"]
    for name, data in icons["paths"].items():
        node = w.create("path", data=data, view_box=tuple(icons["view_box"]))
        assert node.get("data").startswith("M"), name  # parsed and normalized
    loading = DATA["loading_indicator"]
    for data in [*loading["shapes"].values(), *loading["intended"].values()]:
        w.create("path", data=data, view_box=tuple(loading["view_box"]))
