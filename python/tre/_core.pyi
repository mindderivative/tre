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
`View`, `CanvasContext`), including methods on components that predate
this stub file -- Phase 0's own explicit charge is the *current* real
API surface, not just what M30's later phases add. Each later phase
extends this file with its own new components in the same phase that
adds them, per this milestone's own stated convention -- never a
separate, deferred stub-writing pass.

Two real per-field typing decisions worth stating once, not per
occurrence below: an MD3 color is always a `(r, g, b, a)` byte tuple
(`Color`, this file's own local alias) matching every real
`#[pyo3(signature = (..., background, ...))]` on the Rust side, which
takes exactly that raw tuple, not a class of its own -- `engine-py`
never exposes a dedicated Python `Color` type. Every handler parameter
(`on_click=`, `on_change=`, `draw=`, `materialize=`, ...) is typed
`Callable[[], object]` -- confirmed via `crates/engine-py/src/dispatch.
rs`'s own `call_handler`, which always invokes a registered callback
with zero arguments (§16.2's own stated, deliberate scope: no `Event`
object exists yet) and discards whatever it returns.
"""

from __future__ import annotations

from typing import Callable, Sequence

Color = tuple[int, int, int, int]
"""An MD3 `(r, g, b, a)` byte tuple, 0-255 per channel."""

class Node:
    """A handle to one real node in a `Window`'s (or `View`'s) tree.
    Never constructed directly -- always returned by a `Window.add_*`
    method, or read back via `View.node`.
    """

    def animate(
        self,
        property: str,
        to: float | Color | Sequence[float],
        duration_ms: int = 0,
        on_complete: Callable[[], object] | None = None,
    ) -> None:
        """Starts (or retargets) an animation on one property. Returns
        immediately -- never blocks. `duration_ms=0` snaps instantly on
        the next tick rather than easing. `on_complete`, when given, is
        called with no arguments exactly once, the real frame this
        specific animation finishes (only fires for a `Window`-created
        node -- see this stub module's own module-level doc comment).
        """
        ...
    def get(self, property: str) -> float:
        """Reads one property's current, possibly-mid-animation value."""
        ...
    def set_on_click(self, callback: Callable[[], object]) -> None: ...
    def set_on_hover_enter(self, callback: Callable[[], object]) -> None: ...
    def set_on_hover_exit(self, callback: Callable[[], object]) -> None: ...
    def set_on_change(self, callback: Callable[[], object]) -> None:
        """Fires on a real, genuine edit -- a `Slider` drag ending, or
        `set_checked`/`set_text` being called on a `Checkbox`/`TextField`.
        """
        ...
    def set_context_menu(self, content: Node) -> None:
        """Registers `content` as this node's real right-click context
        menu -- opened via `Window.right_click(self)`. Raises if
        `content` belongs to a different `Window`.
        """
        ...
    def enable_interaction(self) -> None:
        """Opts this node into MD3 ripple/hover visual feedback."""
        ...
    def add_child(self, child: Node) -> None:
        """Attaches `child` under this node. Raises if `child` would
        become its own ancestor (a cycle), or already belongs to a
        different `Window`.
        """
        ...
    def remove(self) -> None:
        """Removes this node and its whole subtree from the tree."""
        ...
    def set_checked(self, checked: bool) -> None:
        """`Checkbox`-only -- raises `ValueError` for any other kind."""
        ...
    def set_text(self, content: str) -> None:
        """`TextField`/`Text`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def get_checked(self) -> bool:
        """`Checkbox`-only -- raises `ValueError` for any other kind."""
        ...
    def get_text(self) -> str:
        """`TextField`/`Text`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def is_focused(self) -> bool:
        """Whether this is the `Tree`'s own current keyboard-focused
        node.
        """
        ...

class Window:
    """One real OS window and the node tree painted into it. Add one or
    more to an `App`, then call `App.run()`.
    """

    def __init__(self, width: int = 480, height: int = 200, title: str = "tre v2") -> None: ...
    def set_theme(self, seed: tuple[int, int, int, int], dark: bool = False) -> None:
        """Builds a real MD3 `DynamicTheme` from `seed` and makes it
        this window's active theme, re-theming every already-created
        component live.
        """
        ...

    # -- node factories --------------------------------------------------
    def add_rect(
        self,
        background: Color,
        width: float,
        height: float,
        x: float | None = None,
        y: float | None = None,
    ) -> Node: ...
    def add_text(
        self,
        content: str,
        background: Color,
        width: float,
        height: float,
        font_family: str = "Roboto",
        font_weight: float = 400.0,
        font_size: float = 16.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A plain label -- `background` is repurposed as the glyph
        color (no visible box of its own).
        """
        ...
    def add_button(
        self,
        label: str,
        width: float,
        height: float,
        variant: str = "filled",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """MD3's five real button variants: `"elevated"`, `"filled"`,
        `"filled_tonal"`, `"outlined"`, `"text"`. Raises `ValueError`
        for any other `variant`. Returns the button's own container
        node -- `set_on_click`/`enable_interaction`/`animate` all work
        on it exactly like any other node; this does not call
        `enable_interaction()` for you.
        """
        ...
    def add_icon_button(
        self,
        icon: str,
        size: float = 40.0,
        variant: str = "standard",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """MD3's four real Icon Button variants: `"filled"`,
        `"filled_tonal"`, `"outlined"`, `"standard"`. `icon` is a
        Material Symbols icon name from this project's own curated
        set, same as `add_icon`. Raises `ValueError` for an unknown
        `variant` or `icon`. Returns the button's own container node,
        the same real contract `add_button` establishes.
        """
        ...
    def add_checkbox(
        self,
        background: Color,
        width: float,
        height: float,
        checked: bool = False,
        x: float | None = None,
        y: float | None = None,
    ) -> Node: ...
    def add_slider(
        self,
        background: Color,
        width: float,
        height: float,
        value: float = 0.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """`value` is the initial `thumb_position`, clamped `0.0..=1.0`."""
        ...
    def add_image(
        self,
        path: str,
        width: float,
        height: float,
        fit: str = "fill",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """`fit` is one of `"cover"`, `"contain"`, `"fill"`. Raises if
        `path` can't be read or decoded.
        """
        ...
    def add_icon(
        self,
        name: str,
        color: Color,
        size: float,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """`name` is a Material Symbols icon name from this project's
        own curated set.
        """
        ...
    def add_text_field(
        self,
        background: Color,
        width: float,
        height: float,
        content: str = "",
        font_family: str = "Roboto",
        font_weight: float = 400.0,
        font_size: float = 16.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node: ...
    def build_shell(
        self,
        menu_bar: Node | None = None,
        toolbar: Node | None = None,
        status_bar: Node | None = None,
    ) -> Node:
        """Builds a persistent app-shell layout (menu bar / toolbar /
        dock / status bar around one stable `content` region) and
        returns the `content` node -- navigate by replacing its
        children.
        """
        ...
    def add_splitter(
        self,
        background: Color,
        width: float,
        height: float,
        initial_position: float = 0.5,
    ) -> Node: ...
    def add_virtual_list(
        self,
        item_count: int,
        materialize: Callable[[int], Color],
        item_extent: float | None = None,
        size_hint: Callable[[int], float] | None = None,
        width: float | None = None,
        height: float | None = None,
    ) -> Node:
        """A windowed logical list of `item_count` rows -- only the
        currently-visible window is ever real `Node`s, recycled as the
        list scrolls. Exactly one of `item_extent` (every row the same
        fixed height) or `size_hint` (a per-index height callback) must
        be given.

        **Real, current limitation, not an oversight:** `materialize`
        returns a plain `(r, g, b, a)` color, not a `Node` -- every
        materialized row is always a plain colored `Rect`, full width,
        sized by that index's own extent. There is no way to compose a
        richer row (an icon plus a label, say) through this method as
        it exists today.
        """
        ...
    def add_canvas(
        self,
        width: float,
        height: float,
        draw: Callable[[CanvasContext], object],
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A custom-drawn surface -- `draw` is invoked once per
        `redraw_canvas` call, not automatically every frame.
        """
        ...

    # -- synthetic input dispatch (no real window/display needed) -------
    def begin_container_transform(
        self,
        trigger: Node,
        destination: Node,
        duration_ms: int = 300,
        content_stagger_ms: int = 90,
        on_complete: Callable[[], object] | None = None,
    ) -> None: ...
    def end_container_transform(self, trigger: Node) -> None: ...
    def click(self, node: Node) -> None:
        """Dispatches a real primary-button press+release at `node`'s
        own current center point.
        """
        ...
    def hover(self, node: Node) -> None:
        """Dispatches a real pointer-moved to `node`'s own center
        point, firing hover-enter/exit exactly like a real mouse would.
        """
        ...
    def scroll(self, node: Node, delta_y: float) -> None: ...
    def right_click(self, node: Node) -> None: ...
    def press_key(self, key: str, shift: bool = False) -> None:
        """`key` is one of `"Tab"`, `"Enter"`, `"Space"`, `"Escape"`,
        `"Backspace"`, `"Delete"`, `"ArrowLeft"`, `"ArrowRight"`,
        `"Home"`, `"End"`.
        """
        ...
    def type_text(self, text: str) -> None:
        """Dispatches `text` as real per-character keyboard input to
        whichever node currently has focus.
        """
        ...
    def copy(self) -> str | None:
        """Returns the current selection's text, or `None` if nothing
        is selected -- does not touch the system clipboard.
        """
        ...
    def cut(self) -> str | None:
        """Like `copy()`, but also deletes the selection."""
        ...
    def paste(self, text: str) -> None:
        """Inserts `text` at the current cursor position, replacing any
        selection.
        """
        ...

    # -- docking ----------------------------------------------------------
    def add_dock_zone(self, side: str, container: Node, size: float) -> None:
        """`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`,
        `"center"`.
        """
        ...
    def dock_panel(self, side: str, panel: Node) -> None: ...
    def set_active_tab(self, side: str, index: int) -> None: ...
    def set_dock_handle(self, handle: Node, panel: Node) -> None:
        """Registers `handle` as `panel`'s real drag handle -- pressing
        `handle` starts tracking a drag of `panel`.
        """
        ...
    def set_drop_zone_highlight(self, content: Node) -> None:
        """Registers `content` as this window's single drop-zone
        highlight overlay, shown/positioned automatically during a real
        panel drag.
        """
        ...
    def drag_panel_over(self, x: float, y: float) -> None: ...
    def start_panel_drag(self, handle: Node) -> bool:
        """Returns whether a drag actually started -- `handle` must
        already be registered via `set_dock_handle`.
        """
        ...
    def drop_panel_at(self, x: float, y: float) -> None: ...

    # -- virtual list / canvas plumbing -----------------------------------
    def set_virtual_list_window(self, list: Node, start: int, end: int) -> None:
        """Re-materializes `list`'s own visible window to the
        half-open range `[start, end)`.
        """
        ...
    def redraw_canvas(self, canvas: Node) -> None:
        """Invokes `canvas`'s own registered `draw` callback exactly
        once and applies its result -- never automatic, an app calls
        this whenever its own drawn content actually changed.
        """
        ...

class App:
    """Collects one or more `Window`s and drives them all together in
    one blocking call.
    """

    def __init__(self) -> None: ...
    def add_window(self, window: Window) -> None: ...
    def run(self, max_frames: int | None = None) -> None:
        """Blocks, pumping every added window's real event loop, until
        every window closes (or, if given, `max_frames` is reached on
        each). Design Principle 1's own "one blocking call" -- returns
        `None` (rather than raising) if no real display is reachable,
        the same headless-CI-safe convention every example in this
        project relies on.
        """
        ...

class View:
    """Loads a declarative `view.yaml` file -- the §16.2 MVVM surface's
    Rust-side crossing point. Pair with a Python `ViewModel` subclass
    (`tre.ViewModel`), not used directly for imperative node creation
    the way `Window` is.
    """

    def __init__(
        self,
        path: str,
        stylesheet: str | None = None,
        theme_seed: tuple[int, int, int, int] | None = None,
        dark: bool = False,
    ) -> None:
        """`stylesheet` is a path to a stylesheet YAML file (§16.3's
        cascade); `theme_seed` builds a real MD3 `DynamicTheme` the
        same way `Window.set_theme` does, resolving any `background:
        primary`-style MD3 token name in the view/stylesheet.
        """
        ...
    def node(self, widget_id: str) -> Node:
        """Looks up a declared widget by its own `id:` from the YAML."""
        ...
    def poll_reload(self) -> bool:
        """Checks whether the underlying YAML file changed on disk
        since it was last loaded and, if so, reconciles the tree in
        place (preserving `NodeId`/focus/in-flight animations where
        possible). Returns whether a reload actually happened.
        """
        ...
    def click(self, node: Node) -> None: ...
    def hover(self, node: Node) -> None: ...
    def right_click(self, node: Node) -> None: ...

class CanvasContext:
    """The imperative drawing surface handed to a `Window.add_canvas`
    `draw` callback -- never constructed directly.
    """

    def fill_rect(self, x: float, y: float, width: float, height: float, color: Color) -> None: ...
    def fill_circle(self, cx: float, cy: float, radius: float, color: Color) -> None: ...
    def stroke_path(
        self,
        points: Sequence[Sequence[float]],
        color: Color,
        width: float,
    ) -> None:
        """`points` is a list of `[x, y]` pairs -- a straight polyline
        through them, in canvas-local coordinates.
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
        the polyline through `points`.
        """
        ...

def _record_read(signal: object) -> None:
    """Internal -- called from `Signal.get()`. Only appends `signal` to
    the current binding evaluation's dependency list while one is
    genuinely in progress (`View._attach`); a no-op otherwise. Not part
    of the public API.
    """
    ...
