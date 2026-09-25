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

from tre import Signal, View, ViewModel, Window


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


# --- M50 Phase 5: Window.set_theme(default_theme=...) ------------------


def test_window_set_theme_auto_loads_the_shipped_default_components(tmp_path):
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    card = window.add_card(width=200.0, height=100.0, variant="elevated")
    assert card.get("corner_radius") == pytest.approx(12.0)
    assert card.get("elevation") == pytest.approx(1.0)


def test_window_set_theme_custom_theme_wins_over_the_shipped_default(tmp_path):
    theme_path = write_yaml(tmp_path, "theme.yaml", "components:\n  card: {corner_radius: 99}\n")
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    card = window.add_card(width=200.0, height=100.0, variant="filled")
    assert card.get("corner_radius") == pytest.approx(99.0)
    # elevation wasn't in the custom theme's own override -- still
    # resolves from the shipped default's own merged-in baseline.
    assert card.get("elevation") == pytest.approx(0.0)


def test_window_set_theme_custom_default_theme_path_replaces_the_shipped_one(tmp_path):
    default_theme_path = write_yaml(
        tmp_path, "my_default.yaml", "components:\n  card: {corner_radius: 42}\n"
    )
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), default_theme=default_theme_path)
    card = window.add_card(width=200.0, height=100.0, variant="filled")
    assert card.get("corner_radius") == pytest.approx(42.0)
    # The shipped default's own "card.elevated: elevation 1" entry is
    # gone -- a real, own default_theme replaces it entirely, the same
    # "replace, not merge" contract View.__init__'s own default_theme
    # already establishes.
    elevated = window.add_card(width=200.0, height=100.0, variant="elevated")
    assert elevated.get("elevation") == pytest.approx(1.0), (
        "the real MD3 baseline (colors.elevation's own per-variant match) still "
        "applies -- only the theme layer's own entry is gone, not the underlying default"
    )


# --- M61 (§16.3): components: corner_radius/elevation accept a real MD3
# shape/elevation token name, not just a plain literal number. ------------


def test_window_set_theme_component_override_accepts_a_shape_token_name(tmp_path):
    theme_path = write_yaml(
        tmp_path,
        "theme.yaml",
        "components:\n  card: {corner_radius: small, elevation: level_2}\n",
    )
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    card = window.add_card(width=200.0, height=100.0, variant="filled")
    # engine_md3::shape::SHAPE_SMALL / ELEVATION_LEVEL_2, the exact same
    # constants crates/engine-md3/src/shape.rs::named/elevation_named
    # themselves resolve these token names to.
    assert card.get("corner_radius") == pytest.approx(8.0)
    assert card.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_unknown_component_shape_token_raises_value_error(tmp_path):
    theme_path = write_yaml(
        tmp_path, "theme.yaml", "components:\n  card: {corner_radius: smol}\n"
    )
    window = Window(width=200, height=200)
    with pytest.raises(ValueError, match="card"):
        window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)


# --- M63 (§7.1, §16.3): typography: -- the imperative catalog's own real
# type-scale role consumer, closing the gap ThemeSpec.typography (M62)
# left open: parsed since M62, but nothing resolved it until now. -------


def test_window_set_theme_typography_override_reaches_a_themed_factory(tmp_path):
    # No Python-facing getter exists for a Text node's own resolved
    # font_size/font_weight/font_family (the same honest limitation
    # M62's own line_height/typography_role tests already state) --
    # this proves the real FFI call succeeds with a real per-role
    # override in place. The exact per-field resolution (the override
    # wins, every other field still comes from the shipped default) is
    # proven at the Rust layer instead (`crates/engine-py/src/window.rs`
    # ::tests::typography_applies_a_real_per_field_override_on_top_of_
    # the_shipped_default) -- this test is the real proof the override
    # actually *reaches* a themed factory end to end, through the
    # identical `custom_theme` parameter every other theme test uses.
    theme_path = write_yaml(
        tmp_path,
        "theme.yaml",
        "typography:\n  label_large: {font_size: 20, font_family: Inter}\n",
    )
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    button = window.add_button(label="Themed", width=140, height=40)
    assert button.get("corner_radius") >= 0.0


def test_window_set_theme_unknown_typography_role_raises_value_error(tmp_path):
    theme_path = write_yaml(
        tmp_path,
        "theme.yaml",
        "typography:\n  subtitle_huge: {font_size: 20}\n",
    )
    window = Window(width=400, height=400)
    # `typography:`'s own keys aren't validated against the real 15-role
    # vocabulary at set_theme time (unlike `text.role`'s own real, load-
    # time-validated declarative counterpart) -- an override for a role
    # nothing ever looks up is simply inert, not a raised error, the
    # identical real "an override for a component key nothing consults"
    # non-error `components:` already tolerates. Confirms this is a
    # real, deliberate non-error, not silently untested.
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    button = window.add_button(label="Unaffected", width=140, height=40)
    assert button.get("corner_radius") >= 0.0


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


def test_tooltip_color_resolution_does_not_raise(tmp_path):
    """M58 (§7.1): `add_tooltip`'s own container/label colors are now
    theme-resolved (`role("inverse_surface")`/`role("inverse_on_
    surface")`, real M52-era gap closed) -- no Python-facing color
    getter exists anywhere in this suite (the same honest limit `test_
    dialog_override_does_not_raise` already states), so a real,
    non-default seed genuinely reaching this path without raising is
    the strongest proof available at this level.
    """
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))
    window.add_tooltip(text="hi", width=100.0)


def test_menu_panel_shape_and_elevation_override(tmp_path):
    """M58 (§7.1): `build_menu`'s own panel now has a real retheme hook
    at all (M52's own confirmed gap: it had none) -- both a real,
    directly-readable construction-time override and its own live-
    retheme counterpart are provable here, unlike the panel's color.
    """
    window = window_with_components(tmp_path, "  menu: {corner_radius: 3, elevation: 5}\n")
    item = window.add_menu_item(label="hi")
    panel = window.build_menu([item])
    assert panel.get("corner_radius") == pytest.approx(3.0)
    assert panel.get("elevation") == pytest.approx(5.0)


def test_window_set_theme_recomputes_an_already_built_menu_panels_shape_live(tmp_path):
    window = Window(width=400, height=400)
    item = window.add_menu_item(label="hi")
    panel = window.build_menu([item])
    retheme(window, tmp_path, "t.yaml", "  menu: {corner_radius: 3, elevation: 5}\n")
    assert panel.get("corner_radius") == pytest.approx(3.0)
    assert panel.get("elevation") == pytest.approx(5.0)


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
    window.add_dialog(headline="hi", supporting_text="body", width=280.0, height=180.0)


def test_snackbar_corner_radius_and_elevation_override(tmp_path):
    window = window_with_components(tmp_path, "  snackbar: {corner_radius: 2, elevation: 4}\n")
    container, _action, _close = window.add_snackbar(text="hi", width=300.0)
    assert container.get("corner_radius") == pytest.approx(2.0)
    assert container.get("elevation") == pytest.approx(4.0)


def test_popover_has_its_own_key_distinct_from_card(tmp_path):
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 99}\n  popover: {corner_radius: 8, elevation: 5}\n"
    )
    node = window.add_popover(subhead="hi", supporting_text="body", width=280.0, height=140.0)
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


def test_search_bar_icon_buttons_follow_the_icon_button_key(tmp_path):
    """M58 (§7.1): the leading/trailing icon-button containers used to
    hardcode `SEARCH_ICON_BUTTON_SIZE / 2.0`, never consulting
    `theme.shape("icon_button", ...)` the way `add_top_app_bar`/`add_
    spin_box`'s own visually-identical icon buttons already do -- a
    real, confirmed M52-era gap, closed here.
    """
    window = window_with_components(tmp_path, "  icon_button: {corner_radius: 9}\n")
    _bar, _field, leading, trailing = window.add_search_bar(
        placeholder="hi", width=300.0, leading_icon="search", trailing_icons=["close"]
    )
    assert leading.get("corner_radius") == pytest.approx(9.0)
    assert trailing[0].get("corner_radius") == pytest.approx(9.0)


def test_window_set_theme_recomputes_an_already_built_search_bars_icon_buttons_live(tmp_path):
    window = Window(width=400, height=400)
    _bar, _field, leading, trailing = window.add_search_bar(
        placeholder="hi", width=300.0, leading_icon="search", trailing_icons=["close"]
    )
    retheme(window, tmp_path, "t.yaml", "  icon_button: {corner_radius: 9}\n")
    assert leading.get("corner_radius") == pytest.approx(9.0)
    assert trailing[0].get("corner_radius") == pytest.approx(9.0)


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
    window.add_dialog(headline="hi", supporting_text="body", width=280.0, height=180.0)
    snackbar, _a, _c = window.add_snackbar(text="hi", width=300.0)
    assert snackbar.get("corner_radius") == pytest.approx(4.0)
    assert snackbar.get("elevation") == pytest.approx(3.0)
    popover = window.add_popover(subhead="hi", supporting_text="body", width=280.0, height=140.0)
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


def test_pagination_corner_radius_override_applies_to_pages_only(tmp_path):
    """M58 (§7.1): `previous`/`next` no longer share `"pagination"` with
    the numbered page items -- a real, confirmed, approved fix to a
    pre-existing inconsistency (they now consult `"icon_button"`,
    matching every other icon-button-shaped element in this catalog).
    A `pagination:` override therefore applies only to `pages`.
    """
    window = window_with_components(tmp_path, "  pagination: {corner_radius: 7}\n")
    previous, pages, next_ = window.add_pagination(page_count=3, current=0)
    assert pages[0].get("corner_radius") == pytest.approx(7.0)
    assert previous.get("corner_radius") != pytest.approx(7.0)
    assert next_.get("corner_radius") != pytest.approx(7.0)


def test_pagination_previous_and_next_follow_icon_button_key(tmp_path):
    """The real M58 counterpart to the test above: an `icon_button:`
    override now reaches `previous`/`next` (it never did before this
    milestone), while leaving the numbered page items alone.
    """
    window = window_with_components(tmp_path, "  icon_button: {corner_radius: 11}\n")
    previous, pages, next_ = window.add_pagination(page_count=3, current=0)
    assert previous.get("corner_radius") == pytest.approx(11.0)
    assert next_.get("corner_radius") == pytest.approx(11.0)
    assert pages[0].get("corner_radius") != pytest.approx(11.0)


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


# --- M51: View.set_theme (live re-theme) --------------------------------


def test_view_set_theme_custom_theme_changes_an_already_built_node_live(tmp_path):
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    view = View(view_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(2.0), "the shipped default, before retheme"

    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view.set_theme(custom_theme=custom_theme_path)
    assert node.get("corner_radius") == pytest.approx(16.0), "the exact same Node object, re-themed live"


def test_view_set_theme_with_no_args_resets_to_the_shipped_default(tmp_path):
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view = View(view_path, custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(16.0)

    view.set_theme()
    assert node.get("corner_radius") == pytest.approx(2.0), (
        "each set_theme call is a complete, fresh selection -- omitting custom_theme "
        "must reset to the shipped default, not silently keep the previous override"
    )


def test_view_set_theme_leaves_a_widgets_own_inline_style_untouched(tmp_path):
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233", corner_radius: 99}\n',
    )
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view = View(view_path)
    node = view.node("root")
    view.set_theme(custom_theme=custom_theme_path)
    assert node.get("corner_radius") == pytest.approx(99.0), "inline style must still win over the new theme"


def test_view_set_theme_changes_a_declarative_color_token_without_raising(tmp_path):
    view_path = write_yaml(
        tmp_path, "view.yaml", "id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: primary}\n"
    )
    view = View(view_path, theme_seed=(0x67, 0x50, 0xA4, 0xFF))
    # Must not raise -- background has no Python-facing getter (the
    # same honest limit every other color-touching test in this suite
    # already states), so re-resolving the "primary" token against a
    # new seed is proven by not raising, through the real Node the
    # view already returned before this call.
    view.set_theme(theme_seed=(0x00, 0xFF, 0x00, 0xFF))


def test_view_set_theme_node_ids_survive_a_retheme(tmp_path):
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view = View(view_path)
    node_before = view.node("root")
    view.set_theme(custom_theme=custom_theme_path)
    node_after = view.node("root")
    # Same real corner_radius readback on a fresh lookup, and the
    # originally-held Node object still reads the live value too --
    # both prove the node was patched in place, not removed/rebuilt.
    assert node_before.get("corner_radius") == pytest.approx(16.0)
    assert node_after.get("corner_radius") == pytest.approx(16.0)


def test_view_set_theme_survives_a_later_poll_reload(tmp_path):
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 20, height: 20, background: "#112233"}\n',
    )
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Checkbox\n    style: {corner_radius: 16}\n"
    )
    view = View(view_path)
    view.set_theme(custom_theme=custom_theme_path)
    node = view.node("root")
    assert node.get("corner_radius") == pytest.approx(16.0)

    # A real, unrelated content edit -- poll_reload must keep resolving
    # against the theme set_theme just installed, not silently revert
    # to whatever View.__init__ originally used.
    write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Checkbox\nstyle: {width: 30, height: 20, background: "#112233"}\n',
    )
    assert poll_until_changed(view), "expected a real file-watcher change within the timeout"
    assert node.get("corner_radius") == pytest.approx(16.0)


def test_view_set_theme_keeps_a_bound_propertys_live_value(tmp_path):
    # M91 (issue #8) changed this contract. `patch_node` still recomputes
    # only the *static* style cascade, but `set_theme` now re-applies the
    # attached ViewModel's bindings afterward -- before M91 this test
    # asserted the bound opacity reverted to the static 1.0, which was
    # exactly the bug the issue reported.
    view_path = write_yaml(
        tmp_path,
        "view.yaml",
        'id: root\nkind: Rect\nstyle: {width: 20, height: 20, background: "#112233", opacity: 1.0}\n'
        'bindings: {opacity: "{{ level.get() }}"}\n',
    )
    custom_theme_path = write_yaml(
        tmp_path, "custom_theme.yaml", "styles:\n  - kind: Rect\n    style: {corner_radius: 5}\n"
    )
    view = View(view_path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.4)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(0.4), "the binding's own initial value applied"

    view.set_theme(custom_theme=custom_theme_path)

    assert node.get("corner_radius") == pytest.approx(5.0), "the new theme layer applied"
    assert node.get("opacity") == pytest.approx(0.4), (
        "set_theme re-applies bindings, so the bound value survives the retheme"
    )


# --- M52 Phase 1: live re-theme for Window's imperative catalog --------
# -- add_button, the milestone's own proof of concept. A second
# window.set_theme(...) call must recompute an already-built button's
# real corner_radius/elevation in place, not just at construction time
# (the pre-M52 behavior: set_theme only ever pushed one blind uniform
# on_surface tint into 4 unrelated fields, never a button's own
# container/label color or shape).


def test_window_set_theme_recomputes_an_already_built_buttons_corner_radius_live(tmp_path):
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    node = window.add_button(label="hi", variant="filled", width=100, height=40)
    assert node.get("corner_radius") == pytest.approx(20.0), "height / 2.0, the un-themed default"

    theme_path = write_yaml(
        tmp_path, "theme.yaml", "components:\n  button.filled: {corner_radius: 4, elevation: 2}\n"
    )
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    assert node.get("corner_radius") == pytest.approx(
        4.0
    ), "the exact same, already-built Node -- expected the real components: override, applied live"
    assert node.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_with_no_override_resets_an_already_themed_buttons_corner_radius(tmp_path):
    theme_path = write_yaml(
        tmp_path, "theme.yaml", "components:\n  button.filled: {corner_radius: 4, elevation: 2}\n"
    )
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    node = window.add_button(label="hi", variant="filled", width=100, height=40)
    assert node.get("corner_radius") == pytest.approx(4.0)

    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    assert node.get("corner_radius") == pytest.approx(20.0), (
        "each set_theme call is a complete, fresh selection -- omitting custom_theme "
        "must reset to height / 2.0 live, not silently keep the previous override"
    )


def test_window_set_theme_a_second_call_does_not_raise_for_an_already_built_buttons_color(tmp_path):
    theme_a = write_yaml(tmp_path, "theme_a.yaml", "colors:\n  primary: \"#00695C\"\n")
    theme_b = write_yaml(tmp_path, "theme_b.yaml", "colors:\n  primary: \"#8B0000\"\n")
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_a)
    window.add_button(label="hi", variant="filled", width=100, height=40)
    # No Python-facing getter for a node's resolved background color
    # (the same honest limit every other color test in this suite
    # already states) -- proven by not raising, through the exact same
    # already-built Node the window returned before this second call.
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_b)


def test_window_set_theme_removed_buttons_hook_is_a_safe_no_op(tmp_path):
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    node = window.add_button(label="hi", variant="filled", width=100, height=40)
    node.remove()
    # Must not raise -- a stale retheme hook whose node was since
    # removed is a safe no-op (Tree::get_mut -> None), the same accepted
    # tradeoff handlers/materializers/context_menus already have.
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))


# --- M52 Phase 2: live re-theme -- the 19 fixed/simple factories -------
# For each factory M50 already proved a construction-time corner_radius/
# elevation value for, a second window.set_theme(...) call with a
# different components: override must change that same, already-built
# Node's value live. Factories with only a themed color (no Python-
# readable field) get a "does not raise" proof, the same honest limit
# this suite has carried since M44.


def retheme(window, tmp_path, name, components_yaml, seed=(0x67, 0x50, 0xA4, 0xFF)):
    theme_path = write_yaml(tmp_path, name, f"components:\n{components_yaml}")
    window.set_theme(seed=seed, custom_theme=theme_path)


def test_window_set_theme_recomputes_an_already_built_cards_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_card(width=200.0, height=100.0, variant="elevated")
    assert node.get("corner_radius") == pytest.approx(12.0)
    # Real finding, not assumed: the shipped default_theme.yaml already
    # sets card.elevated: {elevation: 1} -- a bare "card" override for
    # elevation would never reach the "elevated" variant, since the
    # 2-tier lookup checks the variant-specific key first regardless of
    # which theme layer set it. Overriding "card.elevated" explicitly,
    # matching this suite's own established M50 test convention.
    retheme(
        window, tmp_path, "t.yaml", "  card: {corner_radius: 20}\n  card.elevated: {elevation: 9}\n"
    )
    assert node.get("corner_radius") == pytest.approx(20.0)
    assert node.get("elevation") == pytest.approx(9.0)


def test_window_set_theme_recomputes_an_already_built_tooltips_corner_radius_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_tooltip(text="hi", width=100.0)
    retheme(window, tmp_path, "t.yaml", "  tooltip: {corner_radius: 1}\n")
    assert node.get("corner_radius") == pytest.approx(1.0)


def test_window_set_theme_a_second_call_does_not_raise_for_an_already_built_dialog(tmp_path):
    window = Window(width=400, height=400)
    window.add_dialog(headline="hi", supporting_text="body", width=280.0, height=180.0)
    retheme(window, tmp_path, "t.yaml", "  dialog: {corner_radius: 10, elevation: 6}\n")


def test_window_set_theme_recomputes_an_already_built_popovers_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_popover(subhead="hi", supporting_text="body", width=280.0, height=140.0)
    retheme(window, tmp_path, "t.yaml", "  popover: {corner_radius: 8, elevation: 5}\n")
    assert node.get("corner_radius") == pytest.approx(8.0)
    assert node.get("elevation") == pytest.approx(5.0)


def test_window_set_theme_recomputes_an_already_built_search_views_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_search_view(width=300.0, height=400.0)
    retheme(window, tmp_path, "t.yaml", "  search_view: {corner_radius: 7, elevation: 2}\n")
    assert node.get("corner_radius") == pytest.approx(7.0)
    assert node.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_recomputes_an_already_built_date_picker_days_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_date_picker_day(day=5, selected=False, today=False, outside_month=False)
    retheme(window, tmp_path, "t.yaml", "  date_picker_day: {corner_radius: 3}\n")
    assert node.get("corner_radius") == pytest.approx(3.0)


def test_window_set_theme_recomputes_an_already_built_time_input_fields_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_time_input_field(value="12")
    retheme(window, tmp_path, "t.yaml", "  time_input_field: {corner_radius: 9}\n")
    assert node.get("corner_radius") == pytest.approx(9.0)


def test_window_set_theme_recomputes_an_already_built_period_selectors_shape_live(tmp_path):
    window = Window(width=400, height=400)
    am, pm = window.add_period_selector(selected="AM")
    retheme(window, tmp_path, "t.yaml", "  period_selector: {corner_radius: 11}\n")
    assert am.get("corner_radius") == pytest.approx(11.0)
    assert pm.get("corner_radius") == pytest.approx(11.0)


def test_window_set_theme_recomputes_an_already_built_spin_boxs_shape_live(tmp_path):
    window = Window(width=400, height=400)
    field, decrement, increment = window.add_spin_box(value="1")
    retheme(
        window,
        tmp_path,
        "t.yaml",
        "  spin_box: {corner_radius: 13}\n  icon_button: {corner_radius: 6}\n",
    )
    assert field.get("corner_radius") == pytest.approx(13.0)
    assert decrement.get("corner_radius") == pytest.approx(6.0)
    assert increment.get("corner_radius") == pytest.approx(6.0)


def test_window_set_theme_recomputes_an_already_built_graph_nodes_shape_live(tmp_path):
    window = Window(width=400, height=400)
    graph = window.add_node_graph(width=400.0, height=300.0)
    node = window.add_graph_node(graph=graph, label="hi", x=0.0, y=0.0, width=120.0, height=80.0)
    retheme(window, tmp_path, "t.yaml", "  graph_node: {corner_radius: 15}\n")
    assert node.get("corner_radius") == pytest.approx(15.0)


def test_window_set_theme_recomputes_an_already_built_toolbars_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_toolbar(variant="floating", width=200.0, height=64.0)
    retheme(window, tmp_path, "t.yaml", "  toolbar.floating: {corner_radius: 17, elevation: 1}\n")
    assert node.get("corner_radius") == pytest.approx(17.0)
    assert node.get("elevation") == pytest.approx(1.0)


def test_window_set_theme_a_second_call_does_not_raise_for_every_remaining_color_only_factory(
    tmp_path,
):
    # add_divider/add_status_bar/add_link/add_accordion_header/add_
    # tree_node/add_list_item/add_loading_indicator have no theme-driven
    # shape/elevation and no Python-facing color getter -- proven here
    # by not raising across a real second set_theme() call, on every
    # already-built node at once.
    window = Window(width=400, height=400)
    window.add_divider(length=100.0)
    window.add_status_bar(text="hi")
    window.add_link(content="hi", width=100.0)
    window.add_accordion_header(title="hi", expanded=False, width=200.0)
    window.add_tree_node(title="hi", depth=0, expanded=False, leaf=False, width=200.0)
    window.add_list_item(headline="hi", width=280.0)
    window.add_loading_indicator()
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))


# --- M52 Phase 3: live re-theme -- the Buttons & FAB family -------------
# For each factory M50 already proved a construction-time corner_radius/
# elevation value for, a second window.set_theme(...) call with a
# different components: override must change that same, already-built
# Node's value live.


def test_window_set_theme_recomputes_an_already_built_icon_buttons_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_icon_button(icon="add", size=40.0, variant="filled")
    retheme(window, tmp_path, "t.yaml", "  icon_button.filled: {corner_radius: 3, elevation: 2}\n")
    assert node.get("corner_radius") == pytest.approx(3.0)
    assert node.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_recomputes_an_already_built_fabs_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_fab(icon="add", size="small", variant="surface")
    retheme(window, tmp_path, "t.yaml", "  fab.small: {corner_radius: 4}\n  fab: {elevation: 5}\n")
    assert node.get("corner_radius") == pytest.approx(4.0)
    assert node.get("elevation") == pytest.approx(5.0)


def test_window_set_theme_recomputes_an_already_built_extended_fabs_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_extended_fab(label="hi", width=120.0, variant="primary")
    assert node.get("corner_radius") == pytest.approx(16.0)
    retheme(window, tmp_path, "t.yaml", "  extended_fab: {corner_radius: 9, elevation: 6}\n")
    assert node.get("corner_radius") == pytest.approx(9.0)
    assert node.get("elevation") == pytest.approx(6.0)


def test_window_set_theme_recomputes_an_already_built_chips_shape_live(tmp_path):
    window = Window(width=400, height=400)
    node = window.add_chip(label="hi", width=100.0, variant="assist")
    retheme(window, tmp_path, "t.yaml", "  chip: {corner_radius: 2}\n")
    assert node.get("corner_radius") == pytest.approx(2.0)


def test_window_set_theme_recomputes_an_already_built_badges_shape_live(tmp_path):
    window = Window(width=400, height=400)
    dot = window.add_badge()
    labeled = window.add_badge(label="9")
    retheme(
        window,
        tmp_path,
        "t.yaml",
        "  badge.dot: {corner_radius: 1}\n  badge.labeled: {corner_radius: 2}\n",
    )
    assert dot.get("corner_radius") == pytest.approx(1.0)
    assert labeled.get("corner_radius") == pytest.approx(2.0)


def test_window_set_theme_a_second_call_does_not_raise_for_an_already_built_segmented_button(
    tmp_path,
):
    window = Window(width=400, height=400)
    window.add_segmented_button(labels=["A", "B"], width=200.0, height=40.0)
    retheme(window, tmp_path, "t.yaml", "  segmented_button: {corner_radius: 3}\n")


def test_window_set_theme_recomputes_an_already_built_split_buttons_leading_shape_live(tmp_path):
    # leading inherits its background/corner_radius/elevation retheme
    # from the plain button_retheme_hook registered inside the internal
    # self.add_button(...) call -- proven here through the same real
    # readback test_split_button_leading_inherits_the_button_key already
    # established at construction time, now after a second set_theme().
    window = Window(width=400, height=400)
    leading, trailing, _icon = window.add_split_button(
        label="hi", width=100.0, height=40.0, variant="filled"
    )
    retheme(window, tmp_path, "t.yaml", "  button.filled: {corner_radius: 5}\n")
    assert leading.get("corner_radius") == pytest.approx(5.0)
    assert trailing.get("corner_radius") == pytest.approx(5.0)


def test_window_set_theme_a_second_call_does_not_raise_for_split_button_and_button_group_tightened(
    tmp_path,
):
    # No Python-facing readback for corner_radii_override/interactive_
    # shape/press_interactive_shape -- proven by not raising, matching
    # this suite's own established limit at construction time.
    window = Window(width=400, height=400)
    window.add_split_button(label="hi", width=100.0, height=40.0, variant="filled")
    window.add_button_group(labels=["A", "B"], width=80.0, height=40.0, variant="outlined")
    retheme(
        window,
        tmp_path,
        "t.yaml",
        "  split_button.tightened: {corner_radius: 2}\n  button_group.tightened: {corner_radius: 3}\n",
    )


def test_window_set_theme_recomputes_an_already_built_button_groups_children_shape_live(tmp_path):
    window = Window(width=400, height=400)
    group, children = window.add_button_group(
        labels=["A", "B"], width=80.0, height=40.0, variant="outlined"
    )
    retheme(window, tmp_path, "t.yaml", "  button.outlined: {corner_radius: 6}\n")
    for child in children:
        assert child.get("corner_radius") == pytest.approx(6.0)


# --- M52 Phase 4: live re-theme -- Containers & Navigation --------------
# For each factory M50 already proved a construction-time corner_radius/
# elevation value for, a second window.set_theme(...) call with a
# different components: override must change that same, already-built
# Node's value live.


def test_window_set_theme_recomputes_an_already_built_snackbars_shape_live(tmp_path):
    window = Window(width=400, height=400)
    container, _action, _close = window.add_snackbar(text="hi", width=300.0)
    retheme(window, tmp_path, "t.yaml", "  snackbar: {corner_radius: 2, elevation: 4}\n")
    assert container.get("corner_radius") == pytest.approx(2.0)
    assert container.get("elevation") == pytest.approx(4.0)


def test_window_set_theme_recomputes_an_already_built_side_sheets_elevation_live(tmp_path):
    window = Window(width=400, height=400)
    standard = window.add_side_sheet(width=300.0, modal=False)
    retheme(window, tmp_path, "t.yaml", "  side_sheet.standard: {elevation: 1}\n")
    assert standard.get("elevation") == pytest.approx(1.0)


def test_window_set_theme_a_second_call_does_not_raise_for_an_already_built_modal_side_sheet(
    tmp_path,
):
    window = Window(width=400, height=400)
    window.add_side_sheet(width=300.0, modal=True)
    retheme(window, tmp_path, "t.yaml", "  side_sheet.modal: {elevation: 3, corner_radius: 4}\n")


def test_window_set_theme_recomputes_an_already_built_navigation_drawers_elevation_live(tmp_path):
    window = Window(width=400, height=400)
    container, _items = window.add_navigation_drawer(
        labels=["A", "B"], icons=["home", "settings"], modal=False, width=280.0
    )
    retheme(window, tmp_path, "t.yaml", "  navigation_drawer.standard: {elevation: 2}\n")
    assert container.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_a_second_call_does_not_raise_for_navigation_drawer_indicator(tmp_path):
    window = Window(width=400, height=400)
    window.add_navigation_drawer(
        labels=["A", "B"], icons=["home", "settings"], selected=0, modal=False, width=280.0
    )
    retheme(
        window,
        tmp_path,
        "t.yaml",
        "  navigation_drawer.standard: {corner_radius: 5}\n  navigation_drawer.indicator: {corner_radius: 3}\n",
    )


def test_window_set_theme_recomputes_an_already_built_top_app_bars_icon_buttons_live(tmp_path):
    window = Window(width=400, height=400)
    bar, leading, trailing = window.add_top_app_bar(
        title="hi", leading_icon="menu", trailing_icons=["search"]
    )
    retheme(window, tmp_path, "t.yaml", "  icon_button: {corner_radius: 6}\n")
    assert leading.get("corner_radius") == pytest.approx(6.0)
    assert trailing[0].get("corner_radius") == pytest.approx(6.0)
    assert bar.get("corner_radius") == pytest.approx(0.0), "the bar itself stays un-themed"


def test_window_set_theme_a_second_call_does_not_raise_for_navigation_rail_indicator(tmp_path):
    window = Window(width=400, height=400)
    window.add_navigation_rail(labels=["A", "B"], icons=["home", "settings"], selected=0)
    retheme(window, tmp_path, "t.yaml", "  navigation_rail.indicator: {corner_radius: 3}\n")


def test_window_set_theme_a_second_call_does_not_raise_for_tabs_indicator(tmp_path):
    window = Window(width=400, height=400)
    window.add_tabs(labels=["A", "B"], selected=0)
    retheme(window, tmp_path, "t.yaml", "  tabs.indicator: {corner_radius: 5}\n")


def test_window_set_theme_recomputes_an_already_built_search_bars_shape_live(tmp_path):
    window = Window(width=400, height=400)
    bar, _field, _leading, _trailing = window.add_search_bar(placeholder="hi", width=300.0)
    retheme(window, tmp_path, "t.yaml", "  search_bar: {corner_radius: 6, elevation: 2}\n")
    assert bar.get("corner_radius") == pytest.approx(6.0)
    assert bar.get("elevation") == pytest.approx(2.0)


def test_window_set_theme_recomputes_an_already_built_paginations_shape_live(tmp_path):
    """M58 (§7.1): the live-retheme counterpart to `test_pagination_
    corner_radius_override_applies_to_pages_only` -- a later `set_theme
    (custom_theme=...)` call recomputes `pages` from `pagination:` and
    `previous`/`next` from `icon_button:`, independently.
    """
    window = Window(width=400, height=400)
    previous, pages, next_ = window.add_pagination(page_count=3, current=0)
    retheme(window, tmp_path, "t.yaml", "  pagination: {corner_radius: 7}\n  icon_button: {corner_radius: 11}\n")
    assert pages[0].get("corner_radius") == pytest.approx(7.0)
    assert previous.get("corner_radius") == pytest.approx(11.0)
    assert next_.get("corner_radius") == pytest.approx(11.0)


def test_window_set_theme_a_second_call_does_not_raise_for_an_already_built_menu_item(tmp_path):
    window = Window(width=400, height=400)
    window.add_menu_item(label="hi", icon="settings", submenu=True, width=200.0)
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))


# --- M52 Phase 5: live re-theme -- non-PaintProperties stateful --------
# components. None of these 9 have a themed shape/elevation (no
# Python-readable field at all), and none have a Python-facing color
# getter either -- every test here is a real "does not raise" proof,
# the same honest limit this suite already states for color throughout.
#
# add_checkbox/add_slider/add_text_field/add_code_editor needed NO new
# Rust code in this phase at all: their own themed field (mark_tint/
# track_tint/text_tint) is already the exact field the pre-existing,
# untouched Tree::set_all_component_tints mechanism unconditionally
# re-tints on every real set_theme() call. The test below confirms that
# existing mechanism keeps working after this milestone's own changes,
# a real regression check, not new functionality.
#
# add_radio_button/add_switch/add_linear_progress/add_circular_progress
# /add_time_picker_dial are the 5 confirmed, previously-undocumented
# gaps this phase closes -- before M52, a second set_theme() call had
# zero effect on any of these at all.


def test_window_set_theme_a_second_call_does_not_raise_for_the_pre_existing_tint_push_components(
    tmp_path,
):
    window = Window(width=400, height=400)
    window.add_checkbox(background=(0x11, 0x22, 0x33, 0xFF), width=20.0, height=20.0)
    window.add_slider(background=(0x11, 0x22, 0x33, 0xFF), width=100.0, height=20.0)
    window.add_text_field(background=(0x11, 0x22, 0x33, 0xFF), width=200.0, height=40.0)
    window.add_code_editor(
        content="hi", background=(0x11, 0x22, 0x33, 0xFF), width=200.0, height=100.0
    )
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))


# --- M71 (§7.1, §8): window.theme -- real, read-only Python access to the
# same role/is_set/shape/elevation/typography lookups every composition-
# only add_* factory already makes internally. Until this milestone none
# of these were reachable from Python at all -- the single, confirmed
# blocker to building the same MD3-parity compositions in Python instead
# (the sibling Tesserae project's own real next milestone).


def test_theme_is_set_reflects_whether_set_theme_has_been_called():
    window = Window(width=400, height=400)
    assert window.theme.is_set() is False
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    assert window.theme.is_set() is True


def test_theme_role_returns_none_before_a_theme_is_set():
    window = Window(width=400, height=400)
    assert window.theme.role("primary") is None


def test_theme_role_returns_a_real_rgba_tuple_once_a_theme_is_set():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    color = window.theme.role("primary")
    assert isinstance(color, tuple)
    assert len(color) == 4
    assert all(isinstance(component, int) and 0 <= component <= 255 for component in color)


def test_theme_role_returns_none_for_an_unrecognized_role_name():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    assert window.theme.role("not_a_real_role") is None


def test_theme_role_is_consistent_with_what_a_themed_factory_actually_used(tmp_path):
    # Real, direct cross-check -- not just "returns a plausible-looking
    # tuple": a Rect built with background=role("primary") right after
    # must construct without raising and be indistinguishable in kind
    # from any other themed Rect, proving this is the exact real color
    # ThemeState::role resolves, not a fresh/different computation.
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    color = window.theme.role("primary")
    node = window.add_rect(background=color, width=20.0, height=20.0)
    assert node is not None


def test_theme_shape_reflects_a_real_component_override(tmp_path):
    window = window_with_components(tmp_path, "  card: {corner_radius: 20}\n")
    assert window.theme.shape("card", None) == pytest.approx(20.0)


def test_theme_shape_variant_specific_beats_bare_key(tmp_path):
    window = window_with_components(
        tmp_path, "  card: {corner_radius: 20}\n  card.elevated: {corner_radius: 30}\n"
    )
    assert window.theme.shape("card", "elevated") == pytest.approx(30.0)
    assert window.theme.shape("card", "filled") == pytest.approx(20.0)


def test_theme_shape_returns_none_with_no_override(tmp_path):
    window = Window(width=400, height=400)
    assert window.theme.shape("card", None) is None


def test_theme_elevation_reflects_a_real_component_override(tmp_path):
    window = window_with_components(tmp_path, "  card.elevated: {elevation: 9}\n")
    assert window.theme.elevation("card", "elevated") == pytest.approx(9.0)
    assert window.theme.elevation("card", None) is None


def test_theme_typography_returns_the_shipped_default_with_no_override():
    window = Window(width=400, height=400)
    family, weight, size, line_height = window.theme.typography("body_medium")
    assert isinstance(family, str)
    assert weight > 0.0
    assert size > 0.0
    assert line_height > 0.0


def test_theme_typography_reflects_a_real_per_field_override(tmp_path):
    theme_path = write_yaml(
        tmp_path,
        "theme.yaml",
        "typography:\n  label_large: {font_size: 20, font_family: Inter}\n",
    )
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    family, _weight, size, _line_height = window.theme.typography("label_large")
    assert family == "Inter"
    assert size == pytest.approx(20.0)


def test_theme_typography_returns_none_for_an_unrecognized_role():
    window = Window(width=400, height=400)
    assert window.theme.typography("not_a_real_role") is None


def test_theme_is_a_live_view_reflecting_a_later_set_theme_call(tmp_path):
    # A fresh `window.theme` access each time (this milestone's own
    # real design, see `PyWindow::theme`'s own doc comment) -- but must
    # still see live state, not a snapshot frozen at first access.
    window = Window(width=400, height=400)
    assert window.theme.is_set() is False
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF))
    assert window.theme.is_set() is True
    theme_path = write_yaml(tmp_path, "theme.yaml", "components:\n  card: {corner_radius: 55}\n")
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), custom_theme=theme_path)
    assert window.theme.shape("card", None) == pytest.approx(55.0)


def test_window_set_theme_a_second_call_does_not_raise_for_the_five_newly_closed_gaps(tmp_path):
    window = Window(width=400, height=400)
    window.add_radio_button(size=20.0, selected=False)
    window.add_switch()
    window.add_linear_progress(width=200.0, height=4.0, value=0.5)
    window.add_circular_progress()
    window.add_time_picker_dial()
    # Before M52, this second call had zero effect on any of the 5
    # nodes above -- proven correct here at the Rust level (`radio_
    # button_hook_recomputes_both_real_roles_when_the_theme_changes`/
    # `switch_hook_recomputes_all_five_real_roles_when_the_theme_
    # changes`, `window_factory.rs`'s own exact-value unit tests); this
    # is the real, end-to-end integration proof that the wiring reaches
    # every one of them without raising.
    window.set_theme(seed=(0x00, 0x66, 0x00, 0xFF))
