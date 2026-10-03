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
`untrack`) are gone; M99: so are the MD3 widgets and theming. A framework
(Tesserae) owns all of them.
"""

from tre._core import (
    App,
    Painter,
    Event,
    Gradient,
    LoopHandle,
    Node,
    Shader,
    ShaderError,
    Window,
    register_font,
)
from tre.snapshot import png_bytes, write_png

#: M32 Phase 1 (§5, §8, §10): the bundled monospace face a terminal always
#: shapes with (`engine_render::MONOSPACE_FONT_FAMILY`, "Hack Nerd Font
#: Mono") -- give it to a code editor's text input, and to siblings (a
#: gutter, line numbers) that must line up with a terminal's or editor's
#: grid.
MONOSPACE_FONT_FAMILY = "Hack Nerd Font Mono"

__all__ = [
    "App",
    "Painter",
    "Event",
    "Gradient",
    "LoopHandle",
    "MONOSPACE_FONT_FAMILY",
    "Node",
    "Shader",
    "ShaderError",
    "Window",
    "png_bytes",
    "register_font",
    "write_png",
]

# M97: the migration gate's switch -- see `tre/_removed.py`.
import os as _os

if _os.environ.get("TRE_FORBID_REMOVED") == "1":
    from tre._removed import install as _install

    _install()
