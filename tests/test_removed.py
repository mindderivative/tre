"""M97 Phase 2 Step 5: the migration gate's switch, `TRE_FORBID_REMOVED=1`
(`tre/_removed.py`) -- its tables kept in step with the migration table in
`docs/design/target-api.md`, and what it forbids and leaves alone. The switch
patches classes for the rest of a process, so it's exercised in a fresh one.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

import tre
from tre import _core, _removed

SPEC = Path(__file__).resolve().parent.parent / "docs" / "design" / "target-api.md"
TABLE = SPEC.read_text().split("## Migration table", 1)[1]


def table_rows() -> list[tuple[list[str], str, str]]:
    """The migration table's rows: the backticked names in "Today" (with
    any argument list dropped), and the "Today" and "Target" text."""
    rows = []
    for today, target in re.findall(r"^\| (.+?) \| (.+?) \|$", TABLE, re.M):
        if today in ("Today", "---"):
            continue
        names = [re.sub(r"\(.*", "", n) for n in re.findall(r"`([^`]+)`", today)]
        rows.append((names, today, target))
    return rows


def test_every_migration_table_name_is_forbidden_kept_or_explained() -> None:
    handled = {name for members in _removed.REMOVED.values() for name in members}
    handled |= {f"Event.{name}" for name in _removed.REMOVED["Event"]}
    handled |= set(_removed.PROPERTIES) | set(_removed.UNENFORCED)
    for names, today, target in table_rows():
        if target.startswith("unchanged"):
            continue
        if not names:  # a row naming no Python name at all
            assert today in _removed.UNENFORCED, today
            continue
        # A removed class's members, listed after it, go with it.
        if names[0] in _removed.REMOVED["tre"]:
            names = [n for n in names if n in _removed.REMOVED["tre"]]
        for name in names:
            assert name in handled, f"{name!r} is in the migration table but not in _removed.py"


def test_every_forbidden_name_is_in_the_migration_table() -> None:
    table = {name for names, _, _ in table_rows() for name in names}
    for owner, members in _removed.REMOVED.items():
        for name in members:
            key = f"{owner}.{name}" if owner == "Event" else name
            assert key in table, f"{owner}.{name} is forbidden but not in the migration table"
    for name in [*_removed.PROPERTIES, *_removed.UNENFORCED]:
        assert name in table or name in TABLE, name


def test_every_forbidden_name_exists_today() -> None:
    """A typo'd table entry would forbid nothing."""
    classes = {"tre": tre, "Window": _core.Window, "Node": _core.Node, "Event": _core.Event}
    for owner, members in _removed.REMOVED.items():
        for name in members:
            assert hasattr(classes[owner], name), f"{owner}.{name} doesn't exist"


def test_off_by_default() -> None:
    assert os.environ.get("TRE_FORBID_REMOVED") != "1", "run the suite without the switch"
    assert tre.Signal is not None
    assert callable(tre.Window(10, 10, "off").add_rect)


SCRIPT = r"""
import json
import tre

w = tre.Window(200, 100, "switch")
results = {}

def probe(label, fn):
    try:
        fn()
        results[label] = "allowed"
    except AttributeError as e:
        results[label] = str(e)
    except ImportError:
        results[label] = "ImportError"

box = w.create("box", width=50, height=20)
w.root.add_child(box)
probe("tre.Signal", lambda: tre.Signal)
probe("from tre import View", lambda: exec("from tre import View"))
probe("Window.add_button", lambda: w.add_button)
probe("Window.theme", lambda: w.theme)
probe("Node.set_on_click", lambda: box.set_on_click)
probe("animate background", lambda: box.animate("background", (0, 0, 0, 255), 0))
probe("get elevation", lambda: box.get("elevation"))
probe("positional on_complete", lambda: box.animate("opacity", 0.5, 100, lambda: None))
seen = []
box.on("click", lambda e: seen.append(e))
w.simulate("click", node=box)
probe("Event.kind", lambda: seen[0].kind)

# What stays works.
box.animate("opacity", 0.5, 100, on_complete=lambda: None)
box.set(fill=(1, 2, 3, 255), corner_radius=4)
w.advance(100)
results["kept"] = [box.get("opacity"), list(box.get("fill")), seen[0].type, box.get("value")]
results["hasattr add_button"] = hasattr(tre.Window, "add_button")
results["__all__"] = tre.__all__
exec("from tre import *")
print(json.dumps(results))
"""


def test_the_switch_forbids_removed_names_and_keeps_the_rest() -> None:
    env = {**os.environ, "TRE_FORBID_REMOVED": "1"}
    out = subprocess.run(
        [sys.executable, "-c", SCRIPT], env=env, capture_output=True, text=True, check=True
    )
    results = json.loads(out.stdout)
    removed = "is removed in tre 0.3.5 -- use"
    assert results["tre.Signal"] == f"Signal {removed} Tesserae's reactivity (D5)"
    assert results["from tre import View"] == "ImportError"
    assert results["Window.add_button"] == f"Window.add_button {removed} the framework's own widget"
    assert results["Window.theme"].startswith(f"Window.theme {removed}")
    assert results["Node.set_on_click"] == f'Node.set_on_click {removed} node.on("click", ...)'
    assert results["animate background"] == f"Node.property 'background' {removed} \"fill\""
    assert results["get elevation"] == f"Node.property 'elevation' {removed} \"shadows\""
    assert "pass on_complete by keyword" in results["positional on_complete"]
    assert results["Event.kind"] == f"Event.kind {removed} event.type"
    assert results["kept"] == [0.5, [1, 2, 3, 255], "click", None]
    assert results["hasattr add_button"] is False
    assert results["__all__"] == [
        "App", "Event", "LoopHandle", "MONOSPACE_FONT_FAMILY", "Node", "Window", "register_font",
    ]  # fmt: skip
