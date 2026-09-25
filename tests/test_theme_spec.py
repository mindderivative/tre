"""M86: themes and stylesheets handed to `tre` as data, not file paths.

`default_theme_spec=`/`custom_theme_spec=` (on `View(...)`,
`View.set_theme`, `Window.set_theme`) and `stylesheet_spec=` (on
`View(...)`) take a plain dict in the same schema the equivalent YAML
file holds. A framework that loads its own files -- Tesserae -- can then
hand `tre` data only, never a path. Each spec kwarg is mutually
exclusive with its path twin.

Parity is the core contract: a dict must resolve exactly like the same
content read from a YAML file, so most tests here compare the two forms
directly rather than asserting hardcoded values.
"""

import pytest

from tre import View, Window

SEED = (0x67, 0x50, 0xA4, 0xFF)

CHECKBOX_SPEC = {
    "id": "root",
    "kind": "Checkbox",
    "style": {"width": 20, "height": 20, "background": "#112233"},
}


def checkbox_rule(radius):
    return {"styles": [{"kind": "Checkbox", "style": {"corner_radius": radius}}]}


def write_yaml(tmp_path, name, content):
    path = tmp_path / name
    path.write_text(content)
    return str(path)


# --- View(...): a view built entirely from data ---------------------------


def test_all_four_cascade_tiers_resolve_from_data_alone_with_no_files():
    # default theme (42) < custom theme (16) < stylesheet (24) -- each
    # tier supplied as a dict, the view itself via spec=. Nothing on disk.
    view = View(
        spec=CHECKBOX_SPEC,
        default_theme_spec=checkbox_rule(42),
        custom_theme_spec=checkbox_rule(16),
        stylesheet_spec=checkbox_rule(24),
    )
    assert view.node("root").get("corner_radius") == pytest.approx(24.0)


def test_default_theme_spec_replaces_the_shipped_default():
    view = View(spec=CHECKBOX_SPEC, default_theme_spec=checkbox_rule(42))
    assert view.node("root").get("corner_radius") == pytest.approx(42.0)


def test_custom_theme_spec_supersedes_the_default_theme():
    view = View(spec=CHECKBOX_SPEC, custom_theme_spec=checkbox_rule(16))
    assert view.node("root").get("corner_radius") == pytest.approx(16.0)


def test_stylesheet_spec_matches_the_equivalent_stylesheet_file(tmp_path):
    sheet_path = write_yaml(
        tmp_path, "sheet.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 24}\n"
    )
    from_file = View(spec=CHECKBOX_SPEC, stylesheet=sheet_path)
    from_dict = View(spec=CHECKBOX_SPEC, stylesheet_spec=checkbox_rule(24))
    assert from_dict.node("root").get("corner_radius") == pytest.approx(
        from_file.node("root").get("corner_radius")
    )


def test_custom_theme_spec_color_override_reaches_a_declarative_token():
    # `background: primary` resolves against the active scheme; a bad
    # override would raise here, the same contract the file form has.
    View(
        spec={
            "id": "root",
            "kind": "Rect",
            "style": {"width": 20, "height": 20, "background": "primary"},
        },
        theme_seed=SEED,
        custom_theme_spec={"colors": {"primary": "#FF0000"}},
    )


def test_custom_theme_spec_with_an_unknown_color_role_raises_naming_the_role():
    with pytest.raises(ValueError, match="not_a_real_role"):
        View(
            spec=CHECKBOX_SPEC,
            theme_seed=SEED,
            custom_theme_spec={"colors": {"not_a_real_role": "#FF0000"}},
        )


@pytest.mark.parametrize(
    ("path_kwarg", "spec_kwarg"),
    [
        ("stylesheet", "stylesheet_spec"),
        ("default_theme", "default_theme_spec"),
        ("custom_theme", "custom_theme_spec"),
    ],
)
def test_view_rejects_a_path_and_its_spec_twin_together(tmp_path, path_kwarg, spec_kwarg):
    path = write_yaml(tmp_path, "unused.yaml", "styles: []\n")
    with pytest.raises(ValueError, match=f"{spec_kwarg}="):
        View(spec=CHECKBOX_SPEC, **{path_kwarg: path, spec_kwarg: {"styles": []}})


def test_a_typo_in_a_theme_spec_key_is_rejected_naming_the_kwarg():
    with pytest.raises(ValueError, match="custom_theme_spec="):
        View(spec=CHECKBOX_SPEC, custom_theme_spec={"colours": {}})


def test_a_typo_in_a_stylesheet_spec_key_is_rejected_naming_the_kwarg():
    with pytest.raises(ValueError, match="stylesheet_spec="):
        View(spec=CHECKBOX_SPEC, stylesheet_spec={"style": []})


# --- View.set_theme: live re-theme from data -------------------------------


def test_view_set_theme_accepts_a_custom_theme_spec_live():
    view = View(spec=CHECKBOX_SPEC)
    assert view.node("root").get("corner_radius") == pytest.approx(2.0)
    view.set_theme(custom_theme_spec=checkbox_rule(16))
    assert view.node("root").get("corner_radius") == pytest.approx(16.0)


def test_view_set_theme_rejects_a_path_and_its_spec_twin_together(tmp_path):
    path = write_yaml(tmp_path, "unused.yaml", "styles: []\n")
    view = View(spec=CHECKBOX_SPEC)
    with pytest.raises(ValueError, match="custom_theme_spec="):
        view.set_theme(custom_theme=path, custom_theme_spec={})


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
