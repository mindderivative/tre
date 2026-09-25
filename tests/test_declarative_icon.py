"""Real, repeatable coverage for `kind: Icon` -- the declarative
counterpart to `Window.add_icon`, added to close a real gap: `engine-
spec`'s `NodeKindSpec` supported only `Rect`/`Container`/`Text`/
`Checkbox`/`Slider`/`TextField`/`Image` before this, with no way to
express an icon glyph declaratively at all (confirmed directly: a
`kind: Icon` view used to fail with `unknown variant "Icon"`). Found
while scoping the sibling `Tesserae` project's own declarative
component-fragment catalog -- most real MD3 compositions (buttons with
icons, chips, list items, app bars) need one.

The glyph's own color reuses `style.background`, the identical
"background means paint color, not a literal fill" precedent `kind:
Text` already established -- not a new, separate color field.
"""

import pytest

from tre import View


def write(tmp_path, yaml, name="view.yaml"):
    path = tmp_path / name
    path.write_text(yaml)
    return str(path)


def test_kind_icon_builds_a_real_node(tmp_path):
    yaml = """
id: gear
kind: Icon
icon: {name: settings}
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"""
    path = write(tmp_path, yaml)
    view = View(path)
    node = view.node("gear")
    assert node is not None


def test_kind_icon_with_no_icon_block_raises_clearly(tmp_path):
    yaml = """
id: gear
kind: Icon
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"""
    path = write(tmp_path, yaml)
    with pytest.raises(ValueError, match="icon"):
        View(path)


def test_kind_icon_with_an_unknown_name_raises_naming_it(tmp_path):
    yaml = """
id: gear
kind: Icon
icon: {name: not_a_real_icon}
style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
"""
    path = write(tmp_path, yaml)
    with pytest.raises(ValueError, match="not_a_real_icon"):
        View(path)


def test_kind_icon_composes_inside_a_container_like_any_other_kind(tmp_path):
    yaml = """
id: root
kind: Container
style: {flex_direction: horizontal, width: 100, height: 40, gap: 8}
children:
  - id: gear
    kind: Icon
    icon: {name: settings}
    style: {width: 24, height: 24, foreground: "#1C1B1FFF"}
  - id: label
    kind: Text
    text: {content: Settings, font_family: Roboto, font_size: 14}
    style: {width: 60, height: 24, foreground: "#1C1B1FFF"}
"""
    path = write(tmp_path, yaml)
    view = View(path)
    assert view.node("gear") is not None
    assert view.node("label") is not None
