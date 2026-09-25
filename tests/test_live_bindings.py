"""M91 (tre issue #8): live view updates keep `{{ }}` bound values, and
`View.set_stylesheet` replaces a stylesheet in place.

Before M91, `View._attach` discarded what it wired up, so `set_theme`,
`reconcile`, and `poll_reload` -- which rebuild a patched node from its
*static* spec -- left bound fields showing their placeholder until the
bound `Signal` next changed. The `View` now keeps its attachment and
re-applies it after every update.
"""

import time

import pytest

from tre import Signal, View, ViewModel, Window

SEED = (0x67, 0x50, 0xA4, 0xFF)


def base_spec():
    return {
        "id": "root",
        "kind": "Container",
        "style": {"width": 200, "height": 100},
        "children": [
            {
                "id": "label",
                "kind": "Text",
                "text": {"content": "static", "font_family": "Roboto", "font_size": 16},
                "style": {"width": 100, "height": 20, "foreground": "#000000"},
                "bindings": {"text": "{{ label.get() }}"},
            },
            {"id": "other", "kind": "Rect", "style": {"width": 10, "height": 10, "background": "#112233"}},
        ],
    }


def text_field_spec():
    spec = base_spec()
    spec["children"][0] = {
        "id": "label",
        "kind": "TextField",
        "text": {"content": "", "font_family": "Roboto", "font_size": 16},
        "style": {"width": 100, "height": 20, "background": "#EEEEEE"},
        "bindings": {"text": "{{ label.get() }}"},
        "handlers": {"on_change": "on_click"},
    }
    return spec


class LabelVM(ViewModel):
    def __init__(self, view):
        self.label = Signal("bound")
        self.clicks = 0
        super().__init__(view)

    def on_click(self):
        self.clicks += 1


def attached(spec=None, **view_kwargs):
    view = View(spec=spec or base_spec(), theme_seed=SEED, **view_kwargs)
    return view, LabelVM(view)


# --- the issue's reproduction -----------------------------------------------


def test_a_bound_value_survives_set_theme():
    view, _vm = attached()
    view.set_theme(theme_seed=SEED, custom_theme_spec={"colors": {"primary": "#00FF00"}})
    assert view.node("label").get_text() == "bound"


def test_a_bound_value_survives_a_reconcile_that_edits_the_bound_nodes_style():
    view, _vm = attached()
    spec = base_spec()
    spec["children"][0]["style"]["width"] = 120
    view.reconcile(spec=spec)
    assert view.node("label").get_text() == "bound"


def test_a_bound_value_wins_over_an_edited_static_placeholder():
    view, _vm = attached()
    spec = base_spec()
    spec["children"][0]["text"]["content"] = "edited"
    view.reconcile(spec=spec)
    assert view.node("label").get_text() == "bound"


def test_a_bound_value_survives_a_reconcile_that_edits_another_node():
    view, _vm = attached()
    spec = base_spec()
    spec["children"][1]["style"]["width"] = 20
    view.reconcile(spec=spec)
    assert view.node("label").get_text() == "bound"


def test_the_signal_still_drives_the_node_after_an_update():
    view, vm = attached()
    view.set_theme(theme_seed=SEED)
    vm.label.set("changed")
    assert view.node("label").get_text() == "changed"


# --- re-attach picks up the current spec -------------------------------------


def test_a_binding_added_by_a_reconcile_takes_effect():
    spec = base_spec()
    del spec["children"][0]["bindings"]
    view, vm = attached(spec)
    assert view.node("label").get_text() == "static"
    view.reconcile(spec=base_spec())
    assert view.node("label").get_text() == "bound"
    vm.label.set("live")
    assert view.node("label").get_text() == "live"


def test_a_removed_bound_node_no_longer_receives_signal_writes():
    view, vm = attached()
    spec = base_spec()
    del spec["children"][0]
    view.reconcile(spec=spec)
    vm.label.set("after removal")  # must not raise or panic into a missing node


def test_a_signal_write_after_several_updates_applies_once():
    view, vm = attached(text_field_spec())
    for _ in range(3):
        view.set_theme(theme_seed=SEED)
    vm.clicks = 0
    vm.label.set("once")
    assert view.node("label").get_text() == "once"
    assert vm.clicks == 1, "stale callbacks from earlier attachments must not fire too"


def test_a_handler_removed_by_a_reconcile_stops_firing():
    spec = base_spec()
    spec["children"][1]["handlers"] = {"on_click": "on_click"}
    view, vm = attached(spec)
    view.click(view.node("other"))
    assert vm.clicks == 1
    view.reconcile(spec=base_spec())
    view.click(view.node("other"))
    assert vm.clicks == 1


def test_an_imperatively_registered_handler_survives_an_update():
    view, _vm = attached()
    hits = []
    view.node("other").set_on_click(lambda: hits.append(1))
    view.set_theme(theme_seed=SEED)
    view.click(view.node("other"))
    assert hits == [1]


def test_a_second_attach_replaces_the_first():
    view = View(spec=text_field_spec(), theme_seed=SEED)
    first = LabelVM(view)
    second = LabelVM(view)
    first.clicks = second.clicks = 0
    first.label.set("from the first")  # the first attachment is gone
    assert first.clicks == 0
    assert view.node("label").get_text() == "bound"


def test_an_update_before_attach_is_unchanged():
    view = View(spec=base_spec(), theme_seed=SEED)
    view.set_theme(theme_seed=SEED)
    assert view.node("label").get_text() == "static"


# --- set_stylesheet ------------------------------------------------------------


def test_set_stylesheet_restyles_in_place_and_keeps_node_identity():
    view, _vm = attached()
    root = view.node("root")
    view.set_stylesheet(stylesheet_spec={"styles": [{"kind": "Container", "style": {"corner_radius": 7}}]})
    assert root.get("corner_radius") == pytest.approx(7.0), "the old handle must still reach the live node"
    assert view.node("label").get_text() == "bound"


def test_set_stylesheet_with_nothing_clears_the_stylesheet():
    sheet = {"styles": [{"kind": "Container", "style": {"corner_radius": 7}}]}
    view, _vm = attached(stylesheet_spec=sheet)
    assert view.node("root").get("corner_radius") == pytest.approx(7.0)
    view.set_stylesheet()
    assert view.node("root").get("corner_radius") == pytest.approx(0.0)


def test_set_stylesheet_accepts_a_file_path(tmp_path):
    path = tmp_path / "sheet.yaml"
    path.write_text("styles:\n  - kind: Container\n    style: {corner_radius: 5}\n")
    view, _vm = attached()
    view.set_stylesheet(stylesheet=str(path))
    assert view.node("root").get("corner_radius") == pytest.approx(5.0)


def test_set_stylesheet_rejects_both_forms_naming_the_method(tmp_path):
    path = tmp_path / "sheet.yaml"
    path.write_text("styles: []\n")
    view, _vm = attached()
    with pytest.raises(ValueError, match="View.set_stylesheet"):
        view.set_stylesheet(stylesheet_spec={"styles": []}, stylesheet=str(path))


def test_the_new_stylesheet_is_kept_for_later_updates():
    view, _vm = attached()
    view.set_stylesheet(stylesheet_spec={"styles": [{"kind": "Container", "style": {"corner_radius": 7}}]})
    view.set_theme(theme_seed=SEED)
    assert view.node("root").get("corner_radius") == pytest.approx(7.0)


# --- the file path: poll_reload --------------------------------------------------

VIEW_YAML = """\
id: root
kind: Container
style: {width: 200, height: 100}
children:
  - id: label
    kind: Text
    text: {content: static, font_family: Roboto, font_size: 16}
    style: {width: WIDTH, height: 20, foreground: "#000000"}
    bindings: {text: "{{ label.get() }}"}
"""


def test_a_bound_value_survives_poll_reload(tmp_path):
    path = tmp_path / "view.yaml"
    path.write_text(VIEW_YAML.replace("WIDTH", "100"))
    view = View(str(path))
    LabelVM(view)
    time.sleep(0.1)
    path.write_text(VIEW_YAML.replace("WIDTH", "120"))
    deadline = time.time() + 5
    while not view.poll_reload():
        assert time.time() < deadline, "poll_reload never saw the edit"
        time.sleep(0.05)
    assert view.node("label").get_text() == "bound"


# --- a View shown in a Window -------------------------------------------------------


def test_updates_work_on_a_view_shown_in_a_window():
    view, _vm = attached()
    Window.from_view(view, width=200, height=100)
    view.set_theme(theme_seed=SEED)
    assert view.node("label").get_text() == "bound"
