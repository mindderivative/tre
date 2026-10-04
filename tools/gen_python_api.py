#!/usr/bin/env python3
"""Generates docs/api/python.md, the Python quick reference, from the type stub.

    python tools/gen_python_api.py

Classes, constructors, methods and read-only properties come from
`python/tre/_core.pyi` (the first sentence of each docstring is the
description; `OVERRIDES` replaces any that read badly). The settable
properties of `Node` and `Window` and the events they fire are not in the stub:
they are names given to `set`, `get` and `on`, so they are listed in the tables
at the top of this file, which is where to edit them. Regenerate when the
stub changes.
"""

from __future__ import annotations

import ast
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_rust_api import summary  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
STUB = ROOT / "python" / "tre" / "_core.pyi"
OUT = ROOT / "docs" / "api" / "python.md"
PYDOC = "https://docs.python.org/3/library/"

# --- what the stub can't say ------------------------------------------------------

CLASS_DESCRIPTIONS = {
    "App": "Opens and drives one or more windows together in one blocking `run()`.",
    "LoopHandle": "The one thread-safe object: queues a callable onto a running `App`'s loop.",
    "Window": "Owns a node tree and its size and title; creates nodes, shows layers, docks panels.",
    "Node": "A handle to one node in a window's tree: properties, animation, events, structure.",
    "Event": "What a listener receives when it takes one argument; never constructed directly.",
    "Painter": "The drawing surface a canvas node's `draw` callback receives.",
    "Gradient": "A linear, radial or sweep gradient to give a box as its `fill`.",
    "CursorImage": "A pointer shape drawn from pixels, for a node's `cursor`.",
    "Shader": "WGSL that paints a node (a fill) or transforms its rendered content (an effect).",
    "ShaderError": "A shader's source or names are wrong, positioned in the WGSL you gave.",
}
GUIDE = {
    "App": "reference/app.md",
    "Window": "reference/window.md",
    "Node": "reference/node.md",
    "Event": "reference/events.md",
    "Painter": "reference/painter.md",
    "Gradient": "reference/paint.md#gradients",
    "CursorImage": "reference/node.md#cursors",
    "Shader": "reference/shader.md",
    "LoopHandle": "guide/threading.md",
}
BASES = {
    "ShaderError": [
        ("ValueError", PYDOC + "exceptions.html#ValueError"),
        ("Exception", PYDOC + "exceptions.html#Exception"),
        ("BaseException", PYDOC + "exceptions.html#BaseException"),
        ("object", PYDOC + "functions.html#object"),
    ],
}
OBJECT = ("object", PYDOC + "functions.html#object")

# (group, [(names, value, description)])
NODE_PROPERTIES = [
    ("Size and position", [
        ("`width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`", "pixels, `\"auto\"`, or `\"50%\"`", "Box size and limits."),
        ("`aspect_ratio`", "number or `None`", "Width over height."),
        ("`position`", "`\"relative\"` or `\"absolute\"`", "In the flow, or placed by `x`/`y`."),
        ("`x`, `y`", "pixels, percentage, `\"auto\"`", "Left and top offset."),
    ]),
    ("Flex layout", [
        ("`flex_direction`", "`\"horizontal\"` or `\"vertical\"`", "Main axis."),
        ("`flex_wrap`", "`\"no_wrap\"` or `\"wrap\"`", "Whether children wrap."),
        ("`align_items`, `justify_content`", "alignment names", "Cross- and main-axis alignment of children."),
        ("`gap`", "pixels or percentage", "Space between children."),
        ("`padding`, `padding_top`/`_right`/`_bottom`/`_left`", "pixels or percentage", "Inset; a node's own content draws inside it."),
        ("`flex_grow`, `flex_shrink`, `flex_basis`", "numbers / like `width`", "How a flex child grows, shrinks and starts."),
        ("`align_self`", "an `align_items` value or `None`", "Overrides the parent's alignment for this child."),
        ("`margin`, `margin_top`/`_right`/`_bottom`/`_left`", "pixels, percentage, `\"auto\"`", "Outer space."),
    ]),
    ("Grid layout", [
        ("`display`", "`\"flex\"` or `\"grid\"`", "Which layout a node's children use."),
        ("`grid_template_columns`, `grid_template_rows`", "track list", "The grid's tracks."),
        ("`grid_auto_columns`, `grid_auto_rows`, `grid_auto_flow`", "track list / flow name", "Implicit tracks and auto-placement."),
        ("`row_gap`, `column_gap`", "pixels or percentage", "Space between rows or columns."),
        ("`justify_items`, `align_content`, `justify_self`", "alignment names", "Cell and track alignment."),
        ("`grid_column`, `grid_row`", "line, `\"span 2\"`, `\"1 / 3\"`", "A grid child's placement."),
    ]),
    ("Transform, visibility and order", [
        ("`translate_x`, `translate_y`", "pixels (animatable)", "Offset from the laid-out position."),
        ("`scale`, `rotation_deg`", "number (animatable)", "Scale and clockwise rotation about the center."),
        ("`visible`", "`bool`", "`False` hides the node and its subtree."),
        ("`z_index`", "`int`", "Paint and hit order among siblings."),
        ("`clip_children`", "`bool`", "Clips children to the rounded box."),
        ("`cursor`", "cursor name or `None`", "Pointer shape over the node."),
        ("`hit_testable`", "`bool`", "Whether the node can be a pointer target."),
        ("`window_region`", "`\"drag\"`, `\"none\"`, `None`", "Makes a node a title bar that moves the window."),
    ]),
    ("Paint (every kind)", [
        ("`fill`", "`(r, g, b, a)` (animatable)", "Background; glyph color on text; a path's fill."),
        ("`stroke_color`, `stroke_width`", "color / pixels (animatable)", "Border inside the box; a path's outline."),
        ("`corner_radius`", "number or 4-tuple (animatable)", "Rounds background, border and image."),
        ("`opacity`", "`0.0`–`1.0` (animatable)", "Group opacity of the node and its subtree."),
        ("`shadows`", "list of `(color, dx, dy, blur, spread)`", "CSS `box-shadow` model."),
        ("`shader`", "`Shader` or `None`", "WGSL that paints or transforms the node."),
    ]),
    ("Text and text input", [
        ("`text`", "`str`", "The content (read-only on a terminal)."),
        ("`font_family`, `font_weight`, `font_size`", "name / 1–1000 / pixels", "Font."),
        ("`line_height`, `text_align`, `font_style`, `letter_spacing`, `wrap`, `max_lines`, `overflow`", "see the text guide", "Text layout (`text` nodes)."),
        ("`multiline`, `selection`, `show_whitespace`, `syntax_spans`, `folded_ranges`", "—", "Text input editing and decoration."),
        ("`placeholder`, `placeholder_fill`, `caret_color`, `selection_fill`, `obscured`", "—", "Text input colors and password mode."),
    ]),
    ("Image, path and canvas", [
        ("`rgba`, `pixel_width`, `pixel_height`, `fit`", "bytes / ints / `\"fill\"`, `\"contain\"`, `\"cover\"`", "Image pixels and how they fit the box."),
        ("`data`, `view_box`, `trim_start`, `trim_end`", "SVG path / tuple / `0.0`–`1.0`", "A path's shape, viewport and drawn part."),
        ("`draw`", "`draw(painter)`", "A canvas's drawing callback."),
    ]),
    ("Scrolling, lists and terminals", [
        ("`orientation`, `scroll_offset`, `scrollbar_fill`, `scrollbar_width`", "—", "A scroll view's direction, position and bar."),
        ("`item_count`, `item_extent`, `size_hint`, `materialize`", "—", "A virtual list's rows."),
        ("`cols`, `rows`, `shell`, `scrollback_lines`, `palette`", "—", "A terminal's grid, process and colors."),
    ]),
    ("Accessibility and focus", [
        ("`role`, `label`, `value`, `value_min`, `value_max`, `value_step`", "—", "What assistive technology reads."),
        ("`checked`, `selected`, `expanded`, `disabled`, `level`, `live`, `a11y_hidden`", "—", "State announced to assistive technology."),
        ("`focusable`, `tab_index`", "`bool` / `int`", "Whether and where Tab reaches the node."),
    ]),
    ("Read-only", [
        ("`kind`, `focused`, `layer_placement`", "—", "The node's kind, focus state, and a shown layer's side."),
        ("`layout_x`, `layout_y`, `layout_width`, `layout_height`", "pixels", "Where the node is, as laid out (runs pending layout)."),
    ]),
]
NODE_KINDS = [
    ("`\"box\"`", "—", "A layout container with fill, stroke, corners and shadows."),
    ("`\"text\"`", "`text`", "Shaped, non-editable text."),
    ("`\"text_input\"`", "—", "Editable text: selection, clipboard, IME, obscuring."),
    ("`\"image\"`", "`rgba`, `pixel_width`, `pixel_height`", "RGBA8 pixels; video is repeated `set(rgba=…)`."),
    ("`\"path\"`", "`data`", "A vector path."),
    ("`\"canvas\"`", "`draw`", "Immediate-mode drawing through a `Painter`."),
    ("`\"scroll_view\"`", "—", "Clips and scrolls one child."),
    ("`\"virtual_list\"`", "`item_count`, `materialize`", "Builds only the rows its viewport shows."),
    ("`\"terminal\"`", "`shell`, `cols`, `rows`", "A PTY-backed terminal emulator."),
]
NODE_EVENTS = [
    ("`pointer_enter`, `pointer_leave`", "no", "The pointer enters or leaves the node's subtree."),
    ("`pointer_down`, `pointer_move`, `pointer_up`", "yes", "A button is pressed, the pointer moves, a button is released."),
    ("`pointer_cancel`", "yes", "The OS took a press (window move, maximize, menu); no `pointer_up` follows."),
    ("`click`, `secondary_click`", "yes", "Primary or secondary press and release on the same node, or keyboard activation."),
    ("`wheel`", "yes", "A wheel or trackpad scroll."),
    ("`key_down`, `key_up`", "yes", "A key is pressed or released while the node or a descendant has focus."),
    ("`input`", "yes", "Committed text arrives for the focused text input."),
    ("`focus`, `unfocus`", "yes", "A node gains or loses keyboard focus."),
    ("`change`", "no", "A text input's text was changed by the user."),
    ("`scroll`", "no", "A scroll view's offset changed, once a frame."),
    ("`dismiss`", "no", "An outside press or Escape asked a layer to close."),
    ("`a11y_action`", "yes", "Assistive technology asked for an action (increment, expand, set value…)."),
]
WINDOW_PROPERTIES = [
    ("`width`, `height`", "get", "The client area's size (change it with `resize`)."),
    ("`title`", "set, get", "The window title, live if open."),
    ("`scale_factor`", "get", "Display scale; `1.0` until `App.run()` opens the window."),
    ("`dark`", "get", "The OS appearance: `True`, `False`, or `None` when it can't say."),
    ("`partial_redraw`, `partial_redraw_active`", "set, get", "Redraw only what changed; whether that is in effect."),
    ("`show_damage`", "set, get", "Tint what each frame redrew."),
    ("`decorations`", "set, get", "Whether the OS draws the title bar and borders."),
    ("`fullscreen`", "set, get", "Borderless fullscreen."),
    ("`min_width`, `min_height`", "set, get", "The smallest size the user can resize to."),
    ("`icon`", "set", "`(rgba, width, height)` or `None`."),
    ("`maximized`, `minimized`, `active`", "get", "Window state; use the methods to change it."),
    ("`platform`", "get", "`\"wayland\"`, `\"x11\"`, `\"windows\"` or `\"macos\"`."),
    ("`resize_border`, `system_menu`", "set, get", "Border width for undecorated windows; the OS window menu."),
    ("`titlebar_inset`, `native_controls`", "get", "macOS traffic-light area; whether the OS shows its controls."),
    ("`gpu_watchdog`", "set, get", "Seconds before a stuck frame fires `gpu_stalled`; `None` is off."),
    ("`present_mode`", "set, get", "`\"vsync\"` (default) paces frames to the display; `\"low_latency\"` shows the newest frame at once."),
]
WINDOW_EVENTS = [
    ("`resize`", "`width`, `height`", "The client area changed size."),
    ("`color_scheme`", "`dark`", "The OS switched between light and dark."),
    ("`scale_factor`", "`scale_factor`", "The window moved to a display with a different scale."),
    ("`close_requested`", "—", "The user asked to close; `event.cancel()` keeps it open."),
    ("`closed`", "—", "The window closed."),
    ("`dock_target`, `dock_drop`", "`side`, `panel`", "A docking drag moved into a zone / ended."),
    ("`maximized`, `active`", "`maximized`, `active`", "The window was maximized or restored / gained or lost focus."),
    ("`titlebar_inset`", "`titlebar_inset`", "The OS controls' area over the content changed."),
    ("`gpu_lost`", "`reason`, `message`", "The GPU was lost; the run is ending."),
    ("`gpu_error`", "`message`", "The GPU reported an error; the draw was skipped."),
    ("`gpu_stalled`", "`seconds`", "A submitted frame has not completed after `gpu_watchdog` seconds."),
]
EVENT_FIELDS = {
    "type": "The event's name; on every event.",
    "target": "The node the event happened on; every node event.",
    "current": "The node whose listener is running.",
    "x": "Pointer position, local to `current`.",
    "y": "Pointer position, local to `current`.",
    "window_x": "Pointer position in the window.",
    "window_y": "Pointer position in the window.",
    "button": "`\"primary\"`, `\"secondary\"`, `\"middle\"`, `\"back\"` or `\"forward\"`.",
    "delta_x": "Wheel scroll in pixels, positive right.",
    "delta_y": "Wheel scroll in pixels, positive down.",
    "key": "A snake_case key name, or the produced character.",
    "repeat": "Whether a key event is an auto-repeat.",
    "shift": "Modifier held during pointer, wheel, key and click events.",
    "ctrl": "Modifier held during pointer, wheel, key and click events.",
    "alt": "Modifier held during pointer, wheel, key and click events.",
    "meta": "Modifier held during pointer, wheel, key and click events.",
    "text": "The committed text of an `input` event.",
    "old_value": "The previous text (`change`) or offset (`scroll`).",
    "new_value": "The new text (`change`) or offset (`scroll`).",
    "action": "The requested action of an `a11y_action`.",
    "value": "The value of an `a11y_action` such as `set_value`.",
    "related_target": "The node on the other side of a `focus` / `unfocus` move.",
    "focus_visible": "Whether `focus` arrived by keyboard.",
    "width": "New size on `resize`.",
    "height": "New size on `resize`.",
    "dark": "New appearance on `color_scheme`.",
    "scale_factor": "New scale on `scale_factor`.",
    "maximized": "New state on `maximized`.",
    "active": "New state on `active`.",
    "titlebar_inset": "New `(height, width)` on `titlebar_inset`.",
    "reason": "`\"unknown\"` or `\"destroyed\"` on `gpu_lost`.",
    "message": "The driver's or GPU's message on `gpu_lost` and `gpu_error`.",
    "seconds": "How long the frame has run on `gpu_stalled`.",
    "side": "The dock zone under the pointer, or `None`.",
    "panel": "The dragged panel on `dock_drop`.",
}
OVERRIDES: dict[str, str] = {
    "App.add_window": "Adds a window to open when `run()` is called.",
    "App.run": "Opens every added window and runs the event loop until they all close, or `max_frames`.",
    "Window.advance": "Moves the window's clock forward by `ms`, running animations, callbacks and layout.",
    "Window.add_dock_zone": "Registers `container` as the dock zone for `side`.",
    "Window.read_clipboard": "Returns the OS clipboard's text, or `None` when there is none or it can't be reached.",
    "Window.set": "Sets window properties by name, all at once; a bad value raises and changes nothing.",
    "Window.get": "Reads a window property by name.",
    "Window.measure_text": "Returns the `(width, height)` a text node with these properties would take.",
    "Window.simulate": "Delivers a synthetic event as real input would, for tests with no display.",
    "Window.show_layer": "Shows `node` over the window's content, above every layer already open.",
    "Window.hide_layer": "Hides a layer, detaching it, and returns focus to where it was.",
    "Window.create": "Makes a detached node of a kind; attach it with `add_child`.",
    "Node.animate": "Eases a property from its current value to a target, retargeting mid-flight.",
    "Node.insert_child": "Attaches `child` at `index`, moving it if it is attached elsewhere.",
    "Painter.fill_rect": "Fills a rectangle in canvas-local coordinates.",
    "Painter.fill_circle": "Fills a circle in canvas-local coordinates.",
    "Shader.set": "Replaces the uniforms all at once; a mistake raises and changes nothing.",
    "Shader.wgsl": "The WGSL source, as given.",
    "Shader.mode": "`\"fill\"` or `\"effect\"`.",
    "Shader.animated": "Whether the node is repainted every frame.",
    "Shader.uniforms": "The uniforms and their current values, as a new dict.",
    "Shader.inputs": "The input nodes by name, as a new dict.",
    "ShaderError.line": "The 1-based line in your WGSL, or `None` when the problem has no position.",
    "ShaderError.column": "The 1-based column in that line, or `None`.",
    "ShaderError.source_line": "The text of that line, or `None`.",
    "Event.cancel": "Keeps a window open: prevents `close_requested`'s default.",
}


# --- the stub ----------------------------------------------------------------------


def sig(fn: ast.FunctionDef) -> str:
    args = re.sub(r"^(self|cls)(, )?", "", ast.unparse(fn.args))
    ret = f" -> {ast.unparse(fn.returns)}" if fn.returns else ""
    if len(args) > 90:  # keep a long parameter list readable: names only
        names = [a.arg for a in fn.args.posonlyargs + fn.args.args if a.arg not in ("self", "cls")]
        kwonly = [a.arg for a in fn.args.kwonlyargs]
        parts = names + (["*"] + kwonly if kwonly else [])
        if fn.args.kwarg:
            parts.append("**" + fn.args.kwarg.arg)
        args = ", ".join(parts)
    return f"{fn.name}({args}){ret}"


def first(doc: str | None) -> str:
    return summary(doc, limit=130)


def anchor(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def table(head: list[str], rows: list[list[str]]) -> list[str]:
    out = ["| " + " | ".join(head) + " |", "| " + " | ".join("---" for _ in head) + " |"]
    escape = lambda cell: re.sub(r"(?<!\\)\|", r"\\|", cell)  # noqa: E731
    out += ["| " + " | ".join(escape(c) for c in r) + " |" for r in rows]
    return out + [""]


def class_section(cls: ast.ClassDef, lines: list[str]) -> None:
    name = cls.name
    lines += [f"## {name} {{ #{anchor(name)} }}", ""]
    lines += [CLASS_DESCRIPTIONS.get(name) or first(ast.get_docstring(cls)), ""]
    bases = BASES.get(name, [OBJECT])
    chain = " → ".join(f"[`{n}`]({u})" for n, u in bases)
    label = "Threading guide" if name == "LoopHandle" else f"{name} reference"
    guide = f"  ·  **Details:** [{label}](../{GUIDE[name]})" if name in GUIDE else ""
    lines += [f"**Inherits:** {chain}{guide}", ""]

    props, attrs, methods, ctor = [], [], [], None
    for n in cls.body:
        if isinstance(n, ast.AnnAssign) and isinstance(n.target, ast.Name):
            attrs.append(n)
        elif isinstance(n, ast.FunctionDef):
            decos = [ast.unparse(d) for d in n.decorator_list]
            if "property" in decos:
                props.append(n)
            elif n.name in ("__new__", "__init__"):
                ctor = n
            elif n.name.startswith("__") or "overload" in decos:
                continue
            else:
                methods.append(n)
    if ctor is not None:
        args = re.sub(r"^(self|cls)(, )?", "", ast.unparse(ctor.args))
        lines += [f"**Constructor:** `{name}({args})`", ""]

    if name == "Event":
        rows = [
            [f"`{a.target.id}`", f"`{ast.unparse(a.annotation)}`", EVENT_FIELDS.get(a.target.id, "")]
            for a in attrs
        ]
        lines += ["**Properties** (each is `None` when the event has nothing to say about it):", ""]
        lines += table(["Property", "Type", "Description"], rows)
    elif attrs or props:
        lines += ["**Properties:**", ""]
        rows = [
            [f"`{a.target.id}`", f"`{ast.unparse(a.annotation)}`", OVERRIDES.get(f"{name}.{a.target.id}", "")]
            for a in attrs
        ]
        for p in props:
            rt = ast.unparse(p.returns) if p.returns else ""
            d = OVERRIDES.get(f"{name}.{p.name}") or first(ast.get_docstring(p))
            rows.append([f"`{p.name}`", f"`{rt}`", d])
        lines += table(["Property", "Type", "Description"], rows)

    if name == "ShaderError":
        for row in lines[-4:]:
            pass
    if name == "Node":
        lines += ["**Kinds** (`window.create(kind, **props)`):", ""]
        lines += table(["Kind", "Required", "What it is"], [list(r) for r in NODE_KINDS])
        lines += [
            "**Settable properties** (`node.set(...)`, `node.get(name)`; `node.animate(...)` "
            "for the animatable ones):",
            "",
        ]
        for group, rows in NODE_PROPERTIES:
            lines += [f"*{group}*", ""]
            lines += table(["Property", "Value", "Description"], [list(r) for r in rows])
    if name == "Window":
        lines += ["**Settable and readable properties** (`window.set(...)`, `window.get(name)`):", ""]
        lines += table(["Property", "Access", "Description"], [list(r) for r in WINDOW_PROPERTIES])

    if methods:
        lines += ["**Methods:**", ""]
        rows = []
        for m in sorted(methods, key=lambda m: m.name):
            d = OVERRIDES.get(f"{name}.{m.name}") or first(ast.get_docstring(m))
            rows.append([f"`{sig(m)}`".replace("|", "\\|"), d])
        lines += table(["Method", "Description"], rows)

    if name == "Node":
        lines += [
            "**Events** (`node.on(event, handler)`; a bubbling event runs the target's listener, "
            "then each ancestor's):",
            "",
        ]
        lines += table(["Event", "Bubbles", "Fires when"], [list(r) for r in NODE_EVENTS])
    elif name == "Window":
        lines += ["**Events** (`window.on(event, handler)`; a window event has no node):", ""]
        lines += table(["Event", "Payload", "Fires when"], [list(r) for r in WINDOW_EVENTS])
    elif name == "Event":
        lines += ["**Events:** this is what the events above deliver. `stop()` ends propagation; "
                  "`cancel()` keeps a window open on `close_requested`.", ""]
    else:
        lines += ["**Events:** none.", ""]


def main() -> int:
    tree = ast.parse(STUB.read_text())
    classes = {n.name: n for n in tree.body if isinstance(n, ast.ClassDef)}
    funcs = [n for n in tree.body if isinstance(n, ast.FunctionDef)]
    order = ["App", "LoopHandle", "Window", "Node", "Event", "Painter", "Gradient", "CursorImage", "Shader", "ShaderError"]
    missing = set(classes) - set(order)
    if missing:
        print(f"classes not in the page order: {sorted(missing)}", file=sys.stderr)
        return 1

    lines = [
        "# Python API",
        "",
        "<!-- Generated by tools/gen_python_api.py from python/tre/_core.pyi; do not edit by hand. -->",
        "",
        "A quick reference to everything `import tre` gives you: each class with its properties, "
        "methods and events, short descriptions, and what it inherits. The [Rust API](rust.md) is "
        "what these classes sit on. The **Details** link on a class goes to its longer page "
        "(behavior, examples, edge cases).",
        "",
        "`Node` and `Window` take most of their properties by name through `set`, `get` and "
        "(for the animatable ones) `animate`, and their events by name through `on`; those names "
        "are listed with the class. None of these classes can be subclassed.",
        "",
    ]
    lines += table(
        ["Class", "Purpose"], [[f"[`{n}`](#{anchor(n)})", CLASS_DESCRIPTIONS[n]] for n in order]
    )
    for n in order:
        class_section(classes[n], lines)
    lines += ["## Module functions and constants { #module }", ""]
    rows = [[f"`{sig(f)}`", OVERRIDES.get(f.name) or first(ast.get_docstring(f))] for f in funcs]
    rows.append(["`MONOSPACE_FONT_FAMILY`", "The bundled monospace family a terminal always shapes with."])
    rows.append(["`Color`", "A type alias: an `(r, g, b, a)` tuple of ints, 0–255, straight alpha."])
    lines += table(["Name", "Description"], rows)
    OUT.write_text("\n".join(lines).rstrip("\n") + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(lines)} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
