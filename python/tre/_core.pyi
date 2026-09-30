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

__all__ = ["App", "Window", "Node", "Painter", "Event", "LoopHandle", "register_font"]

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
    """`color_scheme`: whether the OS switched to dark mode."""
    maximized: bool | None
    """(0.5.0) `maximized`: whether the window is now maximized."""
    active: bool | None
    """(0.5.0) `active`: whether the window now has the OS's focus."""
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
        to: float | Color | Sequence[float] | Sequence[Any] | str,
        duration_ms: int = 0,
        easing: str | tuple[float, float, float, float] | None = None,
        on_complete: Callable[[], object] | None = None,
    ) -> None:
        """Starts (or retargets) an animation on one property, from its
        current value. Returns immediately -- never blocks.
        `duration_ms=0` snaps instantly on the next tick rather than
        easing. `easing` is `"linear"` (the default) or a cubic bezier
        `(x1, y1, x2, y2)` as CSS `cubic-bezier()` takes it. Animatable:
        `fill`, `stroke_color`, `stroke_width`, `opacity`,
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
        `"text_input"`, `"image"`, `"path"`, `"canvas"`, `"scroll_view"`,
        `"virtual_list"`, or `"terminal"` -- and applies `props`
        atomically, as `Node.set` does. Required: `text` for a text,
        `rgba`/`pixel_width`/`pixel_height` for an image, `data` for a
        path, `draw` for a canvas, `item_count`, `materialize`, and one of
        `item_extent`/`size_hint` for a virtual list, and `shell`, `cols`,
        `rows` for a terminal (which also takes `scrollback_lines`, at
        creation only). Attach it with `add_child`; until it's attached it
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
        or loses focus -- replacing any earlier one.
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
        decorations: bool = ...,
        fullscreen: bool = ...,
        min_width: float = ...,
        min_height: float = ...,
        icon: tuple[bytes, int, int] | None = ...,
    ) -> None:
        """M94: sets window properties -- `title`, and (0.4.0 M5)
        `partial_redraw`: `True` (the default) redraws only what changed
        each frame, `False` redraws the whole window every frame. A window
        whose surface can't be copied into always redraws in full, with a
        warning logged, whatever this says. (0.4.1) `show_damage`: `True`
        tints what each presented frame redrew -- its damage rects in
        magenta, a full redraw outlined in orange -- over the image, never
        the kept frame; off by default. (0.5.0) `decorations`: whether the
        OS draws the title bar and borders, live on an open window;
        `fullscreen`: borderless on the window's monitor; `min_width` and
        `min_height`: the smallest size the user can resize it to, 0 for
        none; `icon`: `(rgba, width, height)` -- straight-alpha RGBA8 bytes,
        `width * height * 4` of them -- or `None`, for the taskbar and
        window switcher on Windows and X11 (Wayland and macOS take the app's
        icon from its desktop file or bundle). Each applies live to an open
        window, or when `App.run()` opens it."""
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
    def get(self, name: Literal["dark"]) -> bool | None: ...
    @overload
    def get(
        self,
        name: Literal[
            "show_damage", "decorations", "maximized", "minimized", "active", "fullscreen"
        ],
    ) -> bool: ...
    @overload
    def get(self, name: Literal["min_width", "min_height"]) -> float: ...
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
        `min_width`, `min_height`, and `platform`: `"wayland"`, `"x11"`,
        `"windows"`, or `"macos"` (on Linux, the open window's own answer;
        before, the backend `winit` would pick)."""
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
        when it changes, as the live window does. Unknown events or fields raise
        `ValueError`.
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

@final
class Painter:
    """The drawing surface a canvas's `draw` callback receives -- never
    constructed directly. M100 renamed it from `CanvasContext`.
    """

    def fill_rect(self, x: float, y: float, width: float, height: float, color: Color) -> None: ...
    def fill_circle(self, cx: float, cy: float, radius: float, color: Color) -> None: ...
    def stroke_path(
        self,
        points: Sequence[Sequence[float]],
        color: Color,
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
    Raises `ValueError` if `data` holds no parseable font face.
    """
    ...
