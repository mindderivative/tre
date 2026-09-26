"""tre v2 -- a GUI engine's building blocks, for a framework to build on.

`App` collects one or more `Window`s and drives them all together in one
blocking `App.run()` call; each `Window` owns its own node tree and size:

    win1 = Window(width=400, height=200, title="Main")
    win2 = Window(width=300, height=150, title="Panel")
    app = App()
    app.add_window(win1)
    app.add_window(win2)
    app.run()

M98: the declarative layer -- `View`, `Component`, `{{ }}` bindings -- and
the reactivity layer (`Signal`, `Computed`, `Effect`, `ViewModel`, `batch`,
`untrack`) are gone; a framework (Tesserae) owns both.
"""

from tre._core import (
    App,
    CanvasContext,
    Event,
    LoopHandle,
    Node,
    Theme,
    Window,
    register_font,
)

#: M32 Phase 1 (§5, §8, §10): the real bundled monospace face
#: `Window.add_terminal`/`add_code_editor` themselves always shape
#: with internally (`engine_render::MONOSPACE_FONT_FAMILY`, "Hack
#: Nerd Font Mono") -- exported here so app-composed siblings (a
#: gutter's own `Text` node, a fold toggle) that must line up with the
#: real editor grid can match its exact real font_family rather than
#: guessing or drifting out of sync with it.
MONOSPACE_FONT_FAMILY = "Hack Nerd Font Mono"

__all__ = [
    "App",
    "CanvasContext",
    "Event",
    "LoopHandle",
    "MONOSPACE_FONT_FAMILY",
    "Node",
    "Theme",
    "Window",
    "register_font",
]

# M97: the migration gate's switch -- see `tre/_removed.py`.
import os as _os

if _os.environ.get("TRE_FORBID_REMOVED") == "1":
    from tre._removed import install as _install

    _install()
