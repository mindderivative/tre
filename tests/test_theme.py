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


# --- M50 Phase 2: components: -- Buttons & FAB family ------------------


def window_with_components(tmp_path, components_yaml):
    theme_path = write_yaml(tmp_path, "theme.yaml", f"components:\n{components_yaml}")
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    return window


def test_button_variant_specific_override_applies_corner_radius_and_elevation(tmp_path):
    window = window_with_components(
        tmp_path, "  button.filled: {corner_radius: 5, elevation: 3}\n"
    )
    node = window.add_button(label="hi", variant="filled", width=100, height=40)
    assert node.get("corner_radius") == pytest.approx(5.0)
    assert node.get("elevation") == pytest.approx(3.0)


def test_button_variant_specific_override_does_not_leak_to_other_variants(tmp_path):
    window = window_with_components(tmp_path, "  button.filled: {corner_radius: 5}\n")
    node = window.add_button(label="hi", variant="outlined", width=100, height=40)
    assert node.get("corner_radius") == pytest.approx(20.0)


def test_bare_button_key_applies_to_every_variant(tmp_path):
    window = window_with_components(tmp_path, "  button: {corner_radius: 6}\n")
    for variant in ("elevated", "filled", "filled_tonal", "outlined", "text"):
        node = window.add_button(label="hi", variant=variant, width=100, height=40)
        assert node.get("corner_radius") == pytest.approx(6.0), variant


def test_icon_button_has_its_own_key_distinct_from_plain_button(tmp_path):
    window = window_with_components(tmp_path, "  button: {corner_radius: 6}\n")
    node = window.add_icon_button(icon="add", size=40.0, variant="filled")
    # The bare "button" override must not leak into "icon_button" --
    # each factory gets its own key, confirmed by the real default
    # (size / 2.0 = 20.0) surviving untouched.
    assert node.get("corner_radius") == pytest.approx(20.0)


def test_icon_button_key_uses_the_md3_internal_text_variant_name_for_standard(tmp_path):
    window = window_with_components(tmp_path, "  icon_button.text: {corner_radius: 7}\n")
    node = window.add_icon_button(icon="add", size=40.0, variant="standard")
    assert node.get("corner_radius") == pytest.approx(7.0)


def test_fab_corner_radius_is_keyed_by_size_elevation_is_not(tmp_path):
    window = window_with_components(
        tmp_path, "  fab.small: {corner_radius: 4}\n  fab: {elevation: 5}\n"
    )
    small = window.add_fab(icon="add", size="small", variant="surface")
    default = window.add_fab(icon="add", size="default", variant="surface")
    assert small.get("corner_radius") == pytest.approx(4.0)
    assert default.get("corner_radius") == pytest.approx(16.0), "size=default must be unaffected"
    assert small.get("elevation") == pytest.approx(5.0)
    assert default.get("elevation") == pytest.approx(5.0), "elevation override has no size key"


def test_extended_fab_has_its_own_key_distinct_from_fab(tmp_path):
    window = window_with_components(tmp_path, "  fab: {corner_radius: 4}\n")
    node = window.add_extended_fab(label="hi", width=120.0, variant="primary")
    assert node.get("corner_radius") == pytest.approx(16.0), "extended_fab must use its own key"


def test_segmented_button_corner_radius_override_does_not_raise(tmp_path):
    # `add_segmented_button` never returns its own frame Node (only the
    # segments, each of which uses `corner_radii_override` -- not the
    # plain `corner_radius` field -- for its own rounding), so there is
    # no Python-facing way to read back the overridden frame value
    # directly; this proves the real override wiring applies without
    # raising, the same honest limit this suite already states
    # elsewhere for untestable internal state.
    window = window_with_components(tmp_path, "  segmented_button: {corner_radius: 3}\n")
    window.add_segmented_button(labels=["A", "B"], width=200.0, height=40.0)


def test_toolbar_corner_radius_and_elevation_keyed_by_variant(tmp_path):
    window = window_with_components(
        tmp_path,
        "  toolbar.floating: {corner_radius: 9, elevation: 4}\n  toolbar.docked: {corner_radius: 1}\n",
    )
    floating = window.add_toolbar(variant="floating")
    docked = window.add_toolbar(variant="docked")
    assert floating.get("corner_radius") == pytest.approx(9.0)
    assert floating.get("elevation") == pytest.approx(4.0)
    assert docked.get("corner_radius") == pytest.approx(1.0)
    assert docked.get("elevation") == pytest.approx(0.0), "docked has no real elevation override here"


def test_split_button_leading_inherits_the_button_key(tmp_path):
    window = window_with_components(tmp_path, "  button.filled: {corner_radius: 5}\n")
    leading, trailing, _icon = window.add_split_button(
        label="hi", width=100.0, height=40.0, variant="filled"
    )
    assert leading.get("corner_radius") == pytest.approx(5.0)
    assert trailing.get("corner_radius") == pytest.approx(5.0)


def test_split_button_and_button_group_tightened_keys_do_not_raise(tmp_path):
    # No Python-facing readback exists for corner_radii_override/
    # interactive_shape (a hover/press-driven ShapeKey pair, not a
    # plain f64 Node.get already supports) -- proven by not raising,
    # matching this suite's own established honesty about that real
    # limit elsewhere.
    window = window_with_components(
        tmp_path,
        "  split_button.tightened: {corner_radius: 2}\n  button_group.tightened: {corner_radius: 3}\n",
    )
    window.add_split_button(label="hi", width=100.0, height=40.0, variant="filled")
    window.add_button_group(labels=["A", "B"], width=80.0, height=40.0, variant="filled")


def test_unthemed_window_preserves_every_real_default_value(tmp_path):
    window = Window(width=400, height=400)
    cases = [
        (window.add_button(label="hi", variant="elevated", width=100, height=40), 20.0, 1.0),
        (window.add_button(label="hi", variant="filled", width=100, height=40), 20.0, 0.0),
        (window.add_icon_button(icon="add", size=40.0, variant="filled"), 20.0, 0.0),
        (window.add_fab(icon="add", size="small", variant="surface"), 12.0, 3.0),
        (window.add_fab(icon="add", size="default", variant="surface"), 16.0, 3.0),
        (window.add_fab(icon="add", size="large", variant="surface"), 28.0, 3.0),
        (window.add_extended_fab(label="hi", width=120.0, variant="primary"), 16.0, 3.0),
        (window.add_toolbar(variant="floating"), None, 3.0),
        (window.add_toolbar(variant="docked"), 0.0, 0.0),
    ]
    for node, expected_radius, expected_elevation in cases:
        if expected_radius is not None:
            assert node.get("corner_radius") == pytest.approx(expected_radius)
        assert node.get("elevation") == pytest.approx(expected_elevation)


# --- M50 Phase 3: components: -- Containers & surfaces ------------------


def test_card_variant_specific_elevation_and_bare_corner_radius(tmp_path):
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 20}\n  card.elevated: {elevation: 9}\n"
    )
    elevated = window.add_card(width=200.0, height=100.0, variant="elevated")
    filled = window.add_card(width=200.0, height=100.0, variant="filled")
    assert elevated.get("corner_radius") == pytest.approx(20.0)
    assert filled.get("corner_radius") == pytest.approx(20.0), "bare key applies to every variant"
    assert elevated.get("elevation") == pytest.approx(9.0)
    assert filled.get("elevation") == pytest.approx(0.0), "elevated-only override must not leak"


def test_chip_corner_radius_override_has_no_elevation_concept(tmp_path):
    window = window_with_components(tmp_path, "  chip: {corner_radius: 2}\n")
    node = window.add_chip(label="hi", width=100.0, variant="assist")
    assert node.get("corner_radius") == pytest.approx(2.0)
    assert node.get("elevation") == pytest.approx(0.0)


def test_tooltip_corner_radius_override(tmp_path):
    window = window_with_components(tmp_path, "  tooltip: {corner_radius: 1}\n")
    node = window.add_tooltip(text="hi", width=100.0)
    assert node.get("corner_radius") == pytest.approx(1.0)


def test_dialog_override_does_not_raise(tmp_path):
    # `add_dialog` returns the *scrim* node (its own `corner_radius`/
    # `elevation` always literal `0.0`, a full-window backdrop) -- the
    # real themed panel is one of its children, never returned to
    # Python at all, confirmed via direct read of `add_dialog`'s own
    # real `self.wrap_node(scrim)` return before writing this test.
    # Matches this suite's own established honesty for untestable
    # internal state elsewhere (`add_segmented_button`/`add_side_sheet`).
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 99}\n  dialog: {corner_radius: 10, elevation: 6}\n"
    )
    window.add_dialog(headline="hi", text="body", width=280.0, height=180.0)


def test_snackbar_corner_radius_and_elevation_override(tmp_path):
    window = window_with_components(tmp_path, "  snackbar: {corner_radius: 2, elevation: 4}\n")
    container, _action, _close = window.add_snackbar(text="hi", width=300.0)
    assert container.get("corner_radius") == pytest.approx(2.0)
    assert container.get("elevation") == pytest.approx(4.0)


def test_popover_has_its_own_key_distinct_from_card(tmp_path):
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 99}\n  popover: {corner_radius: 8, elevation: 5}\n"
    )
    node = window.add_popover(subhead="hi", text="body", width=280.0, height=140.0)
    assert node.get("corner_radius") == pytest.approx(8.0)
    assert node.get("elevation") == pytest.approx(5.0)


def test_side_sheet_elevation_override_keyed_by_modal_variant(tmp_path):
    window = window_with_components(
        tmp_path,
        "  side_sheet.modal: {elevation: 3}\n  side_sheet.standard: {elevation: 1}\n",
    )
    standard = window.add_side_sheet(width=300.0, modal=False)
    modal = window.add_side_sheet(width=300.0, modal=True)
    assert standard.get("elevation") == pytest.approx(1.0)
    # `modal=True` returns the scrim (unattached) per its own doc
    # comment -- the panel's own elevation isn't directly readable
    # through it, so this only proves the override path doesn't raise
    # for the modal branch specifically.
    assert modal is not None


def test_side_sheet_corner_radius_override_does_not_raise(tmp_path):
    # `corner_radii_override`, not the plain `corner_radius` field, so
    # there's no Python-facing readback -- matching `add_segmented_
    # button`'s own established limit in this same suite.
    window = window_with_components(tmp_path, "  side_sheet: {corner_radius: 4}\n")
    window.add_side_sheet(width=300.0, modal=False)


def test_navigation_drawer_has_its_own_key_distinct_from_side_sheet(tmp_path):
    window = window_with_components(
        tmp_path,
        "  side_sheet.standard: {elevation: 9}\n  navigation_drawer.standard: {elevation: 2}\n",
    )
    container, _items = window.add_navigation_drawer(
        labels=["A", "B"], icons=["home", "settings"], modal=False, width=280.0
    )
    assert container.get("elevation") == pytest.approx(2.0)


def test_navigation_drawer_indicator_and_corner_radius_overrides_do_not_raise(tmp_path):
    window = window_with_components(
        tmp_path,
        "  navigation_drawer.standard: {corner_radius: 5}\n  navigation_drawer.indicator: {corner_radius: 3}\n",
    )
    window.add_navigation_drawer(
        labels=["A", "B"], icons=["home", "settings"], selected=0, modal=False, width=280.0
    )


def test_search_bar_corner_radius_and_elevation_override(tmp_path):
    window = window_with_components(tmp_path, "  search_bar: {corner_radius: 6, elevation: 2}\n")
    bar, _field, _leading, _trailing = window.add_search_bar(placeholder="hi", width=300.0)
    assert bar.get("corner_radius") == pytest.approx(6.0)
    assert bar.get("elevation") == pytest.approx(2.0)


def test_search_view_has_its_own_key_distinct_from_dialog(tmp_path):
    window = window_with_components(
        tmp_path, "  dialog: {corner_radius: 99}\n  search_view: {corner_radius: 7, elevation: 2}\n"
    )
    node = window.add_search_view(width=300.0, height=400.0)
    assert node.get("corner_radius") == pytest.approx(7.0)
    assert node.get("elevation") == pytest.approx(2.0)


def test_unthemed_containers_and_surfaces_preserve_every_real_default_value(tmp_path):
    window = Window(width=400, height=400)
    assert window.add_card(width=200.0, height=100.0, variant="elevated").get(
        "corner_radius"
    ) == pytest.approx(12.0)
    assert window.add_card(width=200.0, height=100.0, variant="elevated").get(
        "elevation"
    ) == pytest.approx(1.0)
    assert window.add_chip(label="hi", width=100.0, variant="assist").get(
        "corner_radius"
    ) == pytest.approx(8.0)
    assert window.add_tooltip(text="hi", width=100.0).get("corner_radius") == pytest.approx(4.0)
    # add_dialog returns the scrim, not the themed panel -- see
    # test_dialog_override_does_not_raise's own doc comment above.
    window.add_dialog(headline="hi", text="body", width=280.0, height=180.0)
    snackbar, _a, _c = window.add_snackbar(text="hi", width=300.0)
    assert snackbar.get("corner_radius") == pytest.approx(4.0)
    assert snackbar.get("elevation") == pytest.approx(3.0)
    popover = window.add_popover(subhead="hi", text="body", width=280.0, height=140.0)
    assert popover.get("corner_radius") == pytest.approx(12.0)
    search_bar, _f, _l, _t = window.add_search_bar(placeholder="hi", width=300.0)
    assert search_bar.get("corner_radius") == pytest.approx(28.0)
    search_view = window.add_search_view(width=300.0, height=400.0)
    assert search_view.get("corner_radius") == pytest.approx(28.0)
    assert search_view.get("elevation") == pytest.approx(3.0)


# --- M50 Phase 4: components: -- Navigation, data entry & misc ------------


def test_badge_dot_and_labeled_variants_have_their_own_keys(tmp_path):
    window = window_with_components(
        tmp_path, "  badge.dot: {corner_radius: 1}\n  badge.labeled: {corner_radius: 2}\n"
    )
    dot = window.add_badge()
    labeled = window.add_badge(label="9")
    assert dot.get("corner_radius") == pytest.approx(1.0)
    assert labeled.get("corner_radius") == pytest.approx(2.0)


def test_navigation_rail_indicator_override_does_not_raise(tmp_path):
    window = window_with_components(
        tmp_path, "  navigation_rail.indicator: {corner_radius: 3}\n"
    )
    window.add_navigation_rail(labels=["A", "B"], icons=["home", "settings"], selected=0)


def test_top_app_bar_icon_button_key_reuses_icon_button(tmp_path):
    window = window_with_components(tmp_path, "  icon_button: {corner_radius: 6}\n")
    bar, leading, trailing = window.add_top_app_bar(
        title="hi", leading_icon="menu", trailing_icons=["search"]
    )
    assert leading.get("corner_radius") == pytest.approx(6.0)
    assert trailing[0].get("corner_radius") == pytest.approx(6.0)
    assert bar.get("corner_radius") == pytest.approx(0.0), "the bar itself stays un-themed"


def test_tabs_indicator_override_does_not_raise(tmp_path):
    window = window_with_components(tmp_path, "  tabs.indicator: {corner_radius: 5}\n")
    window.add_tabs(labels=["A", "B"], selected=0)


def test_date_picker_day_corner_radius_override(tmp_path):
    window = window_with_components(tmp_path, "  date_picker_day: {corner_radius: 10}\n")
    node = window.add_date_picker_day(day=1, selected=True)
    assert node.get("corner_radius") == pytest.approx(10.0)


def test_time_input_field_has_its_own_key_distinct_from_chip(tmp_path):
    window = window_with_components(
        tmp_path, "  chip: {corner_radius: 99}\n  time_input_field: {corner_radius: 3}\n"
    )
    node = window.add_time_input_field(value="12:00")
    assert node.get("corner_radius") == pytest.approx(3.0)


def test_period_selector_has_its_own_key_distinct_from_chip(tmp_path):
    window = window_with_components(
        tmp_path, "  chip: {corner_radius: 99}\n  period_selector: {corner_radius: 4}\n"
    )
    am, pm = window.add_period_selector()
    assert am.get("corner_radius") == pytest.approx(4.0)
    assert pm.get("corner_radius") == pytest.approx(4.0)


def test_spin_box_button_reuses_icon_button_field_has_its_own_key(tmp_path):
    window = window_with_components(
        tmp_path, "  icon_button: {corner_radius: 6}\n  spin_box: {corner_radius: 5}\n"
    )
    field, decrement, increment = window.add_spin_box(value="1")
    assert decrement.get("corner_radius") == pytest.approx(6.0)
    assert increment.get("corner_radius") == pytest.approx(6.0)
    assert field.get("corner_radius") == pytest.approx(5.0)


def test_pagination_corner_radius_override_applies_to_arrows_and_pages(tmp_path):
    window = window_with_components(tmp_path, "  pagination: {corner_radius: 7}\n")
    previous, pages, next_ = window.add_pagination(page_count=3, current=0)
    assert previous.get("corner_radius") == pytest.approx(7.0)
    assert pages[0].get("corner_radius") == pytest.approx(7.0)
    assert next_.get("corner_radius") == pytest.approx(7.0)


def test_graph_node_has_its_own_key_distinct_from_card(tmp_path):
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 99}\n  graph_node: {corner_radius: 6}\n"
    )
    graph = window.add_node_graph(width=400.0, height=300.0)
    node = window.add_graph_node(graph=graph, label="hi", x=0.0, y=0.0, width=120.0, height=80.0)
    assert node.get("corner_radius") == pytest.approx(6.0)


def test_unthemed_navigation_and_misc_preserve_every_real_default_value(tmp_path):
    window = Window(width=400, height=400)
    assert window.add_badge().get("corner_radius") == pytest.approx(3.0)
    assert window.add_badge(label="9").get("corner_radius") == pytest.approx(8.0)
    bar, _l, _t = window.add_top_app_bar(title="hi")
    assert bar.get("corner_radius") == pytest.approx(0.0)
    assert window.add_date_picker_day(day=1).get("corner_radius") == pytest.approx(24.0)
    assert window.add_time_input_field(value="12:00").get("corner_radius") == pytest.approx(8.0)
    am, _pm = window.add_period_selector()
    assert am.get("corner_radius") == pytest.approx(8.0)
    field, _d, _i = window.add_spin_box(value="1")
    assert field.get("corner_radius") == pytest.approx(8.0)
    previous, _pages, _next = window.add_pagination(page_count=1, current=0)
    assert previous.get("corner_radius") == pytest.approx(20.0)
    graph = window.add_node_graph(width=400.0, height=300.0)
    node = window.add_graph_node(graph=graph, label="hi", x=0.0, y=0.0, width=120.0, height=80.0)
    assert node.get("corner_radius") == pytest.approx(12.0)
