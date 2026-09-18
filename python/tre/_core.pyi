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
    def set_selected(self, selected: bool) -> None:
        """`RadioButton`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def set_on(self, on: bool) -> None:
        """`Switch`-only -- raises `ValueError` for any other kind."""
        ...
    def set_text(self, content: str) -> None:
        """`TextField`/`Text`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def get_checked(self) -> bool:
        """`Checkbox`-only -- raises `ValueError` for any other kind."""
        ...
    def get_selected(self) -> bool:
        """`RadioButton`-only -- raises `ValueError` for any other
        kind.
        """
        ...
    def get_on(self) -> bool:
        """`Switch`-only -- raises `ValueError` for any other kind."""
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
    def add_fab(
        self,
        icon: str,
        size: str = "default",
        variant: str = "surface",
        x: float | None = None,
        y: float | None = None,
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
        width: float = 200.0,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """One real MD3 menu row. Raises `ValueError` for an unknown
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
    def add_card(
        self,
        width: float,
        height: float,
        variant: str = "elevated",
        x: float | None = None,
        y: float | None = None,
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
        vertical: bool = False,
        x: float | None = None,
        y: float | None = None,
    ) -> Node:
        """A real MD3 1dp divider line -- `length` wide and 1dp tall
        when horizontal (the default), or the reverse when
        `vertical=True`.
        """
        ...
    def add_tooltip(
        self,
        text: str,
        width: float,
        x: float | None = None,
        y: float | None = None,
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
    def add_dialog(self, headline: str, text: str, width: float, height: float) -> Node:
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
    ) -> tuple[Node, Node | None, list[Node]]:
        """A real MD3 top app bar, the *Small* variant. Returns `(bar,
        leading, trailing)`: `leading` is `None` unless `leading_icon`
        was given; `trailing` is one real, independently
        `enable_interaction()`-able `Node` per entry in
        `trailing_icons`, empty if none. `width` defaults to the
        window's own full width.
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
    def add_popover(self, subhead: str, text: str, width: float, height: float) -> Node:
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
        text: str,
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
        on: bool = False,
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
