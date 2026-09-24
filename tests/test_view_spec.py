"""tre issue #3, Part A (Tier 1): `View(spec=...)` builds a real tree
directly from a Python dict, with no YAML text and no backing file at
all -- `engine-spec`'s own tests already prove `Reconciler::load_spec`
in isolation; this file proves the same real capability reaches
`View`, the actual entry point a Python app (or Tesserae's own
macro-expansion layer) uses.
"""

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
