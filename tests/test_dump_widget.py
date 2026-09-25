"""M97 Phase 2: `tools/dump_widget.py` -- the node-subtree dump the 0.3.5
widget migration is checked against, and the reference it generates.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

import tre

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import dump_widget  # noqa: E402


def test_get_kind_names_what_create_takes() -> None:
    w = tre.Window(100, 100, "kind")
    for kind in ("box", "text_input", "scroll_view"):
        assert w.create(kind).get("kind") == kind
    assert w.create("text", text="x").get("kind") == "text"
    checkbox = w.add_checkbox(background=(0, 0, 0, 255), width=18, height=18)
    assert checkbox.get("kind") == "checkbox"


def test_a_rebuilt_button_dumps_like_the_factory() -> None:
    """The point of the tool: a widget rebuilt from building blocks is
    checked node for node against the factory it replaces."""
    legacy = dump_widget.dump_one("add_button")["trees"]["content"]
    w = tre.Window(800, 600, "rebuilt")
    button = w.create(
        "box", width=120, height=40, align_items="center",
        fill=(103, 80, 164, 255), corner_radius=20,
    )  # fmt: skip
    button.add_child(w.create(
        "text", text="Button", width=72, height=20, fill=(255, 255, 255, 255),
        font_weight=500, font_size=14, text_align="center",
    ))  # fmt: skip
    w.root.add_child(button)
    assert [dump_widget.dump(button)] == legacy


def test_a_difference_shows_in_the_dump() -> None:
    w = tre.Window(100, 100, "diff")
    plain, rounded = w.create("box"), w.create("box", corner_radius=4)
    assert "corner_radius" not in dump_widget.dump(plain)["props"]
    assert dump_widget.dump(rounded)["props"]["corner_radius"] == 4.0
    assert "corner_radius" in dump_widget.dump(plain, full=True)["props"]


def test_a_layer_is_dumped_beside_the_content() -> None:
    entry = dump_widget.dump_one("add_dialog")
    assert entry["returns"] == ["layer0:"]
    assert entry["trees"]["content"] == []
    scrim = entry["trees"]["layer0"][0]
    assert scrim["box"] == [0.0, 0.0, 800.0, 600.0]
    assert scrim["children"][0]["children"][0]["props"]["text"] == "Title"


@pytest.mark.skipif(sys.platform != "linux", reason="add_terminal runs /bin/sh")
def test_the_reference_is_current() -> None:
    text, dumps = dump_widget.reference()
    assert dump_widget.REFERENCE_MD.read_text() == text, (
        "run `python tools/dump_widget.py --reference`"
    )
    assert json.loads(dump_widget.REFERENCE_JSON.read_text()) == dumps
    assert len(dumps) == len(dump_widget.FACTORIES) == 59
