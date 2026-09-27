# Accessibility & Fonts

*Theming left `tre` in 0.3.5:* colour schemes, shape, elevation, and type
scales belong to a framework built on it, such as Tesserae.

## Custom fonts

`tre` ships four vendored faces (Roboto Regular/Medium, Noto Sans
Arabic, Hack Nerd Font Mono) and deliberately never discovers system
fonts, so rendering is identical on every machine. To use any other
family, load the font file yourself and register its bytes:

```python
from pathlib import Path
import tre

families = tre.register_font(Path("fonts/Inter-Regular.ttf").read_bytes())
# families == ["Inter"] -- the exact name to use in font_family
```

Registration is process-wide: every existing and future window sees the
font, and a window already running picks it up on its next frame.
Registering identical bytes twice is harmless. Data containing no
parseable font face raises `ValueError`. Until a family is registered, a
`font_family` naming it falls back to a bundled face, so check the
returned names against what your theme's `typography:` uses.

## Keyboard focus & Tab order

A node is in the Tab order once it's focusable:

- `focusable=True` on any node puts it there; Enter and Space then
  activate it, firing `click`.
- A `"text_input"` or `"terminal"` is focusable from the moment it's
  created.

Drive focus and keyboard interaction directly, without a live window:

```python
window.simulate("key_down", key="tab")              # move focus forward
window.simulate("key_down", key="tab", shift=True)  # move focus backward
window.simulate("key_down", key="enter")            # activate the focused node
node.get("focused")                # True if this node currently has focus
```

Named keys are snake_case — `"tab"`, `"enter"`, `"space"`, `"escape"`,
`"backspace"`, `"delete"`, `"arrow_left"`, `"home"`, `"end"`, and so on.

## Screen readers

A real [AccessKit](https://github.com/AccessKit/accesskit) tree is built
fresh from the node tree every frame — the same structure a screen reader
walks and drives actions through: a screen-reader-triggered `Click` or
`Focus` action routes through the identical input pipeline a real mouse
click or Tab press would use, not a separate code path.

## Clipboard

A focused text input handles Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A itself, and
never copies from an `obscured` (password) input. `window.read_clipboard()`
and `window.write_clipboard(text)` reach the OS clipboard directly — both
return a "couldn't" value (`None`, `False`) rather than raising where no
clipboard service is reachable:

```python
window.simulate("key_down", key="a", ctrl=True)  # select all
window.simulate("key_down", key="c", ctrl=True)  # copy
copied = window.read_clipboard()
```
