"""M57 (§8): real, repeatable coverage that `Window`'s own "act on the
currently focused node" methods -- `select_all`/`press_key`/
`type_text`/`copy`/`cut`/`copy_terminal_selection`/`resize` -- read
through `self.active`, not the window's plain, construction-time
`self.tree`/`self.handlers`/`self.root` fields, and so correctly follow
a real `Window.show_view()` switch. Before this milestone, each of
these acted on whichever `View` the `Window` was originally built from
(or its own imperatively-built tree), even after switching to a
different `View` -- a real, confirmed staleness bug found while
writing M56's own tests.

Same real `show_view`-switch pattern `test_view_in_window.py`'s own
`test_show_view_switches_a_live_windows_dispatch_to_a_different_view`
already established for `click`/`focus` -- this file extends that
proof to every method M57 fixed.

`route_to_terminal`/`route_control_char_to_terminal` (used by `press_
key`/`type_text`/`press_ctrl`'s own real terminal-routing check) were
also fixed, but `NodeKind::Terminal` has no real declarative YAML
representation at all (confirmed via grep of `engine-spec`) -- a
`Terminal` can never live inside a `View`'s own tree, so there is no
real way to prove their fix specifically through a `show_view` switch.
`tests/test_terminal.py`'s own existing, unmodified coverage of `copy_
terminal_selection`/`press_ctrl` on a plain (never-`show_view`-called)
`Window` already gives full regression coverage for the ordinary case
-- `self.active.tree` is the same `Rc` as `self.tree` at construction,
so routing through `active` is provably a no-op change for that case.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_view_in_window.py`.
"""

from tre import Signal, View, ViewModel, Window

FIELD_EMPTY = """
id: root
kind: TextField
text: {{content: "{content}", font_family: Roboto, font_size: 16}}
style: {{width: 100, height: 24, background: "#ffffff"}}
"""


def write_field(tmp_path, name, content=""):
    path = tmp_path / name
    path.write_text(FIELD_EMPTY.format(content=content))
    return str(path)


def test_select_all_and_copy_follow_a_show_view_switch(tmp_path):
    """Real, decisive proof: `select_all`/`copy` must act on `view_b`'s
    own focused field after `show_view`, not `view_a`'s (the one the
    `Window` was originally built from) -- only possible if both
    genuinely read through `self.active`, not `self.tree` directly.
    """
    path_a = write_field(tmp_path, "a.yaml", content="")
    path_b = write_field(tmp_path, "b.yaml", content="hello")
    view_a = View(path_a)
    view_b = View(path_b)
    ViewModel(view_a)
    ViewModel(view_b)

    window = Window.from_view(view_a, width=200, height=100)
    window.show_view(view_b)
    window.focus(view_b.node("root"))

    assert window.select_all() is True
    assert window.copy() == "hello"


def test_press_key_and_type_text_follow_a_show_view_switch(tmp_path):
    """`type_text` must insert into `view_b`'s own focused field, and
    `press_key`'s own dispatch (Home, then Shift+End to select) must
    act on that same field -- both only possible through `self.active`.
    """
    path_a = write_field(tmp_path, "a.yaml", content="")
    path_b = write_field(tmp_path, "b.yaml", content="")
    view_a = View(path_a)
    view_b = View(path_b)
    ViewModel(view_a)
    ViewModel(view_b)

    window = Window.from_view(view_a, width=200, height=100)
    window.show_view(view_b)
    field_b = view_b.node("root")
    window.focus(field_b)

    window.type_text("hi")
    assert field_b.get_text() == "hi"

    window.press_key("home")
    window.press_key("end", shift=True)
    assert window.copy() == "hi"


def test_cut_follows_a_show_view_switch(tmp_path):
    """`cut` must read/remove `view_b`'s own real selection and fire
    its own `on_change` handler against `view_b`'s own tree -- proving
    `cut`'s local `NodeContext` (built from `self.active`, not `self.
    tree`/`self.handlers` directly) is correct too, not just the raw
    text mutation.
    """
    path_a = write_field(tmp_path, "a.yaml", content="")
    path_b = write_field(tmp_path, "b.yaml", content="cutme")
    view_a = View(path_a)
    view_b = View(path_b)
    ViewModel(view_a)
    ViewModel(view_b)

    window = Window.from_view(view_a, width=200, height=100)
    window.show_view(view_b)
    field_b = view_b.node("root")
    window.focus(field_b)
    window.select_all()

    calls = []
    field_b.set_on_change(lambda: calls.append(field_b.get_text()))

    assert window.cut() == "cutme"
    assert field_b.get_text() == ""
    assert calls == [""], "cut's own Change handler must fire against view_b's own field"


def test_resize_does_not_raise_and_the_active_views_own_node_stays_clickable(tmp_path):
    """`resize`'s own `Tree::dispatch(Resized)` call must target
    `view_b`'s own currently-active tree/root, not `view_a`'s. No
    layout-inspection API is exposed to Python at all (the same real,
    stated limit `test_view_in_window.py`'s own `test_from_view_
    shares_the_same_live_size_cell_as_the_window` already names) -- the
    only real, observable proxy at this level is that a resize after a
    `show_view` switch doesn't raise, and a click on the *currently
    active* view's own node still finds it correctly afterward.
    """
    path_a = write_field(tmp_path, "a.yaml", content="")
    path_b = write_field(tmp_path, "b.yaml", content="")
    view_a = View(path_a)
    view_b = View(path_b)
    ViewModel(view_a)
    ViewModel(view_b)

    window = Window.from_view(view_a, width=200, height=100)
    window.show_view(view_b)

    window.resize(500, 400)  # must not raise

    calls = []
    field_b = view_b.node("root")
    field_b.set_on_click(lambda: calls.append("clicked"))
    window.click(field_b)
    assert calls == ["clicked"], "view_b's own node must still be reachable after resize"
