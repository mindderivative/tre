"""M30 Phase 8 Step 6 (§11.3): real, repeatable coverage of `Main Menu`
submenus -- extends the existing context-menu overlay mechanism this
codebase already has real since M4 Phase 7, not a new overlay kind.
`add_menu_item`'s own `submenu=True` adds a purely visual trailing
chevron; the real submenu itself is just another real
`build_menu`/`open_menu` pair, opened with the parent item's own
returned `Node` as the anchor.
"""

from tre import Node, Window


def test_add_menu_item_with_submenu_true_does_not_raise():
    window = Window(width=400, height=300)
    item = window.add_menu_item(label="Export As", submenu=True)
    assert isinstance(item, Node)


def test_add_menu_item_submenu_and_icon_together_does_not_raise():
    window = Window(width=400, height=300)
    item = window.add_menu_item(label="Export As", icon="add", submenu=True)
    assert isinstance(item, Node)


def test_a_real_submenu_opens_anchored_to_its_own_parent_item():
    """The real point of this step: a submenu is just another real
    Menu, opened with the parent item's own returned Node as the
    anchor -- reusing open_menu/close_menu exactly as-is, no new
    overlay kind, confirmed directly rather than assumed.
    """
    window = Window(width=400, height=300)

    parent_item = window.add_menu_item(label="Export As", submenu=True)
    _parent_menu = window.build_menu([parent_item])

    submenu_items = [window.add_menu_item(label=label) for label in ["PDF", "PNG", "SVG"]]
    submenu = window.build_menu(submenu_items)

    window.open_menu(parent_item, submenu)  # must not raise -- parent_item is a real anchor
    window.close_menu(submenu)  # must not raise


def test_a_submenu_item_is_a_real_independently_clickable_node_once_the_submenu_is_open():
    window = Window(width=400, height=300)

    parent_item = window.add_menu_item(label="Export As", submenu=True)
    _parent_menu = window.build_menu([parent_item])

    pdf_item = window.add_menu_item(label="PDF")
    submenu = window.build_menu([pdf_item])
    window.open_menu(parent_item, submenu)

    calls = []
    pdf_item.enable_interaction()
    pdf_item.set_on_click(lambda: calls.append("pdf"))
    window.click(pdf_item)
    assert calls == ["pdf"], "a real click on a submenu item must reach its own registered handler"


def test_the_parent_menu_survives_a_real_click_on_its_own_submenu_item():
    """M30 Phase 8 Step 6: the real, confirmed bug this step's own
    `examples/main_menu_submenus.py` found live -- `open_overlay`
    always positions content anchor-relative-*below*, so a submenu
    anchored to a parent menu's own item genuinely sits outside the
    parent menu's own bounds. Before the fix, a real click on a
    submenu item was wrongly treated as an "outside click" against
    the *parent* menu, dismissing it and swallowing the click before
    it ever reached the submenu item's own handler (`Tree::dismiss_
    overlays_outside`'s own fix, proven directly at the engine-core
    level too).

    Unlike the tests above, both menus here are opened via a real
    `window.click()` (not `open_menu` called directly), matching the
    example's own real click-driven reproduction. The parent menu's
    own survival is proven behaviorally: clicking `export_item` (a
    real child of `file_menu`) a second time, after the submenu
    click, must still reach its own handler -- only possible if
    `export_item` is still genuinely attached to the tree.
    """
    window = Window(width=400, height=300)

    trigger = window.add_button(label="File", width=100, height=40)
    export_item = window.add_menu_item(label="Export As", submenu=True)
    file_menu = window.build_menu([export_item], width=180)

    trigger.enable_interaction()
    trigger.set_on_click(lambda: window.open_menu(trigger, file_menu))

    pdf_item = window.add_menu_item(label="PDF")
    export_submenu = window.build_menu([pdf_item], width=140)

    export_clicks: list[str] = []

    def open_export_submenu() -> None:
        export_clicks.append("export")
        window.open_menu(export_item, export_submenu)

    export_item.enable_interaction()
    export_item.set_on_click(open_export_submenu)

    pdf_calls: list[str] = []
    pdf_item.enable_interaction()
    pdf_item.set_on_click(lambda: pdf_calls.append("pdf"))

    window.click(trigger)
    window.click(export_item)
    window.click(pdf_item)
    assert pdf_calls == ["pdf"], "a real click on a submenu item must reach its own registered handler"

    window.click(export_item)
    assert export_clicks == ["export", "export"], (
        "the parent menu must still be genuinely open after a submenu click -- a wrongly "
        "dismissed parent menu would detach export_item from the tree, making this second "
        "click unable to reach its own handler"
    )


def test_a_reopened_submenu_after_closing_does_not_raise():
    window = Window(width=400, height=300)

    parent_item = window.add_menu_item(label="Export As", submenu=True)
    _parent_menu = window.build_menu([parent_item])

    submenu_items = [window.add_menu_item(label=label) for label in ["PDF", "PNG"]]
    submenu = window.build_menu(submenu_items)

    window.open_menu(parent_item, submenu)
    window.close_menu(submenu)
    window.open_menu(parent_item, submenu)  # must not raise -- real reopen
