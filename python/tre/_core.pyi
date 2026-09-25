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
`View`, `CanvasContext`, `Event`), including methods on components that predate
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
    focus()`/`View.focus()` call, and AccessKit's own `Action::Focus`
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
    """`focus`/`blur`: the node on the other side of the move -- the one
    losing focus for `focus`, the one gaining it for `blur`. `None` when
    focus comes from, or goes to, nowhere in the window."""
    focus_visible: bool | None
    """`focus`: `True` when focus arrived by keyboard or an assistive
    technology (or programmatically, after keyboard input), `False` after
    a pointer press -- whether to show a focus indicator."""
    def stop(self) -> None:
        """Ends propagation: no listener on a further ancestor runs."""
        ...
    def cancel(self) -> None:
        """Prevents a cancellable event's default -- only the window's
        `close_requested`, which then leaves the window open. Raises
        `ValueError` for any other event."""
        ...

class Node:
    """A handle to one real node in a `Window`'s (or `View`'s) tree.
    Never constructed directly -- always returned by a `Window.add_*`
    method, or read back via `View.node`.
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
        indicator, `value` stays that widget's numeric value.
        """
        ...
    def set(self, **props: Any) -> None:
        """M94: sets properties atomically -- every value is checked
        first, and a bad one raises `ValueError` without changing
        anything. Today: `role`, `label`, `value` (str or number),
        `value_min`, `value_max`, `value_step`, `checked`, `selected`,
        `expanded`, `disabled`, `level`, `live` (`"off"`, `"polite"`,
        `"assertive"`), `a11y_hidden`, `focusable`, `tab_index`,
        `cursor`, `hit_testable`, `width`/`height` (a number, `"auto"`,
        or `"50%"`); M95 paint on every node: `fill`, `stroke_color`,
        `stroke_width`, `opacity` (group opacity), `corner_radius` (a
        number or `(top_left, top_right, bottom_right, bottom_left)`),
        `shadows` (a list of `(color, offset_x, offset_y, blur,
        spread)`); on a path `data` (SVG path data), `view_box`
        (`(min_x, min_y, width, height)`), `trim_start`/`trim_end`; on a
        text input `placeholder`, `placeholder_fill`, `caret_color`,
        `selection_fill`, `obscured`; on a scroll view `scrollbar_fill`,
        `scrollbar_width`; on a terminal `palette` (a dict of any of
        `ansi` -- 16 colors -- `foreground`, `background`, `cursor`,
        `selection`). Optional ones take `None` to clear.
        """
        ...
    def focus(self) -> None:
        """M94: moves keyboard focus to this node, firing `blur`/`focus`."""
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
        """Fires on a real, genuine edit -- a `Slider` drag ending, or
        `set_checked`/`set_text` being called on a `Checkbox`/`TextField`.
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
        focus()`/`View.focus()` call, or a real AccessKit `Action::
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
    def set_context_menu(self, content: Node) -> None:
        """Registers `content` as this node's real right-click context
        menu -- opened via `Window.right_click(self)`. Raises if
        `content` belongs to a different `Window`.
        """
        ...
    def enable_interaction(self) -> None:
        """Opts this node into MD3 ripple/hover visual feedback."""
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for `event`, replacing any earlier
        listener for it. Events: `pointer_enter`, `pointer_leave`,
        `pointer_down`, `pointer_move`, `pointer_up`, `click`,
        `secondary_click`, `wheel`, `key_down`, `key_up`, `input`,
        `focus`, `blur`, `change`, `a11y_action`. All but
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
        under it exists; then it's freed automatically."""
        ...
    def destroy(self) -> None:
        """M96: frees this node and its whole subtree now, with their
        listeners. Using a handle to a freed node raises `ValueError`."""
        ...
    def set_checked(self, checked: bool) -> None:
        """`Checkbox`-only -- raises `ValueError` for any other kind."""
        ...
    def set_selected(self, selected: bool) -> None:
        """`RadioButton`/`Switch`-only (M90: `Switch` joined, replacing
        `set_on`) -- raises `ValueError` for any other kind.
        """
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
    def get_checked(self) -> bool:
        """`Checkbox`-only -- raises `ValueError` for any other kind."""
        ...
    def get_selected(self) -> bool:
        """`RadioButton`/`Switch`-only (M90: `Switch` joined, replacing
        `get_on`) -- raises `ValueError` for any other kind.
        """
        ...
    def get_text(self) -> str:
        """`TextField`/`Text`/`Terminal`-only -- raises `ValueError` for
        any other kind. For a `Terminal`, returns its whole cell grid
        as plain text (each row's own trailing whitespace trimmed,
        rows joined by `"\\n"`), not just one line.
        """
        ...
    def set_carousel_index(self, index: int) -> None:
        """`Carousel`-only -- raises `ValueError` for any other kind.
        Moves to `index` (clamped to the real child count), starting a
        real eased snap toward it. A no-op if already there.
        """
        ...
    def get_carousel_index(self) -> int:
        """`Carousel`-only -- raises `ValueError` for any other kind.
        The item the carousel is *settling on* -- its real destination,
        not necessarily where it's currently drawn mid-snap (see
        `get_carousel_position`).
        """
        ...
    def get_carousel_position(self) -> float:
        """`Carousel`-only -- raises `ValueError` for any other kind.
        The real, currently-animating strip position: an integer at
        rest, fractional while a snap is still travelling.
        """
        ...
    def set_carousel_scroll(self, value: float) -> None:
        """`Carousel`-only (meaningful for `layout="uncontained"`) --
        raises `ValueError` for any other kind. Sets the real free
        pixel scroll offset, clamped to `[0, max_scroll]`.
        """
        ...
    def get_carousel_scroll(self) -> float:
        """`Carousel`-only -- raises `ValueError` for any other kind."""
        ...
    def set_time_picker_dial_time(self, hour: int, minute: int) -> None:
        """`TimePickerDial`-only -- raises `ValueError` for any other
        kind. Moves both hands directly (an instant move, not an eased
        one -- a clock hand snapping to wherever it's set IS the real
        behavior here). `hour` clamps to `0..=23`, `minute` to
        `0..=59`.
        """
        ...
    def get_time_picker_dial_time(self) -> tuple[int, int]:
        """`TimePickerDial`-only -- raises `ValueError` for any other
        kind. Returns `(hour, minute)`.
        """
        ...
    def set_time_picker_dial_mode(self, mode: str) -> None:
        """`TimePickerDial`-only -- raises `ValueError` for any other
        kind. `mode` is `"hour"` or `"minute"`: which hand a real drag
        on this dial moves next -- the app-level equivalent of real
        MD3's own hour-then-minute dialog focus (this widget has no
        built-in toggle of its own; wire a button/tab to call this).
        """
        ...
    def get_time_picker_dial_mode(self) -> str:
        """`TimePickerDial`-only -- raises `ValueError` for any other
        kind. `"hour"` or `"minute"`.
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

class Theme:
    """M71: read-only access to a `Window`'s live theme resolution --
    the exact same lookups `window_factory.rs`'s own composition-only
    factories already make internally, now reachable from Python via
    `Window.theme`. A fresh wrapper each access (cheap) -- reads always
    see the window's current live state, including after a real
    `set_theme()` call.
    """

    def role(self, name: str) -> tuple[int, int, int, int] | None:
        """The resolved MD3 color for a role name (e.g. `"primary"`),
        or `None` both when no theme is set yet and when `name` isn't a
        real MD3 role.
        """
        ...
    def is_set(self) -> bool:
        """Whether a real theme has been resolved (`set_theme()`/a real
        `theme_seed` was given) at all.
        """
        ...
    def shape(self, component: str, variant: str | None = None) -> float | None:
        """The resolved corner-radius override for `component` (and
        `variant`, if given), or `None` if there's no override --
        callers fall back to their own real formula default.
        """
        ...
    def elevation(self, component: str, variant: str | None = None) -> float | None:
        """`shape`'s own sibling for elevation -- identical contract."""
        ...
    def typography(self, role: str) -> tuple[str, float, float, float] | None:
        """`(family, weight, size, line_height)` for a real MD3
        typography role, or `None` for an unrecognized role name.
        """
        ...

class Window:
    """One real OS window and the node tree painted into it. Add one or
    more to an `App`, then call `App.run()`.
    """

    def __init__(self, width: int = 480, height: int = 200, title: str = "tre v2") -> None: ...
    def create(self, kind: str, **props: Any) -> Node:
        """M95: makes a detached `"box"` or `"path"` node (a path needs
        `data`) and applies `props` atomically, as `Node.set` does.
        Attach it with `add_child`. Raises `ValueError` for an unknown
        kind or a bad property, creating nothing.
        """
        ...
    @property
    def root(self) -> Node:
        """M94: the window's root node (the shown one, after `show_view`)."""
        ...
    def on(self, event: str, handler: Callable[..., object]) -> None:
        """M94: registers `handler` for a window event -- `resize`,
        `color_scheme`, `scale_factor`, `close_requested` (cancellable
        with `event.cancel()`), or `closed` -- replacing any earlier one.
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
        `focus`/`blur` take `node`; `a11y_action` takes `node`, `action`
        (`increment`, `decrement`, `expand`, `collapse`,
        `scroll_into_view`, `set_value`), and `value`. `shift`/`ctrl`/`alt`/`meta` hold
        modifiers. Window events: `resize` (`width`, `height`),
        `color_scheme` (`dark`), `scale_factor` (`scale_factor`),
        `close_requested`, `closed`. Unknown events or fields raise
        `ValueError`.
        """
        ...
    @staticmethod
    def from_view(view: View, width: int = 480, height: int = 200, title: str = "tre v2") -> Window:
        """M42 Phase 1: shows a `View` (a declarative `view.yaml` +
        `ViewModel`, headless until now) in a real, live, on-screen
        window -- shares `view`'s own node tree directly rather than
        building a second, separate one, so a `Signal` write that
        re-evaluates a binding (`view.rs`'s own `_attach`) repaints this
        same window on the very next frame. `add_window`/`App.run()`
        work with the result exactly like any other `Window`.

        A real, live resize of the returned `Window` is immediately
        visible back on `view` itself -- `view.click(node)`/`view.hover
        (node)`, called again after this, lay out against the window's
        *current* real size, not the size passed here.
        """
        ...
    def show_view(self, view: View) -> None:
        """M42 Phase 2: switches which `View` this already-live `Window`
        shows, without closing/reopening it -- each named `View` a real
        app keeps around (its own `Reconciler`/bindings/`Signal`
        subscriptions) stays fully alive; only what this window renders
        and dispatches against changes, picked up on the next real
        frame. Real, stated limit: `view`'s own `width`/`height` are
        synced to this window's current size once, at switch time -- a
        later live resize while a *different* `View` is showing won't
        keep this one in sync until `show_view` is called on it again.
        """
        ...
    @property
    def theme(self) -> Theme:
        """M71: read-only access to this window's own live theme
        resolution. A fresh `Theme` wrapper each access -- reads always
        see this window's current live state, including after a real
        `set_theme()` call.
        """
        ...
    def set_theme(
        self,
        seed: tuple[int, int, int, int],
        dark: bool = False,
        default_theme: str | None = None,
        custom_theme: str | None = None,
        default_theme_spec: object | None = None,
        custom_theme_spec: object | None = None,
    ) -> None:
        """Builds a real MD3 `DynamicTheme` from `seed` and makes it
        this window's active theme.

        M86: `default_theme_spec`/`custom_theme_spec` are the dict forms
        of `default_theme`/`custom_theme` -- the same schema the theme
        YAML file holds (`colors`, `components`, `typography`, `seed`,
        ...), for a caller that loads its own files. Each is mutually
        exclusive with its path twin; passing both raises `ValueError`.

        M52: every real, already-built node this `Window` has created
        via an `add_*` factory is live re-themed in place, the moment
        this is called -- both `colors:` role overrides (a button's own
        real container/label color, not just the small, fixed ripple/
        hover/checkbox-mark/slider-track tint this method always pushed)
        and `components:` shape/elevation overrides (e.g. `{button.
        filled: {corner_radius: 8, elevation: 2}}`) recompute and
        overwrite every matching node's own real paint in place, the
        same "recompute from scratch, snap the result in" convention
        `View.set_theme` (§16.2) already established for the declarative
        surface. A later `add_*` call also picks up the new theme, for
        real, through the identical `ColorScheme::role` lookup every
        component already resolves colors through. **Real, honest limit,
        not silently glossed over:** a component's own real, app-owned
        interaction/selection state (a `Checkbox`'s `checked`, a `Radio
        Button`'s `selected`, which page a `Pagination` currently shows,
        etc.) is never re-derived by this call -- only each node's own
        theme-tier paint is recomputed, exactly the fields that state's
        own color/shape depends on, never the state itself (see `crates/
        engine-spec/src/theme.rs`'s own `components:` doc comment for the
        full key convention). `default_theme`'s own
        `components:` (omit for the engine's own shipped defaults,
        mirroring `View.__init__`'s identical convention) supplies the
        baseline, with `custom_theme`'s own `components:` layered on
        top (custom wins on any overlapping key) -- deliberately scoped
        to `components:` only: `default_theme`'s own `colors:`/`seed:`
        are never consulted here, since color/seed are already fully
        served by the required `seed` argument plus `custom_theme`'s
        own override, and a second theme file quietly competing with a
        required argument would be a real, confusing ambiguity.
        `custom_theme`'s own `seed:`, if present, overrides the `seed`
        argument. Raises `ValueError` for an unknown role name, an
        unparseable color, or invalid theme YAML.
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
        typography_role: str | None = None,
        font_family: str | None = None,
        font_weight: float | None = None,
        font_size: float | None = None,
        line_height: float | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A plain label -- `foreground` is its text color (a label has
        no fill of its own; M90 renamed this from `background`). `typography_role` is a real
        MD3 type-scale role name (e.g. `"body_large"`); it supplies
        `font_family`/`font_weight`/`font_size`/`line_height` as
        defaults, each of which may still be individually overridden.
        With no role and no explicit values, falls back to `"Roboto"`/
        `400.0`/`16.0`/the font's own natural line-height metrics --
        this method's own pre-existing defaults. Raises `ValueError` for
        an unrecognized `typography_role`.
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
        border_color: Color | None = None,
        border_width: float | None = None,
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
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """MD3's four real Icon Button variants: `"filled"`,
        `"filled_tonal"`, `"outlined"`, `"standard"`. `icon` is a
        Material Symbols icon name from this project's own curated
        set, same as `add_icon`. Raises `ValueError` for an unknown
        `variant` or `icon`. Returns the button's own container node,
        the same real contract `add_button` establishes.
        """
        ...
    def add_fab(
        self,
        icon: str,
        size: str = "default",
        variant: str = "surface",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """MD3's three real FAB sizes (`"small"`, `"default"`,
        `"large"`) and four real color variants (`"surface"`,
        `"primary"`, `"secondary"`, `"tertiary"`). Raises `ValueError`
        for an unknown `size`, `variant`, or `icon`. Returns the
        button's own container node -- a real FAB carries a genuine
        rest-state elevation shadow, unlike `add_icon_button`.
        """
        ...
    def add_extended_fab(
        self,
        label: str,
        width: float,
        icon: str | None = None,
        variant: str = "primary",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """`add_fab`'s own real color/elevation system with a real
        icon-plus-label anatomy -- one MD3 size (56dp tall). `icon` is
        optional, matching real MD3's own label-only Extended FAB
        variant. Raises `ValueError` for an unknown `variant` or
        `icon`.
        """
        ...
    def add_segmented_button(
        self,
        labels: Sequence[str],
        width: float,
        selected: Sequence[bool] | None = None,
        height: float = 40.0,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> list[Node]:
        """MD3's real group-of-connected-segments anatomy -- one
        shared outline frame, a real divider between each pair of
        adjacent segments, per-segment selected/unselected paint (a
        real checkmark shown on a selected segment). `labels` must
        have at least 2 entries; `selected` (when given) must have the
        same length as `labels`, defaulting to all unselected. Returns
        one real `Node` per segment, in order -- **not** the frame or
        dividers, which are pure decoration. Group-exclusivity
        (deselecting sibling segments on a real single-select click)
        is deliberately not built in here -- an app wires that up
        itself with `set_on_click`/`Node.animate`/`Node.add_child`/
        `Node.remove()` on the returned segments, the same "group-
        exclusivity is application state, not engine-owned" design
        this catalog's own future `Radio Button` also follows. Raises
        `ValueError` for fewer than 2 labels, a length mismatch, or an
        unknown icon (the checkmark's own curated name).
        """
        ...
    def add_chip(
        self,
        label: str,
        width: float,
        variant: str = "assist",
        icon: str | None = None,
        selected: bool = False,
        removable: bool = False,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """MD3's four real chip variants: `"assist"`, `"filter"`,
        `"input"`, `"suggestion"`. `icon` is an optional leading icon
        (`"filter"` shows a real checkmark instead when `selected`,
        replacing any custom `icon`). `removable` adds a real trailing
        close icon. `selected` only has a visual effect on
        `"filter"`. Raises `ValueError` for an unknown `variant` or
        `icon`.
        """
        ...
    def add_menu_item(
        self,
        label: str,
        icon: str | None = None,
        submenu: bool = False,
        width: float = 200.0,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """One real MD3 menu row. `submenu=True` adds a real trailing
        chevron indicator ("this item opens a nested menu") -- a
        purely visual affordance; the real submenu itself is just
        another `build_menu`/`open_menu` pair, opened with this
        item's own returned `Node` as the anchor (`Main Menu`
        submenus extend the existing context-menu overlay mechanism,
        not a new overlay kind). Raises `ValueError` for an unknown
        `icon`. Not shown anywhere on its own -- pass a list of these
        to `build_menu`.
        """
        ...
    def build_menu(self, items: Sequence[Node], width: float = 200.0) -> Node:
        """Assembles `items` (each from `add_menu_item`) into one real
        MD3 menu panel -- moves each item from wherever it currently
        lives into the returned panel. Returns the panel **not yet
        shown** -- pass it to `open_menu` to actually display it.
        Raises `ValueError` for an empty `items`, or if any item
        belongs to a different `Window`.
        """
        ...
    def open_menu(self, anchor: Node, menu: Node) -> None:
        """Opens `menu` (from `build_menu`) anchored just below
        `anchor`, dismissed on an outside click or Escape -- the same
        real overlay primitive `Node.set_context_menu`'s right-click
        path uses, just triggered directly rather than gated behind a
        right-click. A safe no-op if `menu` is already open. Raises
        `ValueError` if `anchor`/`menu` belong to a different `Window`.
        """
        ...
    def close_menu(self, menu: Node) -> None:
        """Closes a menu opened via `open_menu`. Raises `ValueError`
        if `menu` belongs to a different `Window`.
        """
        ...
    def add_badge(
        self,
        label: str | None = None,
        width: float | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 badge -- a 6dp dot with no `label`, or a 16dp
        labeled pill with one. Always overlaid on a corner of some
        other component via caller-chosen `x`/`y`; not positioned
        relative to another node automatically. `width` only matters
        for the labeled variant (defaults to a circle sized for one
        digit) -- a real multi-digit badge needs a wider caller-
        supplied `width`.
        """
        ...
    def add_linear_progress(
        self,
        width: float,
        height: float = 4.0,
        value: float = 0.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 linear progress indicator -- a track plus a
        filled indicator, `value` clamped `0.0..=1.0`. Read/write the
        current value via `Node.get("value")`/`Node.animate("value",
        ...)`.
        """
        ...
    def add_circular_progress(
        self,
        size: float = 48.0,
        value: float = 0.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """`add_linear_progress`'s own real circular sibling -- a
        stroked arc from 12 o'clock, sweeping clockwise by `value *
        360°`. `value` clamped `0.0..=1.0`, read/write the same way.
        """
        ...
    def add_loading_indicator(
        self,
        size: float = 48.0,
        foreground: Color | None = None,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real, perpetually-looping MD3 Expressive-style loading
        spinner -- morphs between four real shapes (Pentagon, Pill,
        Cookie, Oval) with no app-side wiring needed at all; it starts
        looping the instant it's constructed and never stops. `size`
        is a single square dimension, the same real convention `add_
        circular_progress`'s own `size` param already establishes.
        `foreground` defaults to the current theme's own real `primary`
        role (or a fixed baseline before `Window.set_theme`). **Real,
        honest v1 simplification:** this is a real, recognizable
        subset of MD3 Expressive's own actual seven-shape sequence,
        morphed with plain easing rather than genuine spring physics
        -- not a pixel-for-pixel spec match.
        """
        ...
    def add_time_picker_dial(
        self,
        hour: int = 0,
        minute: int = 0,
        size: float = 256.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 Time Picker's circular clock-face drag control --
        drag anywhere in the box to move the currently-active hand
        (`hour` by default; switch with `Node.set_time_picker_dial_
        mode`). `hour` is a real 24-hour value (`0..=23`); the hour
        hand alone never flips AM/PM (no toggle chrome exists on this
        widget -- see `set_time_picker_dial_mode`). `minute` (`0..=59`)
        snaps to the nearest real 5-minute increment while dragging.
        Releasing the drag produces a `"changed"` event, the same real
        convention `add_slider` already establishes. **Real, honest v1
        simplification:** no digit labels around the face (plain tick
        marks stand in for them) and no AM/PM toggle or digital input
        chrome -- this is the real circular drag primitive, not MD3's
        whole Time Picker dialog.
        """
        ...
    def add_card(
        self,
        width: float,
        height: float,
        variant: str = "elevated",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """MD3's three real card variants: `"elevated"`, `"filled"`,
        `"outlined"`. A plain container -- add arbitrary content with
        `Node.add_child`. Raises `ValueError` for an unknown
        `variant`.
        """
        ...
    def add_divider(
        self,
        length: float,
        orientation: str = "horizontal",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 1dp divider line -- `length` wide and 1dp tall
        when `orientation="horizontal"` (the default), or the reverse
        when `orientation="vertical"`.
        """
        ...
    def add_tooltip(
        self,
        text: str,
        width: float,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 plain tooltip panel -- returned **not yet
        attached anywhere**, the same real contract `build_menu`'s own
        panel has. Show/hide it with `open_menu`/`close_menu`,
        triggered from the anchor's own `Node.set_on_hover_enter`/
        `set_on_hover_exit` -- there is no dedicated `open_tooltip`/
        `close_tooltip` pair, since those would only duplicate
        `open_menu`/`close_menu`.
        """
        ...
    def add_dialog(
        self,
        headline: str,
        supporting_text: str,
        width: float,
        height: float,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 modal dialog -- a full-window scrim with the
        panel (headline + supporting text) centered inside it.
        Deliberately has no `x`/`y` -- a real dialog is always
        centered. Returns the **scrim** node, not yet attached
        anywhere -- pass it to `open_dialog` to actually show it.
        """
        ...
    def open_dialog(self, dialog: Node) -> None:
        """Opens `dialog` (from `add_dialog`) as a real modal --
        blocks interaction with everything behind it (does not
        dismiss on an outside click; dismisses on Escape). A safe
        no-op if `dialog` is already open. Raises `ValueError` if
        `dialog` belongs to a different `Window`.
        """
        ...
    def close_dialog(self, dialog: Node) -> None:
        """Closes a dialog opened via `open_dialog`. Raises
        `ValueError` if `dialog` belongs to a different `Window`.
        """
        ...
    def add_snackbar(
        self,
        text: str,
        width: float,
        action_label: str | None = None,
        closable: bool = False,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node | None, Node | None]:
        """A real MD3 snackbar -- a transient notification. Returns
        `(container, action, close)`: `action`/`close` are `None`
        unless `action_label`/`closable` were given, and each is a
        real, independently `enable_interaction()`-able `Node` --
        unlike `Chip`'s purely decorative `removable` icon, a real
        snackbar action must be clickable on its own. Returned
        genuinely unattached anywhere -- pass `container` to
        `open_snackbar` to actually show it. This engine has no
        timer/scheduler primitive, so a real auto-dismiss-after-
        duration is the app's own responsibility.
        """
        ...
    def open_snackbar(self, snackbar: Node) -> None:
        """Opens `snackbar` (the `container` from `add_snackbar`)
        anchored to the real desktop bottom-left corner. Never
        dismisses on an outside click or Escape, and never blocks
        background interaction (unlike a modal `Dialog`) -- only its
        own action/close, or whatever the app itself drives, closes
        it. A safe no-op if already open. Raises `ValueError` if
        `snackbar` belongs to a different `Window`.
        """
        ...
    def close_snackbar(self, snackbar: Node) -> None:
        """Closes a snackbar opened via `open_snackbar`. Raises
        `ValueError` if `snackbar` belongs to a different `Window`.
        """
        ...
    def add_side_sheet(
        self,
        width: float = ...,
        height: float | None = None,
        modal: bool = False,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 side sheet, docked to the right edge. When
        `modal` is `False` (the default, MD3's real *Standard*
        variant): a plain layout participant, already attached --
        the app re-parents it into its own layout (`Node.add_child`)
        or docks it via the existing `Dock`. When `modal` is `True`
        (real *Modal* variant): a real floating overlay with a
        full-window scrim, returned **unattached** -- pass it to
        `open_side_sheet` to actually show it. `height` defaults to
        the window's own full height.
        """
        ...
    def open_side_sheet(self, side_sheet: Node) -> None:
        """Opens a **modal** `side_sheet` (from `add_side_sheet(...,
        modal=True)`) -- blocks interaction with everything behind
        it, dismisses on Escape but not an outside click. A real,
        explicit no-op for a standard (`modal=False`) side sheet,
        which has no overlay lifecycle at all. A safe no-op if
        already open. Raises `ValueError` if `side_sheet` belongs to
        a different `Window`.
        """
        ...
    def close_side_sheet(self, side_sheet: Node) -> None:
        """Closes a modal side sheet opened via `open_side_sheet`. A
        real, explicit no-op for a standard (`modal=False`) side
        sheet. Raises `ValueError` if `side_sheet` belongs to a
        different `Window`.
        """
        ...
    def add_navigation_rail(
        self,
        labels: list[str],
        icons: list[str],
        selected: int | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> list[Node]:
        """A real MD3 navigation rail, the desktop counterpart to
        Navigation Bar. `labels`/`icons` must be the same length (one
        icon per item) and non-empty. Returns one real, independently
        `enable_interaction()`-able `Node` per item, in order -- the
        rail's own background frame is never returned. Raises
        `ValueError` if `labels`/`icons` lengths mismatch, if empty,
        or if `selected` is out of range.
        """
        ...
    def add_navigation_drawer(
        self,
        labels: list[str],
        icons: list[str],
        selected: int | None = None,
        modal: bool = False,
        width: float = ...,
        height: float | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, list[Node]]:
        """A real MD3 navigation drawer, docked to the left edge.
        Returns `(container, items)`: one real, independently
        `enable_interaction()`-able `Node` per destination, in order.
        When `modal` is `False` (the default, real *Standard*
        variant): `container` is already attached -- re-parent it
        into your own layout or dock it. When `modal` is `True` (real
        *Modal* variant): `container` is the scrim, returned
        **unattached** -- pass it to `open_navigation_drawer` to show
        it. `height` defaults to the window's own full height. Raises
        `ValueError` if `labels`/`icons` lengths mismatch, if empty,
        or if `selected` is out of range.
        """
        ...
    def open_navigation_drawer(self, drawer: Node) -> None:
        """Opens a **modal** `drawer` (from `add_navigation_drawer(...,
        modal=True)`) -- blocks interaction with everything behind
        it, dismisses on Escape but not an outside click. A real,
        explicit no-op for a standard (`modal=False`) drawer, which
        has no overlay lifecycle at all. A safe no-op if already
        open. Raises `ValueError` if `drawer` belongs to a different
        `Window`.
        """
        ...
    def close_navigation_drawer(self, drawer: Node) -> None:
        """Closes a modal navigation drawer opened via
        `open_navigation_drawer`. A real, explicit no-op for a
        standard (`modal=False`) drawer. Raises `ValueError` if
        `drawer` belongs to a different `Window`.
        """
        ...
    def add_top_app_bar(
        self,
        title: str,
        leading_icon: str | None = None,
        trailing_icons: list[str] | None = None,
        width: float | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node | None, list[Node]]:
        """A real MD3 top app bar, the *Small* variant. Returns `(bar,
        leading, trailing)`: `leading` is `None` unless `leading_icon`
        was given; `trailing` is one real, independently
        `enable_interaction()`-able `Node` per entry in
        `trailing_icons`, empty if none. `width` defaults to the
        window's own full width.
        """
        ...
    def add_toolbar(
        self,
        variant: str = "docked",
        orientation: str | None = None,
        vibrant: bool = False,
        width: float | None = None,
        height: float | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 Toolbar -- `variant` is `"docked"` (spans the
        full window width by default, square corners) or `"floating"`
        (hugs its own content by default, always fully rounded, real
        elevation). `orientation` (`"horizontal"`/`"vertical"`) only
        applies to a floating toolbar -- a docked toolbar is always
        horizontal and raises `ValueError` if asked for vertical.
        `vibrant=False` uses a `surface_container` fill, `vibrant=True`
        a `primary_container` fill. A real "container with
        configurable slots" per MD3's own anatomy: populate the
        returned node with any already-built node (a `Button`, `Icon
        Button`, `TextField`, etc.) via the existing, generic
        `Node.add_child` -- there is no specialized children-list
        parameter here.
        """
        ...
    def add_split_button(
        self,
        label: str,
        width: float,
        height: float,
        variant: str = "filled",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node, Node]:
        """A real MD3 Split Button -- a leading button (the same real
        color variants as `add_button`: `"elevated"`/`"filled"`/
        `"filled_tonal"`/`"outlined"`/`"text"`) plus a separate
        trailing menu-icon button, joined by a small real gap. Returns
        `(leading, trailing, trailing_icon)`: `trailing_icon` is the
        real `expand_more` icon node itself -- animate its own
        `"rotation"` property (0.0 to 180.0 degrees) when your own menu
        opens/closes; the engine never does this automatically (the
        same "engine provides the mechanism, app decides the real
        state change" split `Checkbox.checked` already uses). The two
        buttons' own facing inner corners automatically tighten while
        either is hovered, then relax back -- no app-side wiring needed.
        """
        ...
    def add_button_group(
        self,
        labels: list[str],
        width: float,
        height: float,
        variant: str = "filled",
        x: float | None = None,
        y: float | None = None,
    ) -> tuple[Node, list[Node]]:
        """A real MD3 Standard Button Group -- an invisible container
        holding `len(labels)` real `add_button`-built buttons (all the
        same `variant`/`width`/`height`, real MD3 anatomy's own
        default), spaced with a real gap. Pressing one live-reflows its
        own width and its immediate neighbors' *and* reshapes toward a
        real, per-size "square" corner radius (MD3 Expressive's own
        "buttons reshape as you press them"), both driven entirely by
        the engine, no app-side wiring needed. Returns `(group,
        buttons)`. There is no `Connected Button Group` variant here --
        MD3's own spec states it directly replaces the already-built
        `add_segmented_button`, which already covers that real anatomy.
        """
        ...
    def add_tabs(
        self,
        labels: list[str],
        icons: list[str] | None = None,
        selected: int | None = None,
        width: float | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> list[Node]:
        """Real MD3 tabs, the *Primary Navigation Tab* variant.
        `icons`, if given, must be the same length as `labels`.
        Returns one real, independently `enable_interaction()`-able
        `Node` per tab, in order -- the row's own background frame is
        never returned. `width` defaults to the window's own full
        width, divided evenly across the tabs. Raises `ValueError` if
        `labels` is empty, if `icons` is given with a mismatched
        length, or if `selected` is out of range.
        """
        ...
    def add_search_bar(
        self,
        placeholder: str,
        width: float,
        leading_icon: str | None = None,
        trailing_icons: list[str] | None = None,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node, Node | None, list[Node]]:
        """A real MD3 search bar. Returns `(bar, text_field, leading,
        trailing)`: `text_field` is a real `NodeKind.TextField` node
        (typing, focus, selection, IME all work exactly like
        `add_text_field`'s own); `leading` is `None` unless
        `leading_icon` was given; `trailing` is one real Node per
        entry in `trailing_icons`, empty if none.
        """
        ...
    def add_search_view(
        self,
        width: float,
        height: float,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 search view -- the *docked* dropdown suggestions
        panel (MD3's own *full-screen* variant is a mobile pattern,
        not built here). A plain styled container with no fixed
        content anatomy -- populate it with `Node.add_child`. Returned
        genuinely unattached anywhere -- show/hide it via the existing
        `Window.open_menu`/`close_menu` (the same real reuse
        `add_tooltip`'s own panel already has), anchored to the search
        bar's own `bar` node.
        """
        ...
    def add_list_item(
        self,
        headline: str,
        leading_icon: str | None = None,
        trailing_icon: str | None = None,
        supporting_text: str | None = None,
        width: float = ...,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 list item -- one-line (56dp) by default, or a
        real two-line (72dp) variant when `supporting_text` is given.
        Already attached -- pass it (with siblings) to `add_list` to
        group them into an actual list frame.
        """
        ...
    def add_list(
        self,
        items: list[Node],
        width: float = ...,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """Groups `add_list_item`-built rows into one real, plain,
        non-virtualized vertical list -- for small real collections;
        `VirtualList` stays the real choice for large ones. Moves
        each item (detach, then re-parent) into the returned frame,
        the same real mechanism `build_menu` already uses. Raises
        `ValueError` if `items` is empty or any item belongs to a
        different `Window`.
        """
        ...
    def add_accordion_header(
        self,
        title: str,
        expanded: bool = False,
        width: float = ...,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node]:
        """An `Accordion` header -- MD3 has no official Accordion
        page; this reuses List Item's own real anatomy (a headline +
        trailing expand/collapse chevron). Returns `(header, chevron)`:
        `header` is the real clickable row; `chevron` is the real,
        independently-addressable icon `Node` to flip on toggle.
        This engine has no rotation-animation primitive exposed to
        Python, so flip it via `Node.animate("transform", (0.0, 0.0,
        -1.0))` (collapsed: `1.0`) -- a uniform negative scale, which
        for this glyph's own point-symmetric shape looks identical to
        a real 180° rotation. `expanded` seeds the chevron's own
        initial orientation to match.
        """
        ...
    def add_tree_node(
        self,
        title: str,
        depth: int = 0,
        expanded: bool = False,
        leaf: bool = False,
        width: float = ...,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node | None]:
        """One real `Tree View` row -- the identical real grounding
        `add_accordion_header` already has, applied recursively:
        `depth` adds real left indentation per nesting level. Returns
        `(header, chevron)`: `chevron` is `None` when `leaf` is `True`
        (nothing to expand); otherwise it's the same real,
        independently-addressable icon `Node` `add_accordion_header`
        already returns, flipped the same way (`Node.animate(
        "transform", (0.0, 0.0, -1.0))`).
        """
        ...
    def add_date_picker_day(
        self,
        day: int,
        selected: bool = False,
        today: bool = False,
        outside_month: bool = False,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 date-picker day cell -- the *docked* variant's
        own real anatomy (48x48dp, `corner-full`). `selected` wins
        over `today` if both are `True` (a selected today still shows
        the filled circle, not the outline). Deliberately scoped to
        just the cell -- a real calendar grid needs real date
        arithmetic, which belongs in the app (Python's own `datetime`/
        `calendar` modules), not this engine.
        """
        ...
    def add_time_input_field(
        self,
        value: str,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """One real MD3 Time Input hour/minute field (96x72dp,
        `surface_container_highest`, real `corner-small` shape, a
        large Display Medium numeral). A genuine `NodeKind.TextField`
        -- typing, focus, and selection all work exactly like
        `add_text_field`'s own. Deliberately the real *Time Input*
        variant, not the analog clock-face dial -- that needs a
        genuinely new drag-to-angle engine capability this project
        doesn't have.
        """
        ...
    def add_period_selector(
        self,
        selected: str = "AM",
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, Node]:
        """A real MD3 AM/PM period selector -- two real, independently
        `enable_interaction()`-able `Node`s (`am`, `pm`) stacked
        vertically (52x36dp each). The selected one fills with
        `tertiary_container`; the app drives real toggling itself
        (group-exclusivity is application state, the same real
        contract `Segmented Button`/`Chip` already have). Raises
        `ValueError` if `selected` isn't `"AM"` or `"PM"`.
        """
        ...
    def add_popover(
        self,
        subhead: str,
        supporting_text: str,
        width: float,
        height: float,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real MD3 popover, grounded in MD3's own Rich Tooltip
        anatomy (`surface_container`, real `corner-medium` shape, a
        subhead + supporting text). Genuinely *persistent* -- unlike
        the plain `add_tooltip`, it stays open until dismissed.
        Returned genuinely unattached anywhere -- show/hide it via
        the existing `Window.open_menu`/`close_menu` (the same real
        reuse `add_tooltip`'s own panel already has), not a new
        dedicated open/close pair.
        """
        ...
    def add_link(
        self,
        content: str,
        width: float,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real, standalone clickable label (MD3 has no official
        Link page; `primary`-colored, Body Large). Backed by a real,
        dedicated `NodeKind.Link` -- unlike a bare label built from
        the lower-level view/text primitives, it genuinely claims its
        own clicks rather than deferring to its own parent. Does not
        call `enable_interaction()` automatically.
        """
        ...
    def add_spin_box(
        self,
        value: str,
        x: float | None = None,
        y: float | None = None,
    ) -> tuple[Node, Node, Node]:
        """A real numeric increment control -- deliberately named
        `SpinBox`, not `Stepper` (MD3's own vocabulary already uses
        that name for a completely different multi-step flow
        indicator). Returns `(field, decrement, increment)`: `field`
        is a real `NodeKind.TextField` (typing/focus/selection all
        work); `decrement`/`increment` are each a real, independently
        `enable_interaction()`-able `Node` -- the app wires up real
        `+`/`-1` logic itself.
        """
        ...
    def add_pagination(
        self,
        page_count: int,
        current: int = 0,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> tuple[Node, list[Node], Node]:
        """A real MD3 pagination control. Returns `(previous, pages,
        next)`: `pages` is one real, independently `enable_
        interaction()`-able `Node` per page (`1`-indexed labels),
        `previous`/`next` each a real prev/next-page `Node` --
        "which page is current" is app-owned state, the app drives
        real re-selection itself. Raises `ValueError` if `page_count`
        is `0` or `current` is out of range.
        """
        ...
    def add_status_bar(
        self,
        text: str,
        width: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real, styled status bar (24dp, `surface_container`,
        Label Small text). Pass the returned `Node` directly to
        `build_shell`'s own already-real `status_bar` parameter --
        no new shell-level wiring, just the real content. `width`
        defaults to the window's own full width.
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
    def add_radio_button(
        self,
        size: float = 20.0,
        selected: bool = False,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 radio button -- a stroked ring plus a scaling
        inner dot, colors always resolved from the active theme (or a
        real MD3 baseline default) rather than caller-supplied.
        Group-exclusivity is the app's own responsibility (`Node.
        set_selected`/`Node.animate("select_progress", ...)` on each
        radio in a group), the same "application state, not
        engine-owned" design this catalog uses throughout.
        """
        ...
    def add_switch(
        self,
        width: float = 52.0,
        height: float = 32.0,
        selected: bool = False,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 switch -- a track plus a handle that both slides
        and grows as it toggles, colors always resolved from the
        active theme (or a real MD3 baseline default) rather than
        caller-supplied, the same real contract `add_radio_button`
        already establishes.
        """
        ...
    def add_slider(
        self,
        background: Color,
        width: float,
        height: float,
        value: float = 0.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """`value` is the initial position, clamped `0.0..=1.0`; read and animate it as the `"value"` property."""
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
        """`add_image`'s decode-free sibling: `rgba` is already-decoded
        straight-alpha RGBA8 pixels (`pixel_width * pixel_height * 4`
        bytes exactly, or a clear `ValueError`) -- no file, no `image`
        crate involved, the caller owns decoding entirely. `width`/
        `height` are the node's own fixed display box (`add_image`'s
        identical contract); `pixel_width`/`pixel_height` describe
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
        `push_frame` call. `fit` is `add_image`'s own identical
        `"cover"`/`"contain"`/`"fill"` parameter.
        """
        ...
    def add_node_graph(
        self,
        width: float,
        height: float,
        x: float | None = None,
        y: float | None = None,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """The pannable/zoomable viewport half of a Node Graph -- pass
        the returned `Node` as `add_graph_node`'s own `graph` parameter.
        Pan/zoom the whole graph via `graph.animate("transform", (dx,
        dy, scale), ...)`, the same mechanism every other transform-
        animated node already uses.
        """
        ...
    def add_graph_node(
        self,
        graph: Node,
        label: str,
        x: float,
        y: float,
        width: float,
        height: float,
        border_color: Color | None = None,
        border_width: float | None = None,
    ) -> Node:
        """A real, styled node (a title bar over a body) for a Node
        Graph, attached directly under `graph` (not the window root)
        so it pans/zooms together with it -- `x`/`y` are local to
        `graph`'s own origin. Move it later via `node.animate(
        "transform", (dx, dy, 1.0), ...)`. No dedicated edges API and
        no drag-to-move mouse gesture -- see the Rust source's own doc
        comment for the real, stated reasons (this codebase has no
        Python-facing pointer-move-while-pressed hook yet); draw edges
        with an `add_canvas` node reparented into `graph` alongside its
        nodes instead. Raises if `graph` belongs to a different
        `Window`.
        """
        ...
    def add_icon(
        self,
        name: str,
        foreground: Color,
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
    def add_carousel(
        self,
        layout: str,
        width: float,
        height: float,
        background: Color,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 carousel. `layout` is one of `"uncontained"`
        (items keep their own width, scroll by raw pixels),
        `"hero"`, or `"multi_browse"` (items automatically resize and
        snap into place -- `Node.set_carousel_index`/real wheel/drag
        input move it). Raises `ValueError` for an unknown `layout`.
        Returns an empty strip -- add real items the same generic way
        any other container's children are added, via `Node.add_child`.
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
    def thread_handle(self) -> LoopHandle:
        """M87: a thread-safe handle to this `App`'s event loop. `App`,
        `Window` and `View` may only be used from the thread that created
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
        idle one. There it can touch `View`/`Window`/`Node` like an input
        handler can, e.g. `view.reconcile(spec=...)` for hot reload.

        Safe from any thread, before, during, or after `App.run()`.
        Callbacks run in FIFO order at the top of the next frame; one
        queued outside a run waits for the next run's first frame. An
        exception is logged like one from an input handler and doesn't
        stop the loop or later callbacks. Raises `TypeError` if
        `callback` isn't callable.
        """
        ...

class View:
    """Loads a declarative `view.yaml` file -- the §16.2 MVVM surface's
    Rust-side crossing point. Pair with a Python `ViewModel` subclass
    (`tre.ViewModel`), not used directly for imperative node creation
    the way `Window` is.

    Works headless (no `Window` needed) via `click`/`hover`/`right_click`
    below, or shown live via `Window.from_view(view)` -- see `Window`'s
    own doc comment.
    """

    def __init__(
        self,
        path: str | None = None,
        stylesheet: str | None = None,
        theme_seed: tuple[int, int, int, int] | None = None,
        dark: bool = False,
        default_theme: str | None = None,
        custom_theme: str | None = None,
        source: str | None = None,
        spec: object | None = None,
        json: str | None = None,
        stylesheet_spec: object | None = None,
        default_theme_spec: object | None = None,
        custom_theme_spec: object | None = None,
    ) -> None:
        """M86: `stylesheet_spec`/`default_theme_spec`/`custom_theme_spec`
        are the dict forms of `stylesheet`/`default_theme`/`custom_theme`
        (the same schema each YAML file holds), for a caller that loads
        its own files and hands `tre` data only. Each is mutually
        exclusive with its path twin; passing both raises `ValueError`.

        `stylesheet` is a path to a stylesheet YAML file (§16.3's
        cascade); `theme_seed` builds a real MD3 `DynamicTheme` the
        same way `Window.set_theme` does, resolving any `background:
        primary`-style MD3 token name in the view/stylesheet.

        M49: `default_theme`/`custom_theme` (paths to theme YAML files)
        are two more cascade tiers, resolved *beneath* `stylesheet` and
        the widget's own inline `style:` -- `default theme < custom
        theme < stylesheet < inline`. Omitting `default_theme` uses the
        engine's own shipped default. Either theme's `colors:` overrides
        a role in the active `ColorScheme`; either theme's own `seed:`
        (if present) sets the seed when `theme_seed` isn't explicitly
        given (an explicit `theme_seed` always wins). Raises `ValueError`
        for an unknown role name, an unparseable color, or invalid theme
        YAML.

        M50: a theme's own `components:` section (shape/elevation
        overrides for the *imperative* MD3 catalog, `Window.add_button`/
        `add_fab`/etc.) is silently unused here -- `View` has no
        imperative factories to apply it to. Present in the shared
        `ThemeSpec` type so one theme file can serve both `View` and
        `Window`; see `Window.set_theme`'s own docstring for what it does.

        M71: `source`, when given, is used directly as the view's YAML
        text instead of reading `path` from disk -- `path` still
        supplies the real base directory `include:`/`image.src:`
        resolve against, and the real file `poll_reload()`/hot-reload
        watches.

        M78 (tre issue #3 Tier 1): `spec`, when given, is a real Python
        object (a dict shaped like the view's own YAML tree) built
        directly into the tree -- no YAML text at all. `path` becomes
        optional: omitted, there's no base directory to resolve against
        and no file to watch for hot-reload (`poll_reload()` then
        always returns `False`; use `reconcile()` instead).

        0.3.1 review, item 2: `json`, when given, is JSON text parsed
        directly into the tree -- the real first consumer of `engine_
        spec::parse_view_json`. Grouped with `spec`, not `source`: both
        are just different ways to obtain the tree data directly, with
        no real backing file implied by either, so `path` is optional
        with `json` too. `source`'s own `path` requirement is specific
        to it -- pre-processed *real file* content still wanting real
        hot-reload. At most one of `spec`/`source`/`json` may be given;
        at least one of `spec`/`json`/`path` is required. Raises
        `ValueError` if more than one of `spec`/`source`/`json` is
        given, or if none of `spec`/`json`/`path` is given.
        """
        ...
    def node(self, widget_id: str) -> Node:
        """Looks up a declared widget by its own `id:` from the YAML."""
        ...
    def poll_reload(self, source: str | None = None) -> bool:
        """Checks whether the underlying YAML file changed on disk
        since it was last loaded and, if so, reconciles the tree in
        place (preserving `NodeId`/focus/in-flight animations where
        possible). Returns whether a reload actually happened.

        M71: `source`, when given, is reconciled instead of a fresh
        disk read of `path` -- the real change-detection gate still
        watches `path` on disk regardless, so this only changes what
        gets reconciled once a real file change is detected, not
        whether one is. A `View` with no `path` at all (M78's `spec=`
        construction) has no watcher and always returns `False` here --
        see `reconcile()` for the ungated equivalent.
        """
        ...
    def reconcile(
        self, source: str | None = None, spec: object | None = None, json: str | None = None
    ) -> None:
        """M79 (tre issue #3 Part C): the ungated sibling of
        `poll_reload` for a `View` built with `spec=` and no backing
        file to watch. Reconciles against `source` (YAML text), `spec`
        (a real Python object, depythonized directly), or `json` (JSON
        text, 0.3.1 review item 2) -- unconditionally, with no "did
        anything change" check, since the caller's own explicit call
        already is the change signal (typically driven by `tre.Effect`).
        At most one of `spec`/`source`/`json` may be given; exactly one
        is required. Raises `ValueError` if more than one is given, or
        if none is given.
        """
        ...
    def set_stylesheet(
        self,
        stylesheet_spec: object | None = None,
        stylesheet: str | None = None,
    ) -> None:
        """M91 (issue #8): replaces this view's stylesheet and
        re-resolves every node in place, like `set_theme` -- `NodeId`s,
        focus, and in-flight animations are preserved, and the attached
        ViewModel's bindings are re-applied afterward. `stylesheet_spec`
        (a dict) and `stylesheet` (a YAML file path) are mutually
        exclusive; passing neither clears the stylesheet.
        """
        ...
    def set_theme(
        self,
        default_theme: str | None = None,
        custom_theme: str | None = None,
        theme_seed: tuple[int, int, int, int] | None = None,
        dark: bool = False,
        default_theme_spec: object | None = None,
        custom_theme_spec: object | None = None,
    ) -> None:
        """M51: live re-theme. M86: `default_theme_spec`/
        `custom_theme_spec` are the dict forms of `default_theme`/
        `custom_theme`, same contract as `__init__`. Re-resolves `default_theme`/`custom_theme`/
        `theme_seed`/`dark` exactly like `__init__` does, then walks
        every already-built node in this `View`'s tree and recomputes
        its `PaintProperties`/`layout_style` from its own YAML spec
        against the new theme layers, overwriting in place -- the same
        real "recompute and overwrite" step `poll_reload` already runs
        on content changes, just unconditional (spec unchanged, only
        the theme differs) rather than skipped for unchanged nodes.
        `NodeId`/children/focus are preserved; a widget's own inline
        `style:` still wins over any theme layer, exactly like at
        construction time.

        Real, deliberate convention, matching `Window.set_theme`'s own
        precedent: each call is a complete, fresh theme selection --
        omitting `default_theme`/`custom_theme` resets to the engine's
        shipped default / no custom override, *not* "keep whatever the
        previous call used." A `poll_reload()` called after this
        continues resolving against the theme this call installed.

        M91 (issue #8): the attached ViewModel's bindings are re-applied
        afterward, so a bound field keeps its live value. `View` has no imperative
        factories, so `Window.set_theme`'s own `components:` shape/
        elevation section has nothing to apply to here.
        """
        ...
    def instantiate(
        self, path: str, into: Node, source: str | None = None, spec: object | None = None
    ) -> Component:
        """M43 Phase 1: embeds another view's own YAML as a real,
        independent `Component` -- its own bindings/handlers, ready for
        its own separate `ViewModel` to `_attach` to -- spliced into
        this `View`'s live tree as a child of `into`. Call this once per
        instance for multiple simultaneous instances (e.g. one per row
        in a list); each instantiation is fully independent, even when
        the same `path` is used repeatedly.

        M73: `source`, when given, is used directly instead of reading
        `path` from disk -- the same real `View.__init__`/`source=`
        precedent, widened here for the embedded-component macro-
        expansion case (Tesserae's own pre-processed component YAML).

        0.3.1 review, item 3: `spec`, when given, is a real Python
        object built directly into the tree, mirroring `View.__init__`
        's own `spec=` (M78) -- no YAML text at all. Unlike `View.
        __init__`, `path` stays **required** here (every real call site
        already calls `instantiate(path, into)` positionally, and `into`
        -- also required -- comes right after it, so making `path`
        optional would break every one of them). Pass `path=""` when
        using `spec=`/`source=` with no real file to name -- the same
        "no base directory" outcome an omitted `path` means for `View.
        __init__`. At most one of `spec`/`source` may be given; at
        least one of `spec`/`source`/a non-empty `path` is required.
        """
        ...
    def click(self, node: Node) -> None: ...
    def hover(self, node: Node) -> None: ...
    def focus(self, node: Node) -> None: ...
    def right_click(self, node: Node) -> None: ...

class Component:
    """M43 Phase 1: one real, embedded instance of another view's own
    YAML, created via `View.instantiate`/`Component.instantiate` -- not
    constructed directly. Behaves like a small `View` scoped to just
    this instance's own widgets (its own `node`/`_attach`), sharing the
    same live `Tree` as whatever it was instantiated into.

    Has no `click`/`hover`/`right_click` of its own -- dispatch on one
    of its nodes goes through the *owning* `View`/`Window`'s existing
    method instead, e.g. `view.click(component.node("button"))`.
    """

    def node(self, widget_id: str) -> Node:
        """Looks up a declared widget by its own `id:`, scoped to this
        component instance."""
        ...
    def instantiate(
        self, path: str, into: Node, source: str | None = None, spec: object | None = None
    ) -> Component:
        """Embeds another component inside this one -- components nest
        recursively, the identical real mechanism `View.instantiate`
        itself uses.

        M73: `source`, when given, is used directly instead of reading
        `path` from disk -- see `View.instantiate`'s own docstring.

        0.3.1 review, item 3: `spec`, when given, mirrors `View.
        instantiate`'s own -- see its docstring for the real reasoning,
        including why `path` stays required here.
        """
        ...
    def remove(self) -> None:
        """M43 Phase 2: real, structural teardown -- unsubscribes every
        `Signal` this instance's own bindings subscribed to (so a later
        write to one no longer tries to reach a `NodeId` that's gone),
        then removes this instance's whole subtree from the shared
        `Tree`. Safe to call once; the instance shouldn't be used again
        afterward (its own `NodeId`s are no longer valid).
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

def _record_read(signal: object) -> None:
    """Internal -- called from `Signal.get()`. Only appends `signal` to
    the current binding evaluation's dependency list while one is
    genuinely in progress (`View._attach`); a no-op otherwise. Not part
    of the public API.
    """
    ...

def _begin_recording() -> None:
    """Internal -- pushes a fresh recording frame. `Computed`/`Effect`/
    `untrack` (`tre.__init__`) pair this with `_end_recording` to open a
    dependency-tracking scope around their own callable, the same real
    mechanism `View._attach` already uses for `{{ }}` bindings. Not part
    of the public API.
    """
    ...

def _end_recording() -> list[Any]:
    """Internal -- pops the current recording frame and returns every
    distinct object `_record_read` saw while it was on top (by identity,
    in first-read order). Not part of the public API.
    """
    ...
