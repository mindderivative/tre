"""M86: themes and stylesheets handed to `tre` as data, not file paths.

`Window.set_theme`'s `default_theme_spec=`/`custom_theme_spec=` take a
plain dict in the same schema the equivalent YAML file holds (M98 removed
`View`'s own spec arguments, and `stylesheet_spec=`, with `View`). A framework that loads its own files -- Tesserae -- can then
hand `tre` data only, never a path. Each spec kwarg is mutually
exclusive with its path twin.

Parity is the core contract: a dict must resolve exactly like the same
content read from a YAML file, so most tests here compare the two forms
directly rather than asserting hardcoded values.
"""

import pytest

from tre import Window

SEED = (0x67, 0x50, 0xA4, 0xFF)

CHECKBOX_SPEC = {
    "id": "root",
    "kind": "Checkbox",
    "style": {"width": 20, "height": 20, "background": "#112233"},
}


def write_yaml(tmp_path, name, content):
    path = tmp_path / name
    path.write_text(content)
    return str(path)


# --- View(...): a view built entirely from data ---------------------------


# --- View.set_theme: live re-theme from data -------------------------------


# --- Window.set_theme: the imperative catalog's theme from data -----------

WINDOW_THEME_YAML = """\
colors:
  primary: "#00FF00"
components:
  button: {corner_radius: small}
  card: {elevation: level_2}
typography:
  body_large: {font_family: Inter, font_size: 18}
"""

WINDOW_THEME_DICT = {
    "colors": {"primary": "#00FF00"},
    "components": {
        "button": {"corner_radius": "small"},
        "card": {"elevation": "level_2"},
    },
    "typography": {"body_large": {"font_family": "Inter", "font_size": 18}},
}


def test_window_custom_theme_spec_resolves_identically_to_the_yaml_file(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", WINDOW_THEME_YAML)
    from_file = Window()
    from_file.set_theme(seed=SEED, custom_theme=theme_path)
    from_dict = Window()
    from_dict.set_theme(seed=SEED, custom_theme_spec=WINDOW_THEME_DICT)

    file_theme, dict_theme = from_file.theme, from_dict.theme
    for role in ("primary", "on_primary", "surface", "on_surface"):
        assert dict_theme.role(role) == file_theme.role(role)
    assert dict_theme.shape("button") == file_theme.shape("button")
    assert dict_theme.elevation("card") == file_theme.elevation("card")
    assert dict_theme.typography("body_large") == file_theme.typography("body_large")


def test_window_custom_theme_spec_values_actually_apply():
    window = Window()
    window.set_theme(seed=SEED, custom_theme_spec=WINDOW_THEME_DICT)
    theme = window.theme
    assert theme.role("primary") == (0x00, 0xFF, 0x00, 0xFF)
    assert theme.shape("button") is not None
    family, _weight, size, _line_height = theme.typography("body_large")
    assert family == "Inter"
    assert size == pytest.approx(18.0)


def test_window_default_theme_spec_supplies_the_baseline_components():
    window = Window()
    window.set_theme(
        seed=SEED,
        default_theme_spec={"components": {"card": {"elevation": "level_3"}}},
        custom_theme_spec={"components": {"button": {"corner_radius": "small"}}},
    )
    # default supplies card, custom supplies button -- both present.
    assert window.theme.elevation("card") is not None
    assert window.theme.shape("button") is not None


def test_window_rejects_a_path_and_its_spec_twin_together(tmp_path):
    path = write_yaml(tmp_path, "unused.yaml", "colors: {}\n")
    window = Window()
    with pytest.raises(ValueError, match="default_theme_spec="):
        window.set_theme(seed=SEED, default_theme=path, default_theme_spec={})


def test_a_failed_window_set_theme_with_a_bad_spec_leaves_the_theme_unset():
    window = Window()
    with pytest.raises(ValueError):
        window.set_theme(seed=SEED, custom_theme_spec={"colors": {"not_a_real_role": "#FF0000"}})
    assert window.theme.is_set() is False
