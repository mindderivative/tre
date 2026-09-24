"""tre issue #3, Part A (Tier 1) and Part C: `View(spec=...)` builds a
real tree directly from a Python dict, with no YAML text and no
backing file at all -- `engine-spec`'s own tests already prove
`Reconciler::load_spec`/`reconcile_spec` in isolation; this file
proves the same real capability reaches `View`, the actual entry point
a Python app (or Tesserae's own macro-expansion layer) uses. `View.
reconcile(...)` (Part C) is the ungated sibling of `poll_reload` for a
caller with no backing file to watch.

0.3.1 review, user-requested follow-up (item 2): `json=` is the real,
first consumer of `engine_spec::parse_view_json` (tre issue #3, Part A
Tier 2) -- previously shipped with zero callers anywhere in the
workspace. `json=`/`spec=` both hand off to the identical `Reconciler
::load_spec` path (parallel content sources, no real backing file
implied by either), while `source=` alone still requires `path=` for
its own different reason (pre-processed *real file* content still
wanting real hot-reload) -- see `View.__init__`'s own docstring.
"""

import json as jsonlib

import pytest
from tre import View


def test_a_purely_programmatic_view_builds_with_no_path_or_source():
    spec = {
        "id": "root",
        "kind": "Container",
        "style": {"flex_direction": "Horizontal", "width": 200, "height": 100},
        "children": [
            {
                "id": "swatch",
                "kind": "Rect",
                "style": {
                    "width": 40,
                    "height": 40,
                    "background": "#6750A4",
                    "corner_radius": 8,
                },
            },
        ],
    }
    view = View(spec=spec)
    node = view.node("swatch")
    assert node is not None
    assert node.get("corner_radius") == 8.0


def test_spec_and_source_together_is_a_clear_error():
    with pytest.raises(ValueError, match="spec=.*source="):
        View(spec={"id": "root", "kind": "Container"}, source="id: root\nkind: Container\n")


def test_neither_spec_nor_path_is_a_clear_error():
    with pytest.raises(ValueError, match="path=.*spec="):
        View()


def test_a_real_theme_seed_still_resolves_md3_tokens_through_spec():
    spec = {
        "id": "root",
        "kind": "Rect",
        "style": {"width": 40, "height": 40, "background": "primary", "corner_radius": "medium"},
    }
    view = View(spec=spec, theme_seed=(0x67, 0x50, 0xA4, 0xFF))
    node = view.node("root")
    assert node.get("corner_radius") == 12.0


def test_path_given_alongside_spec_is_used_only_as_a_base_dir_hint(tmp_path):
    # A real, existing directory (but no view.yaml inside it at all --
    # spec= supplies the content, path= only orients base_dir/include
    # resolution, matching what the constructor's own docstring states).
    spec = {"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}}
    view = View(path=str(tmp_path / "virtual.yaml"), spec=spec)
    assert view.node("root") is not None


def test_reconcile_spec_updates_a_purely_programmatic_view_with_no_watcher():
    # tre issue #3, Part C: a spec=-only View has no watcher at all, so
    # poll_reload() can never open its own file-change gate -- reconcile()
    # is the real, ungated sibling for exactly this caller.
    spec = {
        "id": "root",
        "kind": "Rect",
        "style": {"width": 40, "height": 40, "background": "#112233", "corner_radius": 4},
    }
    view = View(spec=spec)
    assert view.poll_reload() is False

    new_spec = {
        "id": "root",
        "kind": "Rect",
        "style": {"width": 40, "height": 40, "background": "#112233", "corner_radius": 20},
    }
    view.reconcile(spec=new_spec)
    assert view.node("root").get("corner_radius") == 20.0


def test_reconcile_source_form_also_works():
    view = View(spec={"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}})
    view.reconcile(source="id: root\nkind: Container\nstyle: {width: 10, height: 10}\n")
    assert view.node("root") is not None


def test_reconcile_spec_and_source_together_is_a_clear_error():
    view = View(spec={"id": "root", "kind": "Container"})
    with pytest.raises(ValueError, match="spec=.*source="):
        view.reconcile(spec={"id": "root", "kind": "Container"}, source="id: root\nkind: Container\n")


def test_reconcile_with_neither_spec_nor_source_is_a_clear_error():
    view = View(spec={"id": "root", "kind": "Container"})
    with pytest.raises(ValueError, match="source=.*spec="):
        view.reconcile()


def test_json_builds_a_real_view_with_no_path_needed():
    # json= is self-sufficient like spec=, not like source= -- no real
    # backing file is implied, so no path= is required alongside it.
    json_text = jsonlib.dumps(
        {
            "id": "root",
            "kind": "Rect",
            "style": {"width": 40, "height": 40, "background": "#112233", "corner_radius": 4},
        }
    )
    view = View(json=json_text)
    assert view.node("root").get("corner_radius") == 4.0


def test_reconcile_json_form_also_works():
    view = View(json=jsonlib.dumps({"id": "root", "kind": "Container", "style": {"width": 10, "height": 10}}))
    new_json = jsonlib.dumps(
        {
            "id": "root",
            "kind": "Rect",
            "style": {"width": 10, "height": 10, "background": "#334455", "corner_radius": 6},
        }
    )
    view.reconcile(json=new_json)
    assert view.node("root").get("corner_radius") == 6.0


def test_spec_and_json_together_is_a_clear_error():
    # The real, shared 3-way validation (spec=/source=/json=) --
    # confirms it's not just a pairwise check that missed this
    # combination.
    with pytest.raises(ValueError, match="spec=, source=, json="):
        View(spec={"id": "root", "kind": "Container"}, json="{}")


def test_source_and_json_together_is_a_clear_error(tmp_path):
    with pytest.raises(ValueError, match="spec=, source=, json="):
        View(path=str(tmp_path / "v.yaml"), source="id: root\nkind: Container\n", json="{}")


def test_reconcile_json_and_source_together_is_a_clear_error():
    view = View(spec={"id": "root", "kind": "Container"})
    with pytest.raises(ValueError, match="spec=, source=, json="):
        view.reconcile(source="id: root\nkind: Container\n", json="{}")
