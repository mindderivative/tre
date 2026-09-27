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
`CanvasContext`, `Event`), including methods on components that predate
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
never exposes a dedicated Python `Color` type. `draw=`/`materialize=`/
`size_hint=`/`on_complete=` stay `Callable[[], object]` (or, for
`draw`, `Callable[[CanvasContext], object]`) -- unrelated to the real
`Event` payload below, none of them route through `Node.set_on_click`/
etc.'s own `HandlerMap`. `on_click=`/`on_hover_enter=`/`on_hover_exit=`/
`on_change=` accept *either* a zero-argument callable (every
pre-existing handler in this project's own examples/tests) or a
one-argument callable receiving a real `Event` (M54 Phase 2, §8,
§16.2) -- `dispatch::wants_event_payload` arity-sniffs which shape a
given callable declared, once, at registration time, so both keep
working, never both at once for the same handler.
"""

from __future__ import annotations

from typing import Any, Callable, Sequence

Color = tuple[int, int, int, int]
"""An MD3 `(r, g, b, a)` byte tuple, 0-255 per channel."""

class Event:
    """The real payload a `Node.set_on_click`/`set_on_hover_enter`/
    `set_on_hover_exit`/`set_on_change`/`set_on_focus_enter`/`set_on_
    focus_exit` handler receives when it declares one parameter (M54
    Phase 2, M55, §8, §10, §16.2) -- never constructed directly, always
    built and handed in by the engine. Every field beyond `kind`/
    `source` is `None` when this event's own real kind has nothing to
    say about it (never fabricated): a real keyboard `Enter`/`Space`
    `"click"` has `position`/`button` both `None`; `"hover_enter"`/
    `"hover_exit"` never carry `button`/`old_value`/`new_value`;
    `"change"` never carries `position`/`button`; `"focus_enter"`/
    `"focus_exit"` never carry any of `position`/`button`/`old_value`/
    `new_value` at all (a focus change, unlike a click/hover, never has
    a real pointer position -- Tab navigation, an explicit `Window.
    focus()` call, and AccessKit's own `Action::Focus`
    are all equally real, equally position-less sources).
    """

    kind: str
    """One of `"click"`, `"hover_enter"`, `"hover_exit"`, `"change"`,
    `"focus_enter"`, `"focus_exit"`.
    """
    source: int
    """A stable, opaque integer identity for the node this event fired
    on. `node` (below) is the real, live counterpart for the case this
    alone can't serve -- a handler shared generically across several
    nodes, with no way to know which one just fired without it. A
    handler that already closed over the specific `Node` it registered
    on (the same way every pre-existing handler in this project's own
    examples/tests already does) has no real need for either.
    """
    node: Node
    """M56 (§8, §16.2): the real, live `Node` this event fired on --
    additive alongside `source`, not a replacement. Calling back on it
    immediately from inside the handler (`event.node.animate(...)`,
    `.get(...)`, `.set_on_click(...)`, etc.) is safe -- this is the
    exact same live handle a `Node.set_on_click`/etc. registration
    already holds, built and handed in fresh for every dispatch, never
    a stale snapshot.
    """
    position: tuple[float, float] | None
    """The real pointer position for a pointer-driven `"click"`, or a
    `"hover_enter"`/`"hover_exit"`'s own real `PointerMoved` position
    -- both hover events from the same real move share one position
    (wherever the pointer now is), not each node's own former center.
    `None` for a keyboard-triggered `"click"` or any `"change"`.
    """
    button: str | None
    """`"primary"`, `"secondary"`, or `"middle"` for a real
    pointer-driven `"click"` -- `None` for a keyboard-triggered
    `"click"` or any other kind.
    """
    old_value: Any | None
    """`"change"` only: the value immediately before this edit -- a
    `bool` (`Checkbox`/`RadioButton`/`Switch`), a `str` (`TextField`),
    a `float` (`Slider`), or a `(hour, minute)` int tuple
    (`TimePickerDial`). `None` for every other kind.
    """
    new_value: Any | None
    """`"change"` only: the value immediately after this edit, same
    real per-`NodeKind` type as `old_value`. `None` for every other
    kind.
    """

    # M94: the M93 target-API fields, set for `node.on(...)`/`window.on(...)`
    # listener events. Legacy `set_on_*` events set `type` (equal to `kind`)
    # and `target` (equal to `node`) too. Every other field is `None` unless
    # the event has something to say about it.
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

class Node:
    """A handle to one real node in a `Window`'s tree.
    Never constructed directly -- always returned by a `Window`
    (`create`, `root`, or an `add_*` factory) or another `Node`.
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
        easing. M95: `easing` is `"linear"` (the default) or a cubic
        bezier `(x1, y1, x2, y2)` as CSS `cubic-bezier()` takes it; the
        M93 paint names -- `fill`, `stroke_color`, `stroke_width`,
        `opacity`, `corner_radius` (a number or a 4-tuple), `shadows`,
        and on a path `data`/`trim_start`/`trim_end` -- animate here.
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
    def get(self, property: str) -> Any:
        """Reads one property: an animatable number's current, possibly
        mid-animation value (a `float`), or -- M94 -- any property `set`
        accepts, plus `focused`. On a built-in slider or progress
        indicator, `value` stays that widget's numeric value. M97:
        `kind` is the node's kind, by the name `create` takes.
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
        """M94: moves keyboard focus to this node, firing `unfocus`/`focus`."""
        ...
    def set_layout(
        self,
        width: float | None = None,
        height: float | None = None,
        padding: float | None = None,
        padding_top: float | None = None,
        padding_right: float | None = None,
        padding_bottom: float | None = None,
        padding_left: float | None = None,
        margin: float | None = None,
        margin_top: float | None = None,
        margin_right: float | None = None,
        margin_bottom: float | None = None,
        margin_left: float | None = None,
        gap: float | None = None,
        flex_grow: float | None = None,
        flex_shrink: float | None = None,
        flex_basis: float | None = None,
        align_items: str | None = None,
        justify_content: str | None = None,
        flex_direction: str | None = None,
    ) -> None:
        """M48: general live layout mutation. Only the fields actually
        passed are changed -- every omitted field keeps its current
        value. Applies immediately (not eased): layout fields aren't
        animatable the way paint properties are.

        M71: `flex_direction` (`"horizontal"`/`"vertical"`, not taffy's
        own `"row"`/`"column"`) sets a node's own main axis -- the one
        layout property every other `add_*` factory previously gave no
        way to change after construction at all.

        M59: widened with per-side padding/margin (each independently
        optional, layered *on top of* the uniform `padding=`/`margin=`
        when both are given -- the per-side kwarg always wins for that
        one side), `flex_grow`/`flex_shrink`/`flex_basis`, and `align_
        items`/`justify_content`. `align_items` accepts `"start"`,
        `"end"`, `"flex_start"`, `"flex_end"`, `"center"`, `"baseline"`,
        `"stretch"`; `justify_content` accepts those same seven plus
        `"space_between"`, `"space_around"`, `"space_evenly"` -- raises
        `ValueError` for anything else, matching `press_key`'s own
        "unknown key" contract.
        """
        ...
    def set_on_click(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """`callback` may take zero arguments, or one -- a real `Event`
        (M54 Phase 2), with `event.position`/`event.button` set for a
        real pointer-driven click, both `None` for a keyboard `Enter`/
        `Space` activation.
        """
        ...
    def set_on_hover_enter(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """`callback` may take zero arguments, or one -- a real `Event`
        (M54 Phase 2) with `event.position` set to the real pointer
        position that triggered this transition.
        """
        ...
    def set_on_hover_exit(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """`set_on_hover_enter`'s own real counterpart, same `Event`
        contract.
        """
        ...
    def set_on_change(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """Fires on a real, genuine edit -- a `TextField`'s text changing,
        by typing or by `set_text`.
        `callback` may take zero arguments, or one -- a real `Event`
        (M54 Phase 2) with `event.old_value`/`event.new_value` set to
        this edit's own real before/after values.
        """
        ...
    def set_on_focus_enter(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """Fires when this node becomes the keyboard-focused node --
        real click-to-focus, Tab/Shift-Tab navigation, a real `Window.
        focus()` call, or a real AccessKit `Action::
        Focus` request (M55). `callback` may take zero arguments, or
        one -- a real `Event` with `position`/`button`/`old_value`/
        `new_value` all `None` (a focus change carries none of those).
        """
        ...
    def set_on_focus_exit(
        self, callback: Callable[[], object] | Callable[[Event], object]
    ) -> None:
        """`set_on_focus_enter`'s own real counterpart, same `Event`
        contract.
        """
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for `event`, replacing any earlier
        listener for it. Events: `pointer_enter`, `pointer_leave`,
        `pointer_down`, `pointer_move`, `pointer_up`, `click`,
        `secondary_click`, `wheel`, `key_down`, `key_up`, `input`,
        `focus`, `unfocus`, `change`, `a11y_action`. All but
        `pointer_enter`/`pointer_leave`/`change` bubble to ancestors
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
    def __eq__(self, other: object) -> bool:
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
    def set_text(self, content: str) -> None:
        """`TextField`/`Text`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def push_frame(self, rgba: bytes, width: int, height: int) -> None:
        """Replaces this node's currently-displayed pixel content with
        `rgba` -- straight-alpha 8-bit RGBA pixels, `len(rgba)` must be
        exactly `width * height * 4`. Real, live video update for a
        `Window.add_video`-created node (or any `Image` node); raises
        `ValueError` for any other kind, or if `rgba`'s own length
        doesn't match `width`/`height`.
        """
        ...
    def get_text(self) -> str:
        """`TextField`/`Text`/`Terminal`-only -- raises `ValueError` for
        any other kind. For a `Terminal`, returns its whole cell grid
        as plain text (each row's own trailing whitespace trimmed,
        rows joined by `"\\n"`), not just one line.
        """
        ...
    def is_focused(self) -> bool:
        """Whether this is the `Tree`'s own current keyboard-focused
        node.
        """
        ...
    def set_syntax_spans(
        self, spans: Sequence[tuple[int, int, tuple[int, int, int, int]]]
    ) -> None:
        """`TextField`-only: sets real syntax coloring for byte ranges
        of `get_text()`'s own content. Each `(start, end, (r, g, b,
        a))` names a range and the color to paint it. Replaces the
        whole list every call -- re-tokenize and call again after each
        real edit; `engine-core` performs no tokenization of its own
        (app-side only). Raises `ValueError` for any other kind.
        """
        ...
    def set_folded_ranges(self, ranges: Sequence[tuple[int, int]]) -> None:
        """`TextField`-only: collapses each `(start, end)` real byte
        range of `get_text()`'s own content into one visible "⋯"
        marker at paint time. Replaces the whole list every call.
        `Home`/`End`/`ArrowUp`/`ArrowDown` are fold-aware (M38 Phase
        3): a real cursor move that would land strictly inside a
        folded range snaps forward to right after that fold's own
        real marker instead, matching the *displayed* caret's own
        identical clamp. Raises `ValueError` for any other kind.
        """
        ...
    def set_clip_children(self, clip: bool) -> None:
        """Opts this node into clipping its own real children to its
        own box -- the real, general form of the clip `VirtualList`/
        `Carousel` already have built in, closing "no `NodeKind`
        besides `VirtualList` clips its own children today". Universal,
        not kind-specific, unlike `set_syntax_spans`/`set_folded_
        ranges` above. `False` (every node, by default) is a true
        no-op. Real, honest v1 limit: clipping only -- this does not
        give a container a real scroll offset or wheel-input wiring of
        its own; content past the box is genuinely hidden, not
        scrollable into view.
        """
        ...
    def set_terminal_selection(
        self, start_row: int, start_col: int, end_row: int, end_col: int
    ) -> None:
        """`Terminal`-only: seeds a real cell-range selection directly,
        without a live mouse drag -- a real linear (reading-order)
        selection, the same convention every terminal emulator uses.
        `start == end` (both coordinates equal) reads as no selection
        at all. Raises `ValueError` for any other kind.
        """
        ...

class Window:
    """One real OS window and the node tree painted into it. Add one or
    more to an `App`, then call `App.run()`.
    """

    def __init__(self, width: int = 480, height: int = 200, title: str = "tre v2") -> None: ...
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
        """M94: the window's root node (the shown one, after `show_view`)."""
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for a window event -- `resize`,
        `color_scheme`, `scale_factor`, `close_requested` (cancellable
        with `event.cancel()`), `closed`, or (M99) the docking drag's
        `dock_target`/`dock_drop` -- replacing any earlier one.
        Raises `ValueError` for an unknown event.
        """
        ...
    def off(self, event: str) -> None:
        """M94: removes the window's listener for `event`, if any."""
        ...
    def set(self, *, title: str = ...) -> None:
        """M94: sets window properties -- today only `title`."""
        ...
    def get(self, name: str) -> Any:
        """M94: reads `width`, `height`, `title`, or `scale_factor`
        (`1.0` until `App.run()` opens the window)."""
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
        `close_requested`, `closed`. Unknown events or fields raise
        `ValueError`.
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
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node: ...
    def add_text(
        self,
        content: str,
        foreground: Color,
        width: float,
        height: float,
        font_family: str | None = None,
        font_weight: float | None = None,
        font_size: float | None = None,
        line_height: float | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A plain label -- `foreground` is its text color (a label has
        no fill of its own; M90 renamed this from `background`). Unset
        font fields fall back to `"Roboto"`/`400.0`/`16.0`/the font's own
        natural line height. M99 removed the MD3 `typography_role`.
        """
        ...
    def add_image_from_bytes(
        self,
        rgba: bytes,
        pixel_width: int,
        pixel_height: int,
        width: float,
        height: float,
        fit: str = "fill",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """An `Image` node from already-decoded straight-alpha RGBA8
        pixels (`pixel_width * pixel_height * 4` bytes exactly, or a
        clear `ValueError`) -- the caller owns decoding (M99 removed
        `add_image(path)`). `width`/`height` are the node's own fixed
        display box; `pixel_width`/`pixel_height` describe
        `rgba` itself, and `fit` resolves any mismatch between the two.
        The node this returns is a real, ordinary `Image` node --
        `Node.push_frame` keeps working on it afterward, identically to
        one built via `add_video`.
        """
        ...
    def add_video(
        self,
        width: float,
        height: float,
        fit: str = "fill",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real `Image` node meant to be updated live via
        `Node.push_frame` -- a frame *sink*, not a decoder. The
        application decodes video however it likes (PyAV, OpenCV, a
        camera driver, frames generated on the fly) and pushes each
        decoded frame; this method only creates the display surface,
        initialized as fully transparent until the first real
        `push_frame` call. `fit` is `"cover"`, `"contain"`, or `"fill"`,
        as for `add_image_from_bytes`.
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
        multiline: bool = False,
        show_whitespace: bool = False,
    ) -> Node:
        """M71: `multiline`/`show_whitespace` mirror `add_code_editor`'s
        own two real `TextFieldState` fields -- both default `False`,
        the pre-existing behavior for every caller that doesn't pass
        them.
        """
        ...
    def add_code_editor(
        self,
        content: str,
        background: Color,
        width: float,
        height: float,
        font_weight: float = 400.0,
        font_size: float = 14.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real, genuinely multiline `TextField` (`Enter` inserts a
        newline, `Home`/`End` operate on the current line, `ArrowUp`/
        `ArrowDown` navigate by line preserving column, `Tab` inserts
        real indentation). Composes with `Node.set_syntax_spans`,
        `Node.set_folded_ranges`, and an app-composed sibling gutter
        (M31 Phases 1-5) for real syntax highlighting, code folding,
        and line numbers. Always shapes with the real bundled
        monospace face (M32 Phase 1, `"Hack Nerd Font Mono"`) --
        `font_family` isn't exposed here, since any other name would
        defeat a genuinely monospace editor's own point. Content
        taller or a line wider than the box both scroll and clip
        automatically, and the caret auto-scrolls into view (both
        vertically and horizontally) as it moves -- no app-side
        wiring needed.
        """
        ...
    def add_terminal(
        self,
        shell: str,
        cols: int,
        rows: int,
        background: Color,
        font_size: float = 14.0,
        scrollback_lines: int = 1000,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real, live pseudo-terminal -- spawns `shell` on a real PTY
        and parses its real byte stream with a real VT100 parser.
        `width`/`height` are computed from `cols`/`rows`, not given
        directly. Read its current contents back via `Node.get_text()`
        (each row's own trailing whitespace trimmed, rows joined by
        `"\\n"`, reflecting whatever is currently scrolled into view);
        a focused terminal receives real keystrokes through `Window.
        press_key`/`type_text`/`press_ctrl`/a real platform keyboard
        event alike. `scrollback_lines` real lines of history are
        retained (`0` for none); `Window.scroll` on this node, or a
        real mouse wheel over it, moves the viewport into it. A real
        mouse drag (or `Node.set_terminal_selection`) selects real
        text; `Window.copy_terminal_selection`/a genuine Ctrl+Shift+C
        reads/copies it. Real, honestly-scoped v1: no real resize-with-
        window, and POSIX only. Raises `OSError` if `shell` can't be
        spawned on a real PTY.
        """
        ...
    def get_monospace_cell_size(self, font_size: float) -> tuple[float, float]:
        """The real `(width, height)` cell size of the bundled
        monospace face (`"Hack Nerd Font Mono"`) at `font_size` --
        the exact real metrics `add_terminal`/`add_code_editor`
        themselves size against, for app-level layout code (a gutter's
        own per-line click target, a fold toggle) that needs to line up
        with the identical real grid, instead of an approximation.
        """
        ...
    def resize_terminal(self, node: Node, cols: int, rows: int) -> None:
        """Resizes a real, live `Terminal`'s own grid -- the real
        kernel-level PTY (so the shell's own `SIGWINCH`-driven reflow
        sees the real new size) and the underlying VT100 parser's own
        screen buffer, then this node's own real box (matching the
        exact real cell-size formula `add_terminal` itself used at
        construction). Real, honest note: resizing *after* the shell
        has already drawn a full prompt at the old width can leave that
        shell's own redraw looking corrupted for some shells -- a real,
        inherent PTY/shell-level phenomenon, not something this method
        can fix by resizing differently. Raises `ValueError` if `node`
        isn't a real `Terminal` this `Window` created.
        """
        ...
    def add_scroll_view(
        self,
        width: float,
        height: float,
        orientation: str = "vertical",
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real, general scrollable viewport over exactly one child.
        Returns an empty container -- compose your own real content in
        via `Node.add_child` (it needs a real, explicit size on the
        scroll axis matching its own true content extent, same as
        every other `add_*` factory's own children). `orientation`
        is the scroll axis: `"vertical"` (the default) or `"horizontal"`
        -- never both at once. Real wheel scrolling and `Window.scroll`
        both already work with no further setup, and so does a real
        mouse drag on the scrollbar thumb the engine now paints and
        drags automatically whenever there's real overflow content --
        no further setup needed there either.
        """
        ...
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
    def focus(self, node: Node) -> None:
        """Directly focuses `node` (M55) -- no real `InputEvent`
        represents "focus this specific node," so this calls the same
        real mechanism click-to-focus/Tab navigation/AccessKit's own
        `Action::Focus` all use, firing a registered `FocusEnter`/
        `FocusExit` handler exactly like any of those would.
        """
        ...
    def resize(self, width: int, height: int) -> None:
        """A direct, programmatic "resize this window" entry point --
        the same no-live-window-needed pattern `click`/`hover` use.
        Resizes the root node's own real layout box (a fresh
        `compute_layout` reflects the new size) and updates `self.
        width`/`height`, which every other synthetic method here and
        every interactive `add_*` factory method reads for its own
        layout. A real, live OS window resize also updates the same
        real, shared `width`/`height` -- an interactive `add_*` call
        made from a live click handler after a real resize sizes
        against the window's real current dimensions, not its
        construction-time ones.
        """
        ...
    def scroll(self, node: Node, delta_y: float, delta_x: float = 0.0) -> None:
        """Dispatches a real wheel scroll at `node`'s own center point
        -- bubbles up to the nearest `VirtualList`/`Carousel`/
        `ScrollView` ancestor, the same real "scroll bubbling" behavior
        a genuine mouse wheel already has. If `node` is itself a real
        `Terminal`, this moves its own real viewport into scrollback
        instead (positive `delta_y` reveals older history, matching a
        real wheel-up notch; `delta_x` is ignored there). `delta_x` is
        for a real horizontal `ScrollView` -- ignored by every other
        real scrollable kind, which stay vertical-only.
        """
        ...
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
    def press_ctrl(self, letter: str) -> bool:
        """Sends a real Ctrl+`<letter>` control byte to the currently
        focused `Terminal` -- `letter="c"` sends the real SIGINT byte
        (`0x03`), the same as pressing Ctrl+C in any real terminal
        emulator. `letter` must be exactly one ASCII letter (case-
        insensitive), or raises `ValueError`. Returns whether a
        `Terminal` was actually focused to receive it -- `False`
        touches nothing else (never a `TextField`'s own clipboard
        state; use `copy`/`cut`/`paste` below for that).
        """
        ...
    def copy(self) -> str | None:
        """**Hermetic** -- never touches the real system clipboard.
        Returns the current selection's text, or `None` if nothing is
        selected. A real, no-live-window-needed synthetic entry point
        for testing (M17 Phase 1); use `copy_to_system_clipboard()`
        (M53) for the real thing -- e.g. wiring a context-menu "Copy"
        item.
        """
        ...
    def cut(self) -> str | None:
        """**Hermetic** -- never touches the real system clipboard.
        Like `copy()`, but also deletes the selection. Use `cut_to_
        system_clipboard()` (M53) for the real thing.
        """
        ...
    def paste(self, text: str) -> None:
        """**Hermetic** -- takes `text` directly rather than reading the
        real system clipboard. Inserts it at the current cursor
        position, replacing any selection. Use `paste_from_system_
        clipboard()` (M53) to actually read the real clipboard first.
        """
        ...
    def copy_terminal_selection(self) -> str | None:
        """`copy()`'s own real `Terminal` sibling -- returns the
        currently focused terminal's own selected text (seed one with
        `Node.set_terminal_selection` or a real mouse drag), or `None`
        if nothing is focused, the focused node isn't a `Terminal`, or
        its selection is empty. Does not touch the system clipboard --
        the real live path is a genuine Ctrl+Shift+C.
        """
        ...
    def select_all(self) -> bool:
        """M53: selects the currently-focused `TextField`'s own entire
        content -- the real `Ctrl+A` convention (cursor lands at the
        end, not the start). Returns whether a real `TextField` was
        actually focused to receive it; a true no-op otherwise.
        """
        ...
    def copy_to_system_clipboard(self) -> bool:
        """M53: `copy()`'s own **real**, non-hermetic sibling -- writes
        the currently-focused `TextField`'s own real selection to the
        real OS clipboard, the same real path a genuine Ctrl+C uses.
        Returns `True` only on a genuine, complete write -- `False`
        both when nothing is focused/selected and when the real OS
        clipboard is unreachable (a real, possible condition in some
        headless/sandboxed environments -- logged, never raised). This
        is the real method a context-menu "Copy" item's own `on_click`
        callback should call.
        """
        ...
    def cut_to_system_clipboard(self) -> bool:
        """`copy_to_system_clipboard()`'s own real Cut sibling --
        genuinely removes the currently-focused field's own selection
        and fires a real `Change` handler, but only once the real
        clipboard write actually succeeds (a failed write never
        destroys the selection with no way to recover it).
        """
        ...
    def paste_from_system_clipboard(self) -> bool:
        """`copy_to_system_clipboard()`'s own real Paste sibling --
        reads the real OS clipboard and inserts it into whichever field
        is currently focused, the same real path a genuine Ctrl+V uses.
        Returns whether the real clipboard *read* succeeded, not
        whether a field happened to be focused to receive it -- a
        genuine OS read can fail on its own, independent of this
        `Window`'s own tree state. **Real, environment-dependent limit,
        not silently glossed over:** on some sandboxed setups (no real
        clipboard manager installed), the OS clipboard's own content
        may only be served while the *writing* process's own clipboard
        handle is still alive -- a `paste_from_system_clipboard()` call
        made after that handle has already gone out of scope can
        legitimately return `False` even though the preceding
        `copy_to_system_clipboard()` genuinely succeeded.
        """
        ...

    # -- docking ----------------------------------------------------------
    def add_dock_zone(self, side: str, container: Node, size: float) -> None:
        """`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`,
        `"center"`.
        """
        ...
    def dock_panel(self, side: str, panel: Node) -> None:
        """Docks `panel` into `side`'s zone and shows it."""
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
        project relies on.
        """
        ...

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

def register_font(data: bytes) -> list[str]:
    """M86: registers a font the caller already loaded -- a `.ttf`/
    `.otf`/`.ttc` file's raw bytes -- with every current and future
    window in this process. `tre` never reads a font file itself; the
    caller (a framework like Tesserae) owns that.

    Returns the family names the data contains: the exact strings a
    theme's `typography:` `font_family` must use to resolve to it.
    Registering identical bytes twice is a no-op that still returns the
    names. A window already running picks the font up on its next frame.
    Raises `ValueError` if `data` holds no parseable font face.
    """
    ...
