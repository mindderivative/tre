"""M30 Phase 7 Step 1 (§5, §7): real, repeatable coverage of
`Window.add_date_picker_day` -- the *docked* Date Picker variant's own
real day-cell anatomy. Deliberately scoped to just the cell; a real
calendar grid needs real date arithmetic (month lengths, weekday-of-
month, leap years), which is genuinely application logic with zero
real MD3-specific content, already trivially available via Python's
own `datetime`/`calendar` modules -- not an engine-owned primitive.
"""

from tre import Node, Window


def test_add_date_picker_day_plain_returns_a_node():
    window = Window(width=400, height=400)
    cell = window.add_date_picker_day(day=15)
    assert isinstance(cell, Node)


def test_add_date_picker_day_selected_does_not_raise():
    window = Window(width=400, height=400)
    cell = window.add_date_picker_day(day=15, selected=True)
    assert isinstance(cell, Node)


def test_add_date_picker_day_today_does_not_raise():
    window = Window(width=400, height=400)
    cell = window.add_date_picker_day(day=15, today=True)
    assert isinstance(cell, Node)


def test_add_date_picker_day_selected_and_today_does_not_raise():
    """Real, deliberate state precedence: both booleans can be True
    at once (today happens to also be selected) -- selected wins
    visually, but the API itself doesn't reject the combination.
    """
    window = Window(width=400, height=400)
    cell = window.add_date_picker_day(day=15, selected=True, today=True)
    assert isinstance(cell, Node)


def test_add_date_picker_day_outside_month_does_not_raise():
    window = Window(width=400, height=400)
    cell = window.add_date_picker_day(day=31, outside_month=True)
    assert isinstance(cell, Node)


def test_a_themed_date_picker_day_does_not_raise():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    cell = window.add_date_picker_day(day=15, selected=True)
    assert isinstance(cell, Node)


def test_a_real_month_grid_of_day_cells_can_be_built_from_python_date_math():
    """The real point of this step's own deliberate scope: the app
    composes a genuine calendar using nothing but Python's own
    `calendar` module plus this one real per-cell primitive, with no
    engine-owned date arithmetic anywhere.
    """
    import calendar

    window = Window(width=400, height=400)
    year, month = 2026, 9
    weeks = calendar.monthcalendar(year, month)
    today_day = 18

    cells = []
    for week in weeks:
        for day in week:
            if day == 0:
                continue
            cell = window.add_date_picker_day(day=day, today=(day == today_day))
            cells.append(cell)

    assert len(cells) == 30, "September 2026 has 30 real days"
    assert all(isinstance(cell, Node) for cell in cells)


def test_each_day_cell_is_a_real_independently_clickable_node():
    window = Window(width=400, height=400)
    cells = [window.add_date_picker_day(day=d) for d in (1, 2, 3)]

    clicked = []
    for i, cell in enumerate(cells):
        cell.enable_interaction()
        cell.set_on_click(lambda i=i: clicked.append(i))

    window.click(cells[1])
    assert clicked == [1], "clicking one day cell must reach only its own registered handler"
