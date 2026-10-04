"""Type stubs for `tre._core`, the compiled pyo3 extension module.

M30 Phase 0 (§5, §7, §8): pyo3 extension modules ship no type
information of their own -- an IDE or `mypy`/`pyright` sees only an
opaque `.so`, so every real class/method here is invisible to static
analysis without a hand-written `.pyi`. This file is the one, single
source of truth for that surface; keep it in exact sync with the real
`#[pyo3(signature = ...)]` attributes in `crates/engine-py/src/*.rs`
when either side changes -- a stub that drifts from the real signature
is worse than no stub, since it tells an IDE something false with full
confidence.

Scope: every class `crates/engine-py/src/lib.rs`'s own `#[pymodule]`
function registers via `m.add_class::<...>()` (`App`, `Window`, `Node`,
`Painter`, `Event`, `LoopHandle`) and `register_font`, including methods on components that predate
this stub file -- Phase 0's own explicit charge is the *current* real
API surface, not just what M30's later phases add. Each later phase
extends this file with its own new components in the same phase that
adds them, per this milestone's own stated convention -- never a
separate, deferred stub-writing pass.

A color is always a `(r, g, b, a)` byte tuple (`Color`, this file's own
local alias) -- `engine-py` exposes no dedicated Python `Color` type. A
listener registered with `on(...)` may take no arguments or one, the
`Event`; `draw`, `materialize`, `size_hint`, and `on_complete` callbacks
take what their own docs say.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any, Literal, final, overload

__all__ = [
    "App",
    "Window",
    "Node",
    "Painter",
    "Event",
    "CursorImage",
    "Gradient",
    "LoopHandle",
    "StatsHandle",
    "Shader",
    "ShaderError",
    "register_font",
    "set_system_fonts",
    "system_fonts",
]

Color = tuple[int, int, int, int]
"""An `(r, g, b, a)` byte tuple, 0-255 per channel, straight alpha."""

@final
class Event:
    """What a `node.on(...)` or `window.on(...)` listener receives when it
    declares one parameter -- never constructed directly. A field is
    `None` when the event has nothing to say about it (never fabricated).
    M100 removed the legacy `kind`, `source`, `node`, and `position`:
    `type` and `target` replace the first and third.
    """

    button: str | None
    """Pointer events: `"primary"`, `"secondary"`, `"middle"`, or (0.4.1)
    the mouse's side buttons, `"back"` and `"forward"`. Only the primary
    button's press and release make a `click`."""
    old_value: Any | None
    """`"change"`: a text input's text immediately before the edit.
    `"scroll"` (0.4.2): the scroll view's offset before the change."""
    new_value: Any | None
    """`"change"`: the text immediately after the edit. `"scroll"`: the
    offset after it."""

    # M94: the M93 target-API fields. Every field but `type` is `None`
    # unless the event has something to say about it.
    type: str
    """The event's name, e.g. `"click"`, `"pointer_down"`, `"resize"`."""
    target: Node | None
    """The node the event is about -- where it happened. `None` for a
    window event."""
    current: Node | None
    """The node whose listener is running: `target` itself, or an
    ancestor it bubbled to."""
    x: float | None
    """Pointer and wheel events: the pointer's position, local to
    `current`."""
    y: float | None
    window_x: float | None
    """Pointer and wheel events: the pointer's position in the window."""
    window_y: float | None
    delta_x: float | None
    """`wheel`: pixels, positive scrolling right."""
    delta_y: float | None
    """`wheel`: pixels, positive scrolling down."""
    key: str | None
    """`key_down`/`key_up`: a snake_case key name (`"enter"`,
    `"arrow_left"`, `"f5"`) or the character a character key produces
    (`"a"`, `"A"` with Shift)."""
    repeat: bool | None
    """`key_down`: whether this is an auto-repeat of a held key."""
    shift: bool | None
    """Pointer, wheel, key, and click events: modifier keys held."""
    ctrl: bool | None
    alt: bool | None
    meta: bool | None
    text: str | None
    """`input`: the committed text."""
    action: str | None
    """`a11y_action`: the requested action."""
    value: Any | None
    """`a11y_action` with action `"set_value"`: the requested value."""
    width: float | None
    """`resize`: the window's new width."""
    height: float | None
    dark: bool | None
    stats: dict[str, Any] | None
    """`frame`: the frame's costs, as `Window.frame_stats()`'s `last`."""
    reduced_motion: bool | None
    """`reduced_motion`: whether the OS now asks for less motion."""
    high_contrast: bool | None
    """`high_contrast`: whether the OS now asks for more contrast."""
    """`color_scheme`: whether the OS switched to dark mode."""
    maximized: bool | None
    """(0.5.0) `maximized`: whether the window is now maximized."""
    active: bool | None
    """(0.5.0) `active`: whether the window now has the OS's focus."""
    titlebar_inset: tuple[float, float] | None
    """(0.5.0) `titlebar_inset`: the new `(height, width)` the OS's window
    controls take over the content."""
    reason: str | None
    """(0.5.1) `gpu_lost`: `"unknown"` for a GPU fault, `"destroyed"` for a
    destroyed device."""
    message: str | None
    """(0.5.1) `gpu_lost`: what the driver reported (empty for a destroyed
    device); `gpu_error`: the GPU error."""
    seconds: float | None
    """(0.5.1) `gpu_stalled`: how long the submitted frame has run."""
    scale_factor: float | None
    """`scale_factor`: the window's new scale factor."""
    related_target: Node | None
    """`focus`/`unfocus`: the node on the other side of the move -- the one
    losing focus for `focus`, the one gaining it for `unfocus`. `None` when
    focus comes from, or goes to, nowhere in the window."""
    focus_visible: bool | None
    """`focus`: `True` when focus arrived by keyboard or an assistive
    technology (or programmatically, after keyboard input), `False` after
    a pointer press -- whether to show a focus indicator."""
    side: str | None
    path: str | None
    """`file_hover`/`file_drop`: the first dragged file's path."""
    paths: list[str] | None
    """`file_hover`/`file_drop`: every dragged file's path."""
    pointer_id: int | None
    """`touch_start`/`touch_move`/`touch_end`/`touch_cancel`: which finger."""
    phase: str | None
    """`pan` and `pinch`: `"began"`, `"changed"`, `"ended"` or `"cancelled"`."""
    count: int | None
    """`tap`: 1, or 2 for a double tap."""
    scale: float | None
    """`pinch`: the distance between fingers over what it was at the start."""
    scale_delta: float | None
    """`pinch`: `scale` over the previous event's -- the step to apply."""
    total_x: float | None
    """`pan`, `pinch`: how far the gesture has moved in all."""
    total_y: float | None
    velocity_x: float | None
    """`pan` ending: the speed the finger lifted at, pixels a second."""
    velocity_y: float | None
    """`dock_target`/`dock_drop`: the dock zone under the pointer
    (`"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`), or `None`
    when it's over no zone."""
    panel: Node | None
    """`dock_drop`: the panel that was dragged."""
    def stop(self) -> None:
        """Ends propagation: no listener on a further ancestor runs."""
        ...
    def cancel(self) -> None:
        """Prevents a cancellable event's default -- only the window's
        `close_requested`, which then leaves the window open. Raises
        `ValueError` for any other event."""
        ...

@final
class Node:
    """A handle to one real node in a `Window`'s tree.
    Never constructed directly -- always returned by a `Window`
    (`create`, `root`) or another `Node`. Handles compare and hash equal
    when they name the same node.
    """

    def animate(
        self,
        property: str,
        to: float | Color | Gradient | Sequence[float] | Sequence[Any] | str,
        duration_ms: int = 0,
        easing: str | tuple[float, float, float, float] | tuple[str, float] | None = None,
        on_complete: Callable[[], object] | None = None,
    ) -> None:
        """Starts (or retargets) an animation on one property, from its
        current value. Returns immediately -- never blocks.
        `duration_ms=0` snaps instantly on the next tick rather than
        easing. `easing` is `"linear"` (the default) or a cubic bezier
        `(x1, y1, x2, y2)` as CSS `cubic-bezier()` takes it, or (0.5.4) a
        spring: `"spring"` or `("spring", bounce)`, `bounce` from -1 to 1
        (exclusive) -- `duration_ms` is its period and it lasts until it settles,
        carrying the speed of a number's animation it interrupts. Animatable:
        `fill`, `stroke_color`, `stroke_width`, `opacity`, `blur`, `backdrop_blur`,
        `corner_radius` (a number or a 4-tuple), `shadows`, the transform
        parts `translate_x`/`translate_y`/`scale`/`rotation_deg`, a
        scroll view's `scroll_offset`, and a path's `data`/`trim_start`/
        `trim_end`; any other name raises `ValueError`.
        `on_complete`, when given, is called with no arguments exactly
        once, the real frame this specific animation finishes; an
        animation replaced or stopped before then never calls it.
        """
        ...
    def get_target(self, name: str) -> Any:
        """M95: the value `name`'s running animation is heading to --
        the same as `get(name)` when nothing is animating it."""
        ...
    def stop_animation(self, name: str) -> None:
        """M95: stops `name`'s running animation where it is."""
        ...
    def get(self, name: str) -> Any:
        """Reads one property: any property `set` accepts -- an animating
        one at its current, mid-animation value -- plus the read-only
        `kind` (by the name `create` takes), `focused`, `layer_placement`,
        and `layout_x`/`layout_y`/`layout_width`/`layout_height`, which
        run any pending layout first. Raises `ValueError` for an unknown
        name.
        """
        ...
    def set(self, **props: Any) -> None:
        """Sets properties atomically: every value is checked first, and a
        bad one raises `ValueError` without changing anything. Optional
        properties take `None` to clear. Every node has the layout,
        paint, transform, visibility, interaction, and accessibility
        properties; each kind adds its own (text, text input, image,
        path, canvas, scroll view, virtual list, terminal). An unknown
        name lists the valid ones. See the Properties reference.
        """
        ...
    def redraw(self) -> None:
        """M96: runs this canvas's `draw` callback now, replacing what it
        shows with what the callback draws. Raises `ValueError` for any
        other kind."""
        ...
    def focus(self) -> None:
        """M94: moves keyboard focus to this node, firing `unfocus`/`focus`.
        (0.4.2) Scroll views around it scroll to show it, as they do for any
        focus change."""
        ...
    def scroll_into_view(self) -> None:
        """0.4.2: scrolls every scroll view around this node just enough to
        show it, innermost first, at once. The `scroll_into_view`
        accessibility action does the same, after any `a11y_action`
        listener."""
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for `event`, replacing any earlier
        listener for it. Events: `pointer_enter`, `pointer_leave`,
        `pointer_down`, `pointer_move`, `pointer_up`, `click`,
        `secondary_click`, `wheel`, `key_down`, `key_up`, `input`,
        `focus`, `unfocus`, `change`, `a11y_action`, `dismiss` (a layer
        asked to close), `scroll`, and (0.5.0) `pointer_cancel` -- the press
        was taken to move or resize the window, and no `pointer_up` or
        `click` will follow it. All but `pointer_enter`/`pointer_leave`/
        `change`/`dismiss`/`scroll` bubble to ancestors
        until a listener calls `event.stop()`. `handler` receives an
        `Event`, or nothing if it takes no parameters. Raises
        `ValueError` for an unknown event.
        """
        ...
    def off(self, event: str) -> None:
        """M94: removes this node's listener for `event`, if any."""
        ...
    def capture_pointer(self) -> None:
        """M94: routes every later pointer event to this node until the
        button is released or `release_pointer()` is called."""
        ...
    def release_pointer(self) -> None:
        """M94: ends this node's pointer capture, if it holds it."""
        ...
    def __eq__(self, other: object, /) -> bool:
        """M94: equal when both handles name the same node."""
        ...
    def __hash__(self) -> int: ...
    def add_child(self, child: Node) -> None:
        """Appends `child` under this node, moving it if it's attached
        elsewhere. Raises if `child` would become its own ancestor (a
        cycle), or already belongs to a different `Window`.
        """
        ...
    def insert_child(self, index: int, child: Node) -> None:
        """M96: attaches `child` so that afterwards `children()[index] ==
        child`, moving it if it's already attached anywhere -- the
        keyed-reorder primitive. A moved node keeps its identity,
        listeners, focus, and running animations. `index` counts the
        children once `child` has left its old place; past the end raises
        `IndexError`."""
        ...
    def children(self) -> list[Node]:
        """M96: this node's children, in order."""
        ...
    def parent(self) -> Node | None:
        """M96: this node's parent, or `None` for the root or a detached
        node."""
        ...
    def remove(self) -> None:
        """M96 (R5): detaches this node from its parent. It stays alive,
        and can be attached again, while any handle to it or to anything
        under it exists; then it's freed automatically. Focus inside it
        gets `unfocus` first and isn't moved anywhere; everything else --
        scroll offsets, text and selection, running animations -- is kept."""
        ...
    def destroy(self) -> None:
        """M96: frees this node and its whole subtree now, with their
        listeners; focus inside it gets `unfocus` first. Using a handle to a
        freed node raises `ValueError`."""
        ...

@final
class Window:
    """One real OS window and the node tree painted into it. Add one or
    more to an `App`, then call `App.run()`.
    """

    def __new__(
        cls, width: int = 480, height: int = 200, title: str = "tre v2", decorations: bool = True
    ) -> Window:
        """Raises `ValueError` for a zero width or height. (0.5.0)
        `decorations=False` opens the window without the OS's title bar and
        borders, for the framework to draw its own."""
        ...
    def create(self, kind: str, **props: Any) -> Node:
        """M96: makes a detached node of `kind` -- `"box"`, `"text"`,
        `"text_input"`, `"image"`, `"path"`, `"svg"`, `"canvas"`,
        `"scroll_view"`, `"virtual_list"`, or `"terminal"` -- and applies `props`
        atomically, as `Node.set` does. Required: `text` for a text,
        `rgba`/`pixel_width`/`pixel_height` for an image, `data` for a
        path, `svg` (a `str` or `bytes` document) for an svg, `draw` for a
        canvas, `item_count`, `materialize`, and one of
        `item_extent`/`size_hint` for a virtual list, and `shell`, `cols`,
        `rows` for a terminal (which also takes `scrollback_lines`, at
        creation only). An svg also takes `svg_color` (what `currentColor`
        is) and `svg_images` (`{href: (rgba, width, height)}`, the raster
        pictures the document refers to: the framework decodes them, the
        engine never does). Attach it with `add_child`; until it's attached it
        is freed once no handle points into it. Raises `ValueError` for an
        unknown kind or a bad property, creating nothing. See the
        Properties reference for every property.
        """
        ...
    @property
    def root(self) -> Node:
        """M94: the window's root node -- a flex row with 16px padding and
        gaps, sized to the window."""
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for a window event -- `resize`,
        `color_scheme`, `scale_factor`, `close_requested` (cancellable
        with `event.cancel()`), `closed`, (M99) the docking drag's
        `dock_target`/`dock_drop`, or (0.5.0) `maximized` and `active`,
        fired when the window is maximized or restored and when it gains
        or loses focus, and `titlebar_inset`, fired when the area the OS's
        window controls take over the content changes (macOS) -- and (0.5.1)
        `gpu_lost` (`reason`, `message`: the GPU was lost, and the run ends
        right after), `gpu_error` (`message`: the GPU reported an error,
        once per distinct message, and the draw was skipped) and
        `gpu_stalled` (`seconds`: a submitted frame hasn't completed, with
        `gpu_watchdog` on) -- replacing any earlier one.
        Raises `ValueError` for an unknown event.
        """
        ...
    def off(self, event: str) -> None:
        """M94: removes the window's listener for `event`, if any."""
        ...
    def minimize(self) -> None:
        """(0.5.0) Minimizes the window -- or, before `App.run()`, opens it
        minimized."""
        ...
    def maximize(self) -> None:
        """(0.5.0) Maximizes the window -- or opens it maximized."""
        ...
    def restore(self) -> None:
        """(0.5.0) Restores a minimized or maximized window to its normal
        size; before `App.run()`, undoes `minimize()`/`maximize()`."""
        ...
    def close(self) -> None:
        """(0.5.0) Closes the window as if the user had: `close_requested`
        fires first, and a listener that cancels it keeps the window open.
        It happens on the loop's next turn, not during the call; a window
        that isn't open has nothing to close."""
        ...
    def set(
        self,
        *,
        title: str = ...,
        partial_redraw: bool = ...,
        show_damage: bool = ...,
        profile_nodes: bool = ...,
        glyph_cache: bool = ...,
        decorations: bool = ...,
        fullscreen: bool = ...,
        min_width: float = ...,
        min_height: float = ...,
        icon: tuple[bytes, int, int] | None = ...,
        resize_border: float = ...,
        system_menu: bool = ...,
        gpu_watchdog: float | None = ...,
        present_mode: str = ...,
        dpi_scaling: bool = ...,
        transparent: bool = ...,
        blur_behind: bool = ...,
        click_through: bool = ...,
    ) -> None:
        """M94: sets window properties -- `title`, and (0.4.0 M5)
        `partial_redraw`: `True` (the default) redraws only what changed
        each frame, `False` redraws the whole window every frame. A window
        whose surface can't be copied into always redraws in full, with a
        warning logged, whatever this says. (0.4.1) `show_damage`: `True`
        tints what each presented frame redrew -- its damage rects in
        magenta, a full redraw outlined in orange -- over the image, never
        the kept frame; off by default. (0.5.4) `profile_nodes`: `True` times each
        node the paint walk reaches, so `frame_stats()['profile']` can say where
        the scene-building time went; off by default (it costs two clock reads a
        node). (0.5.4) `glyph_cache`: `True` draws text from a cache of rendered glyph
        images instead of each glyph's outline: about four times cheaper to build, but
        the images are rasterised once and then placed, so edge pixels differ slightly
        from the default (the text is in the same place); off by default. (0.5.0) `decorations`: whether the
        OS draws the title bar and borders, live on an open window;
        `fullscreen`: borderless on the window's monitor; `min_width` and
        `min_height`: the smallest size the user can resize it to, 0 for
        none; `icon`: `(rgba, width, height)` -- straight-alpha RGBA8 bytes,
        `width * height * 4` of them -- or `None`, for the taskbar and
        window switcher on Windows and X11 (Wayland and macOS take the app's
        icon from its desktop file or bundle). Each applies live to an open
        window, or when `App.run()` opens it. `resize_border`: how many
        pixels along each edge of an undecorated window resize it -- a
        primary press there starts the resize and reaches no node, and the
        pointer shows a resize cursor; off while maximized or fullscreen,
        0 (the default) for none. `system_menu`: whether a secondary press
        on a `window_region="drag"` node (and, on Windows, Alt+Space on an
        undecorated window) opens the OS's window menu -- off by default,
        an opt-in on every platform for a framework that doesn't show its
        own. `gpu_watchdog`: (0.5.1) seconds, greater than 0, after which a
        submitted frame that hasn't completed fires `gpu_stalled` -- once, and
        it only reports, since stuck GPU work can't be cancelled; `None`, the
        default, is off. `present_mode`: (0.5.4) `"vsync"` (the default) paces
        frames to the display; `"low_latency"` shows the newest frame at once
        where the surface allows (`Mailbox`), at the cost of rendering as fast
        as possible while something animates. Takes effect live. `dpi_scaling`:
        (0.5.4) `True` lays the window out in logical pixels and draws it at the
        display's `scale_factor`, so an app written at 1x looks the same size
        and crisp on a HiDPI screen; `width`, `height` and every pointer
        position are then logical. `False`, the default, leaves everything in
        physical pixels, for a framework that multiplies by `scale_factor`
        itself. Takes effect live. `transparent`: (0.5.4) `True` opens the window
        see-through -- the OS gives it an alpha channel, so a root `fill` with
        alpha below 255 (or `(0, 0, 0, 0)`) shows the desktop through it, for
        rounded or shaped frameless windows; only before `App.run()` (the OS
        fixes it when the window is made), a `ValueError` after. Read
        `get("transparent_active")` once open: `False` where the surface can't
        blend with the desktop and the window stays opaque. `blur_behind`:
        (0.5.4) asks the compositor to blur what is behind the window, where it
        can (Wayland with KDE's blur protocol, macOS; ignored elsewhere); live.
        `click_through`: (0.5.4) `True` makes the whole window ignore the pointer,
        so clicks, scrolls and hover reach what is behind it; live, and a
        `ValueError` where the platform can't."""
        ...
    @overload
    def get(self, name: Literal["width", "height", "scale_factor"]) -> float: ...
    @overload
    def get(self, name: Literal["title"]) -> str: ...
    @overload
    def get(self, name: Literal["partial_redraw"]) -> bool: ...
    @overload
    def get(self, name: Literal["partial_redraw_active"]) -> bool | None: ...
    @overload
    def get(self, name: Literal["dark", "reduced_motion", "high_contrast"]) -> bool | None: ...
    @overload
    def get(
        self,
        name: Literal[
            "show_damage",
            "profile_nodes",
            "glyph_cache",
            "decorations",
            "maximized",
            "minimized",
            "active",
            "fullscreen",
            "system_menu",
            "native_controls",
        ],
    ) -> bool: ...
    @overload
    def get(self, name: Literal["titlebar_inset"]) -> tuple[float, float]: ...
    @overload
    def get(self, name: Literal["gpu_watchdog"]) -> float | None: ...
    @overload
    def get(self, name: Literal["present_mode"]) -> str: ...
    @overload
    def get(self, name: Literal["dpi_scaling", "transparent", "blur_behind", "click_through"]) -> bool: ...
    @overload
    def get(self, name: Literal["transparent_active"]) -> bool | None: ...
    @overload
    def get(self, name: Literal["min_width", "min_height", "resize_border"]) -> float: ...
    @overload
    def get(self, name: Literal["platform"]) -> str: ...
    @overload
    def get(self, name: str) -> Any:
        """M94: reads `width`, `height`, `title`, `scale_factor` (`1.0`
        until `App.run()` opens the window), (M106) `dark`: the OS's
        current appearance, `True` for dark, `False` for light, or `None`
        where the platform can't say -- Linux reads it from the XDG
        settings portal, before `App.run()` too; macOS and Windows answer
        once the window is open -- `partial_redraw`, or (0.4.0)
        `partial_redraw_active`: whether the open window really redraws
        only what changed -- the setting, and a surface that allows it --
        `None` until `App.run()` opens the window -- (0.4.1)
        `show_damage`, or (0.5.0) `decorations`, `maximized`, `minimized`,
        and `active` (whether the window has focus) -- the open window's
        own answer, or before `App.run()` what it opens as -- `fullscreen`,
        `min_width`, `min_height`, `resize_border`, and `platform`: `"wayland"`, `"x11"`,
        `"windows"`, or `"macos"` (on Linux, the open window's own answer;
        before, the backend `winit` would pick) -- and `titlebar_inset`:
        `(height, width)`, the top-left area the OS's window controls take
        over the content, non-zero only for an undecorated macOS window
        outside fullscreen, and `native_controls`: whether those controls
        are shown (so the framework hides its own) -- and `gpu_watchdog`: the
        stall watchdog's limit in seconds, or `None` when it is off."""
        ...
    def show_layer(
        self,
        node: Node,
        anchor: Node | None = None,
        placement: str = "below",
        modal: bool = False,
        dismissible: bool = True,
    ) -> None:
        """M96: shows `node` over the window's content, above every layer
        already open. With `anchor`, it's placed against that node on the
        `placement` side (`"below"`, `"above"`, `"start"`, `"end"`),
        flipped or shifted to fit at every layout; without one it sits at
        its own `x`/`y`. `modal` blocks input beneath it and moves focus
        into it; `dismissible` delivers `dismiss` to it on an outside press
        (which it consumes) or Escape (the topmost dismissible layer only).
        Events inside a layer stop at it, and it's its own Tab scope."""
        ...
    def hide_layer(self, node: Node) -> None:
        """M96: hides the layer `node`, detaching it (alive while held), and
        returns focus inside it to the node that held focus when it
        opened."""
        ...
    def measure_text(
        self,
        text: str,
        font_family: str = "Roboto",
        font_size: float = 16.0,
        font_weight: float = 400.0,
        font_style: str = "normal",
        letter_spacing: float = 0.0,
        line_height: float | None = None,
        max_width: float | None = None,
        wrap: str = "word",
        max_lines: int | None = None,
        overflow: str = "clip",
    ) -> tuple[float, float]:
        """M96: the `(width, height)` `text` takes, laid out exactly as a
        text node with these properties paints it -- wrapped within
        `max_width` when given, cut to `max_lines`, ended with "…" for
        `overflow="ellipsis"`. For sizing a widget to its content."""
        ...
    def advance(self, ms: float) -> None:
        """M96: moves this window's time forward by exactly `ms`
        milliseconds, then runs animations, their `on_complete` callbacks,
        and layout at the new time -- headless tests, where `App.run()`
        renders no frames. The first call pins the window's clock at the
        real current time; `App.run()` returns it to the real clock."""
        ...
    def frame_stats(self, reset: bool = False) -> dict[str, Any]:
        """(0.5.4) What this window's frames cost, as a dict: `frames` (drawn
        since the window opened), `skipped` (passes that found nothing to draw),
        `last` (the last frame's stages, redraw kind and node count, or `None`)
        and `recent` (the last 240 frames: `fps`, `total_ms` and `cpu_ms` as
        `mean`/`p95`/`max`, `stage_ms` per stage -- `tick`, `layout`, `prepare`,
        `acquire`, `draw`, `present` -- and how many `redraws` were `nothing`,
        `full` or `partial`). `acquire` and `present` are where the loop waits
        for the display; `cpu_ms` is a frame without them. `reset=True` clears
        the history after reading it. Nothing is recorded before `App.run()`
        opens the window. Also (0.5.4): `gpu_timing` (whether the adapter can time
        the GPU), a frame's `gpu_ms` (measured on one frame in sixteen and read back a few
        frames later, so `None` for the rest) and `recent["gpu_ms"]` (their mean), and `profile`: with
        `window.set(profile_nodes=True)`, the last frame's scene-building time by
        node kind and its slowest nodes (else `None`)."""
        ...
    def stats_handle(self) -> StatsHandle:
        """(0.5.4) A handle any thread can use to read this window's frame
        statistics (`handle.read()`) without waiting for the event loop. Make
        it on the loop's thread, then pass it on."""
        ...
    def start_trace(self, path: str) -> None:
        """(0.5.4) Writes every frame this window draws from now on to `path` as a
        Chrome / Perfetto trace (open it at ui.perfetto.dev or `chrome://tracing`):
        a slice for each frame with its stages inside, and the GPU's time on a
        second track once it is read back. Raises `OSError` if the file can't be
        created and `ValueError` if a trace is already running."""
        ...
    def stop_trace(self) -> int:
        """(0.5.4) Closes the trace `start_trace` opened and returns how many frames
        it holds (`0` if none is running). Raises `OSError` if writing failed."""
        ...
    def snapshot(
        self,
        width: float | None = None,
        height: float | None = None,
        scale: float | None = None,
        time: float = 0.0,
    ) -> tuple[bytes, int, int]:
        """(0.5.4) What the window draws, as `(rgba, width, height)`:
        straight-alpha RGBA8 bytes, `width * height * 4` of them, top row first.
        Rendered offscreen from the window's tree, so it works before
        `App.run()` and with no display. `width` and `height` are logical
        pixels and default to the window's; `scale` (default the window's,
        `1.0` before it opens or with `dpi_scaling` off) multiplies them into
        the pixels returned; `time` is the clock an animated shader sees as
        `frame.time`. Animations are drawn at their current values. Save it
        with `tre.write_png`. Raises `ValueError` for a size or scale that
        isn't greater than 0, and `RuntimeError` with no GPU or for a size
        the GPU can't render."""
        ...
    def simulate(self, event: str, node: Node | None = None, **fields: Any) -> None:
        """M94: delivers a synthetic event exactly as real input would,
        for headless tests. Pointer events (`pointer_down`, `pointer_up`,
        `pointer_move`, `pointer_enter`, `click`, `secondary_click`,
        `wheel`) aim at `node`'s center, at `x`/`y` local to `node`, or at
        window-space `x`/`y`; `button` and `delta_x`/`delta_y` where they
        apply. `pointer_leave` moves the pointer out of the window.
        `key_down`/`key_up` take `key` and `repeat`; `input` takes `text`;
        `focus`/`unfocus` take `node`; `a11y_action` takes `node`, `action`
        (`increment`, `decrement`, `expand`, `collapse`,
        `scroll_into_view`, `set_value`), and `value`. `shift`/`ctrl`/`alt`/`meta` hold
        modifiers. Window events: `resize` (`width`, `height`),
        `color_scheme` (`dark`), `scale_factor` (`scale_factor`),
        `close_requested`, `closed`, and (0.5.0) `maximized` (`maximized`)
        and `active` (`active`), which set the window's state and fire only
        when it changes, as the live window does -- and before `App.run()`,
        `maximize()`, `restore()`, and `minimize()` change it with no event,
        so simulating the state you are already in fires nothing -- and
        `titlebar_inset` (`height`, `width`), likewise -- and (0.5.1)
        `gpu_lost` (`reason`, `message`), `gpu_error` (`message`) and
        `gpu_stalled` (`seconds`), which deliver the event to its listener
        with no GPU involved. Unknown events or fields raise `ValueError`.
        """
        ...
    # -- size and clipboard ----------------------------------------------
    def resize(self, width: int, height: int) -> None:
        """Sets the window's size from code; the root's layout box
        follows. Fires no `resize` event (`simulate("resize", ...)` does).
        A live OS resize updates the same size."""
        ...
    def read_clipboard(self) -> str | None:
        """M100: the OS clipboard's text, or `None` when it holds no text or
        can't be reached (a headless environment may have no clipboard
        service; logged, never raised)."""
        ...
    def write_clipboard(self, text: str) -> bool:
        """M100: puts `text` on the OS clipboard; `False` when it can't be
        reached (logged, never raised)."""
        ...

    # -- docking ----------------------------------------------------------
    def add_dock_zone(self, side: str, container: Node, size: float) -> None:
        """`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`,
        `"center"`.
        """
        ...
    def dock_panel(self, side: str, panel: Node) -> None:
        """Docks `panel` into `side`'s zone and shows it. A panel docked in
        another zone moves, as a drag would move it: its old zone stops
        listing it and shows another of its panels."""
        ...
    def set_active_panel(self, side: str, index: int) -> None:
        """M99: shows the `index`th panel docked in `side`'s zone (was
        `set_active_tab`). Raises `ValueError` for an index out of range.
        """
        ...
    def start_panel_drag(self, panel: Node) -> None:
        """M99: starts dragging `panel`, a docked panel -- call it from the
        framework's own drag handle's `pointer_down`. While the pointer
        moves, the `dock_target` window event reports the zone under it;
        the primary button's release moves the panel there and reports
        `dock_drop`. Raises `ValueError` if `panel` isn't docked.
        """
        ...
    def undock_panel(self, panel: Node) -> None:
        """M105: takes `panel` out of docking -- out of its zone's panels,
        whose later indexes shift down, and off the tree. If it was
        shown, the zone shows the next panel, else the previous, else
        nothing. A drag of it in progress is cancelled. It stays alive
        while a handle to it exists, so `dock_panel` can dock it again,
        as after `remove()`. Raises `ValueError` if `panel` isn't docked.
        """
        ...

@final
class App:
    """Collects one or more `Window`s and drives them all together in
    one blocking call.
    """

    def __init__(self) -> None: ...
    def add_window(self, window: Window) -> None: ...
    def thread_handle(self) -> LoopHandle:
        """M87: a thread-safe handle to this `App`'s event loop. `App`
        and `Window` may only be used from the thread that created
        them; this handle may be passed to and used from any thread.
        Every handle from one `App` shares the same queue.
        """
        ...
    def run(self, max_frames: int | None = None) -> None:
        """Blocks, pumping every added window's real event loop, until
        every window closes (or, if given, `max_frames` is reached on
        each). Design Principle 1's own "one blocking call" -- returns
        `None` (rather than raising) if no real display is reachable,
        the same headless-CI-safe convention every example in this
        project relies on. Raises `RuntimeError` if a window's GPU can't
        be set up (no adapter, no device, or an unsupported surface), or
        if no window was added.
        """
        ...

@final
class StatsHandle:
    """(0.5.4) A thread-safe reader of one window's frame statistics, from
    `Window.stats_handle()`. Like `LoopHandle`, an object a background thread may
    hold."""

    def read(self, reset: bool = False) -> dict[str, Any]:
        """The dict `Window.frame_stats()` returns, except that `profile` is always
        `None` (it names `Node`s, which belong to the loop's thread). Does not
        wait for the event loop. `reset=True` clears the history after reading."""
        ...

@final
class LoopHandle:
    """M87: a thread-safe handle to an `App`'s event loop, from
    `App.thread_handle()`. The one `tre` object a background thread
    (a file watcher, a network client) may use.
    """

    def call_soon(self, callback: Callable[[], object]) -> None:
        """Queues `callback` (called with no arguments) to run on the
        `App`'s event-loop thread, and wakes the loop -- including an
        idle one. There it can touch a `Window` and its `Node`s like an
        input handler can -- e.g. rebuild a screen for hot reload.

        Safe from any thread, before, during, or after `App.run()`.
        Callbacks run in FIFO order at the top of the next frame; one
        queued outside a run waits for the next run's first frame. An
        exception is logged like one from an input handler and doesn't
        stop the loop or later callbacks. Raises `TypeError` if
        `callback` isn't callable.
        """
        ...

class ShaderError(ValueError):
    """A shader's source or names are wrong. `line`, `column` and
    `source_line` place the problem in the WGSL you gave; each is `None`
    when the problem has no position."""

    line: int | None
    column: int | None
    source_line: str | None

@final
class CursorImage:
    """(0.5.4) A pointer shape drawn from pixels, for a node's `cursor`:
    `node.set(cursor=CursorImage(rgba, 32, 32, hotspot=(4, 4)))`. `rgba` is
    straight-alpha RGBA8 bytes, `width * height * 4` of them, each side 1 to
    256 pixels; `hotspot` is the pixel that is the pointer's position. The same
    image made twice is one cursor. The image is in device pixels, not scaled
    for HiDPI, so give a larger one there. The engine decodes nothing: decode a
    PNG yourself. The OS cursor is made once `App.run()` runs its loop (see
    `ready`); the default shape shows until then. Raises `ValueError` for a bad
    size, a wrong byte count, or a hotspot outside the image."""

    def __new__(
        cls, rgba: bytes, width: int, height: int, hotspot: tuple[int, int] = ...
    ) -> CursorImage: ...
    @property
    def size(self) -> tuple[int, int]: ...
    @property
    def hotspot(self) -> tuple[int, int]: ...
    @property
    def ready(self) -> bool:
        """Whether the OS cursor has been made."""
        ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Gradient:
    """(0.5.4) A gradient to give a box (`Rect`/`Container`, `window.create("box")`),
    a path or a text node as its `fill` (a path's or box's `stroke_color` takes one
    too, and so does a canvas painter call's color, each spanning the shape's own
    bounds): `node.set(fill=Gradient.linear([...]))`. Build one with
    `Gradient.linear`, `Gradient.radial` or `Gradient.sweep`. Positions are
    relative to the box, so a gradient follows its box as layout resizes it.
    Immutable; to change a fill, set or animate `fill` to another. Setting
    `fill` to a colour replaces a gradient; `node.get("fill")` returns the
    `Gradient` while one is set. Animating `fill` between gradients of the same
    kind and number of stops interpolates their colours, positions and angles;
    animating from a flat colour fades the gradient in."""

    @staticmethod
    def linear(
        stops: Sequence[Color] | Sequence[tuple[float, Color]], angle: float = 180.0
    ) -> Gradient:
        """Along a line through the box's centre at `angle` degrees (0 up, 90 right,
        180 down), spanning the box. `stops` are colours spaced evenly, or
        `(offset, color)` pairs with offsets from 0 to 1 that don't decrease."""
        ...
    @staticmethod
    def radial(
        stops: Sequence[Color] | Sequence[tuple[float, Color]],
        center: tuple[float, float] | None = None,
        radius: float = 1.0,
    ) -> Gradient:
        """Outward from `center` (fractions of the box; `None` is `(0.5, 0.5)`, the middle). `radius` is a fraction of the
        half-diagonal, so `1.0` reaches the far corner of a centred gradient."""
        ...
    @staticmethod
    def sweep(
        stops: Sequence[Color] | Sequence[tuple[float, Color]],
        center: tuple[float, float] | None = None,
        start: float = 0.0,
    ) -> Gradient:
        """Around `center` (`None` is the middle), starting `start` degrees clockwise from up."""
        ...
    @property
    def kind(self) -> str:
        """`"linear"`, `"radial"` or `"sweep"`."""
        ...
    @property
    def stops(self) -> list[tuple[float, Color]]: ...
    @property
    def angle(self) -> float | None: ...
    @property
    def center(self) -> tuple[float, float] | None: ...
    @property
    def radius(self) -> float | None: ...
    @property
    def start(self) -> float | None: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Shader:
    """0.5.1: WGSL with one function, `fn shade(p: Pixel) -> vec4<f32>`,
    checked when it is made -- a mistake raises `ShaderError` with the line
    and column in your source, and needs no GPU.

    `uniforms` maps names to a number or a tuple of 2 to 4 numbers (an
    `f32` or a `vec2`/`vec3`/`vec4`). `inputs` maps names to `Node`s of one
    window, read in the shader as `input_<name>(uv)`: an image or video node is
    its pixels, any other node must have a shader, whose output is read. `mode` is `"fill"`
    (paints the node's box behind its content) or `"effect"` (transforms the
    node's own rendered content, read as `content(uv)`). `animated=True`
    redraws every frame. Shaders are shared: give one `Shader` to several
    nodes and `set(uniforms=...)` updates them all.
    """

    def __new__(
        cls,
        wgsl: str,
        uniforms: dict[str, float | tuple[float, ...]] | None = None,
        inputs: dict[str, Node] | None = None,
        mode: str = "fill",
        animated: bool = False,
    ) -> Shader: ...
    @staticmethod
    def filter(
        *,
        saturate: float = ...,
        brightness: float = ...,
        contrast: float = ...,
        grayscale: float = ...,
        hue_rotate: float = ...,
        invert: float = ...,
        sepia: float = ...,
    ) -> Shader:
        """(0.5.4) A ready-made effect shader that applies CSS colour filters to a
        node and its subtree: `node.set(shader=Shader.filter(grayscale=1.0))`.
        Give the filters you want as keywords; they apply in the order given,
        each as CSS defines it (1.0 is no change for `saturate`, `brightness` and
        `contrast`; `grayscale`, `invert` and `sepia` run 0 to 1; `hue_rotate` is
        in degrees). Each is a uniform of the same name, so
        `shader.set(uniforms={...})` changes them live, naming every filter the
        shader was made with. A node has one shader: this takes its place.
        Raises `TypeError` for an unknown filter and `ValueError` for no filter
        or a value out of range."""
        ...
    @property
    def wgsl(self) -> str: ...
    @property
    def mode(self) -> str: ...
    @property
    def animated(self) -> bool: ...
    @property
    def uniforms(self) -> dict[str, float | tuple[float, ...]]: ...
    @property
    def inputs(self) -> dict[str, Node]: ...
    def set(self, *, uniforms: dict[str, float | tuple[float, ...]]) -> None:
        """Replaces the uniforms, all at once: a mistake raises and changes
        nothing."""
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Painter:
    """The drawing surface a canvas's `draw` callback receives -- never
    constructed directly. M100 renamed it from `CanvasContext`.
    """

    def fill_rect(self, x: float, y: float, width: float, height: float, color: Color | Gradient) -> None: ...
    def fill_circle(self, cx: float, cy: float, radius: float, color: Color | Gradient) -> None: ...
    def stroke_path(
        self,
        points: Sequence[Sequence[float]],
        color: Color | Gradient,
        width: float,
    ) -> None:
        """Strokes a path in canvas-local coordinates. `points` starts
        with an `[x, y]` point; each later entry is a line (`[x, y]`), a
        quadratic curve (`[cx, cy, x, y]`), or a cubic curve (`[c1x, c1y,
        c2x, c2y, x, y]`). Raises `ValueError` for any other shape.
        """
        ...
    def set_hit_test_circle(self, cx: float, cy: float, radius: float) -> None:
        """Replaces this canvas's default rectangular hit test with a
        circular one.
        """
        ...
    def set_hit_test_path(self, points: Sequence[Sequence[float]], tolerance: float) -> None:
        """Replaces this canvas's default rectangular hit test with a
        stroke-shaped one -- a point hits if it's within `tolerance` of
        the path `points` describes, in `stroke_path`'s form.
        """
        ...

def register_font(data: bytes) -> list[str]:
    """M86: registers a font the caller already loaded -- a `.ttf`/
    `.otf`/`.ttc` file's raw bytes -- with every current and future
    window in this process. `tre` never reads a font file itself; the
    caller (a framework like Tesserae) owns that.

    Returns the family names the data contains: the exact strings a
    node's `font_family` must use to resolve to it.
    Registering identical bytes twice is a no-op that still returns the
    names. A window already running picks the font up on its next frame.
    (0.5.4) A registered font also fills in glyphs that a node's own
    `font_family` lacks: the registered families follow it in the node's
    family stack, so one call per script is enough.
    Raises `ValueError` if `data` holds no parseable font face.
    """
    ...

def set_system_fonts(enabled: bool) -> None:
    """(0.5.4) Lets text use the fonts installed on this machine: for the
    glyphs the bundled and registered fonts lack (CJK, Hebrew, Indic, colour
    emoji) and for family names that aren't registered. Off by default, so
    text is the same on every machine -- turn it on for an app that must show
    any language, and ship fonts with `register_font` where the same pixels
    everywhere matter. Applies to every window in this process, live: open
    windows repaint their text on their next frame."""
    ...

def system_fonts() -> bool:
    """(0.5.4) Whether system fonts are on (`set_system_fonts`)."""
    ...
