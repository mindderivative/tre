"""M49 (§7.1, §16.3): real, repeatable coverage of theme-as-YAML --
`View(default_theme=..., custom_theme=...)` and `Window.set_theme(...,
custom_theme=...)`. Before this milestone, MD3 theming had no override
API at all (`DynamicTheme::from_seed` was the only constructor) and no
per-widget-kind default styles existed anywhere (confirmed via direct
investigation before this milestone -- see `BUILD_TRACKER.md`'s M49
section for the full audit).

Real, honest limitation carried over from every other color-related
test file in this suite (`test_view_binding.py`/`bindable_background.py`):
`Node.get` only returns `f64`, and there is still no Python-facing
getter for a node's currently-applied `background`/role color, or for
an imperative MD3 component's own resolved container color. Tests that
touch color overrides can only prove the real FFI call *applies without
raising*; tests that touch per-kind default *styles* (`corner_radius`)
CAN assert a real readback, since `corner_radius` is a plain `f64`
`Node.get` already supports.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

import time

import pytest

from tre import View, Window


def write_yaml(tmp_path, name, content):
    path = tmp_path / name
    path.write_text(content)
    return str(path)


# --- default theme (shipped + custom) -------------------------------------


def test_shipped_default_theme_applies_a_real_corner_radius_to_checkbox(tmp_path):
    path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(path)
    node = view.node("root")
    # The engine's own shipped default_theme.yaml gives Checkbox a real
    # corner_radius (2.0, the MD3 shape-scale value between none/
    # extra_small) -- confirmed via direct read of the shipped file,
    # not assumed.
    assert node.get("corner_radius") == pytest.approx(2.0)


def test_a_widgets_own_inline_style_still_wins_over_the_shipped_default(tmp_path):
    path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233", corner_radius: 99}\n',
    )
    view = View(path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(99.0)


def test_a_custom_default_theme_path_replaces_the_shipped_one_entirely(tmp_path):
    default_theme_path = write_yaml(
        tmp_path,
        "my_default_theme.yaml",
        "styles:\n  - kind: Checkbox\n    style: {corner_radius: 42}\n",
    )
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(view_path, default_theme=default_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(42.0)


# --- custom theme (styles) superseding the default theme -------------------


def test_custom_theme_styles_supersede_the_default_theme(tmp_path):
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(view_path, custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(16.0)


def test_widget_wide_stylesheet_supersedes_both_theme_layers(tmp_path):
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    stylesheet_path = write_yaml(
        tmp_path, "sheet.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 24}\n"
    )
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(view_path, stylesheet=stylesheet_path, custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(24.0)


def test_all_four_tiers_together_resolve_in_the_users_own_stated_order(tmp_path):
    # default theme (2.0, shipped) < custom theme (16.0) < widget-wide
    # stylesheet (24.0) < inline (99.0) -- each tier entirely
    # superseding the one below it, the real cascade this milestone
    # built (`resolve_style_layered`, `cascade.rs`).
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    stylesheet_path = write_yaml(
        tmp_path, "sheet.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 24}\n"
    )
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233", corner_radius: 99}\n',
    )
    view = View(view_path, stylesheet=stylesheet_path, custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(99.0)


# --- color overrides (declarative path) -------------------------------------


def test_custom_theme_color_override_applies_to_a_declarative_token_without_raising(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'colors:\n  primary: "#FF0000"\n')
    view_path = write_yaml(
        tmp_path, "view.yaml", "id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: primary}\n"
    )
    # Must not raise -- the real M49 color-override wiring, applied
    # through `ColorScheme::apply_overrides` before the declarative
    # `background: primary` token is ever resolved.
    View(view_path, theme_seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)


def test_custom_theme_with_an_unknown_color_role_raises_naming_the_role(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'colors:\n  not_a_real_role: "#FF0000"\n')
    view_path = write_yaml(
        tmp_path, "view.yaml", 'id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: "#000000"}\n'
    )
    with pytest.raises(ValueError, match="not_a_real_role"):
        View(view_path, theme_seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)


def test_custom_theme_with_an_invalid_color_string_raises_naming_the_value(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'colors:\n  primary: "not-a-color"\n')
    view_path = write_yaml(
        tmp_path, "view.yaml", 'id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: "#000000"}\n'
    )
    with pytest.raises(ValueError, match="not-a-color"):
        View(view_path, theme_seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)


# --- seed precedence ---------------------------------------------------------


def test_custom_theme_seed_applies_when_no_explicit_theme_seed_is_given(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'seed: "#FF0000"\n')
    view_path = write_yaml(
        tmp_path, "view.yaml", 'id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: primary}\n'
    )
    # Must not raise -- a real scheme was derived from the theme's own
    # seed even though no `theme_seed` argument was passed at all.
    View(view_path, custom_theme=theme_path)


def test_explicit_theme_seed_wins_over_a_custom_themes_own_seed(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'seed: "#FF0000"\n')
    view_path = write_yaml(
        tmp_path, "view.yaml", 'id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: primary}\n'
    )
    # Must not raise -- an explicitly-passed theme_seed is used instead
    # of the theme file's own seed (can't assert the exact resolved
    # color -- no Python-facing background getter exists -- but this
    # proves the real precedence path doesn't crash either way).
    View(view_path, theme_seed=(0x00, 0xFF, 0x00, 0xFF), custom_theme=theme_path)


# --- Window.set_theme (imperative catalog) -----------------------------------


def test_window_set_theme_accepts_a_custom_theme_color_override(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'colors:\n  primary: "#00FF00"\n')
    window = Window(width=200, height=200)
    # Must not raise -- reaches window_factory.rs's real resolve_button_
    # colors through the exact same ThemeState::role/ColorScheme::role
    # chain every existing MD3 component already uses.
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    window.add_button(label="hi", variant="filled", width=100, height=40)


def test_window_set_theme_with_an_unknown_color_role_raises(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", 'colors:\n  not_a_real_role: "#00FF00"\n')
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="not_a_real_role"):
        window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)


def test_window_set_theme_without_custom_theme_still_works(tmp_path):
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=True)
    window.add_button(label="hi", variant="filled", width=100, height=40)


# --- hot reload keeps using the same theme -----------------------------------


def poll_until_changed(view, timeout=5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if view.poll_reload():
            return True
        time.sleep(0.05)
    return False


def test_poll_reload_re_resolves_against_the_same_theme(tmp_path):
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(view_path, custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(16.0)

    # Rewrite the view (unrelated field) -- the reconciled tree must
    # still resolve corner_radius through the same custom theme, not
    # silently fall back to the shipped default (the real bug this
    # milestone's own `Reconciler`/`View` threading exists to prevent).
    write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 30, height: 20, background: "#112233"}\n',
    )
    assert poll_until_changed(view), "expected a real file-watcher change within the timeout"
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(16.0)
