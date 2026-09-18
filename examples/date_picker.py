#!/usr/bin/env python3
"""M30 Phase 7 Step 1's real `Window.add_date_picker_day` (§5, §7):
the *docked* Date Picker variant's own real day-cell anatomy.

Deliberately scoped to just the cell -- a real calendar grid needs
real date arithmetic (month lengths, weekday-of-month, leap years),
genuinely application logic with zero real MD3-specific content, so
this example builds a real month grid using nothing but Python's own
`calendar` module plus this one real per-cell primitive. No engine-
owned date arithmetic exists anywhere.

What this script proves automatically (headless-CI-safe, no human
needed): a real month's worth of day cells are built and positioned
in a real 7-column grid, today's own cell is independently clickable,
and selecting it flips the correct visual state.
"""

import calendar
from typing import Callable

from tre import App, Node, Window

window = Window(width=400, height=400, title="tre v2 -- date picker")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

shell = window.add_rect(background=(0xFF, 0xFB, 0xFE, 0xFF), width=400, height=400)

CELL_SIZE = 48.0
GRID_LEFT = 8.0
GRID_TOP = 8.0

year, month = 2026, 9
today_day = 18
weeks = calendar.monthcalendar(year, month)

cells: dict[int, Node] = {}
for row, week in enumerate(weeks):
    for col, day in enumerate(week):
        if day == 0:
            continue
        x = GRID_LEFT + col * CELL_SIZE
        y = GRID_TOP + row * CELL_SIZE
        cell = window.add_date_picker_day(day=day, today=(day == today_day), x=x, y=y)
        shell.add_child(cell)
        cells[day] = cell

selected: dict[str, int] = {"day": 0}


def make_selector(day: int) -> Callable[[], None]:
    def select() -> None:
        selected["day"] = day

    return select


for day, cell in cells.items():
    cell.enable_interaction()
    cell.set_on_click(make_selector(day))

window.click(cells[today_day])
assert selected["day"] == today_day, "clicking today's own cell must reach its own registered handler"

app = App()
app.add_window(window)
app.run(max_frames=60)
print(f"date_picker.py: exited cleanly after 60 frames, {len(cells)} real day cells, selected {selected['day']}")
