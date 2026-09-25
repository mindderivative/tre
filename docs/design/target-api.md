# Target API (M93, proposed)

!!! warning "Design proposal — not implemented"
    This page specifies where `tre` is heading under the approved
    building-block program (M93–M103). Nothing here exists yet except
    where marked **(exists)**. Revision 2 incorporates Tesserae's review
    (items 1–15, each agreed by the project owner). M94 onward implements
    it only after the owner approves this revision.

## Purpose

`tre` becomes a small engine that provides **building blocks** — a node
tree, layout, painting, text, animation, input, accessibility, layers,
and the event loop — and nothing a framework can build from them.
Declarative views, the MD3 component catalog, and MD3 theming move to
the framework (Tesserae). Every name is chosen so a framework author can
tell at a glance what it is.

Decisions D1–D11 (approved) are in `BUILD_TRACKER.md`'s Program section.
This page adds design choices **R1–R12** for review.

Three principles run through every section:

- **No hidden theme (D7).** Every color `tre` paints is a property. Where
  `tre` draws nothing — focus rings, scrims — the spec says so.
- **One name per concept**, across node kinds, properties, and events.
- **Readback is exact.** `get` returns precisely the value `set` stored.

## Revision 2: Tesserae's review

| Item | Change | Section | Lands in |
| --- | --- | --- | --- |
| 1 | `click`/`secondary_click` bubble; `pointer_enter`/`pointer_leave` are subtree events | [Events](#events) | M94 |
| 2 | `a11y_action` event; `value_min`/`_max`/`_step`, `level`, `live`, `a11y_hidden`; full role list | [Properties](#properties), [Events](#events) | M94 |
| 3 | `corner_radius` takes a 4-tuple and animates; `shadows` list replaces `shadow_*` | [Properties](#properties) | M95 |
| 4 | Every painted color is a property; no focus ring or scrim | [Painting rules](#painting-rules) | M95 |
| 5 | Animate from the current value; `stop_animation`; `get` vs `get_target` | [`set`, `get`, `animate`](#set-get-animate) | M95 |
| 6 | `flex_wrap`, `align_self`, `"auto"`/percentages, `aspect_ratio`; stroke inside; group opacity | [Properties](#properties) | M96 |
| 7 | Reading `layout_*` runs pending layout, even inside `batch` | [Properties](#properties) | M96 |
| 8 | `max_lines`, ellipsis, `wrap`, `letter_spacing`, `font_style` | [Properties](#properties) | M96 |
| 9 | Layer focus trap and restore, stacking, flip/fit placement, key-event scope | [Layers](#layers) | M96 |
| 10 | `close_requested` (cancellable), `closed`, `scale_factor`, title setter | [Window](#window) | M94 |
| 11 | SVG `d` syntax plus `view_box`; morphing requirements | [Path data](#path-data) | M95 |
| 12 | `window.advance(ms)` for deterministic time | [Window](#window) | M96 |
| 13 | Event `input` (was `text_input`); `change` is text-only; `Event.target` + `current` | [Events](#events), [Event](#event) | M94 |
| 14 | Lifetime of detached nodes; drop and destroy safe off-thread (issue #10) | [Lifetime](#lifetime) | M96 |
| 15 | Atomic `set`; unknown names raise listing valid ones; `window.create(kind, **props)` | [`set`, `get`, `animate`](#set-get-animate), R9 | M96 |
| (b) | Focus-within: `focus`/`blur` bubble; capture and propagation | R10, [Events](#events) | M94 |
| (c) | Colors interpolate in sRGB; readback is the exact tuple | R4 | M95 |

## Naming convention

| Rule | Examples |
| --- | --- |
| **Methods are verb phrases**: `create` makes a detached node, `add_`/`insert_`/`remove` change structure, `set`/`get` read and write properties, `show_`/`hide_` change layer visibility | `create`, `add_child`, `insert_child`, `set`, `get`, `show_layer` |
| **Properties are nouns**, spelled identically in `set`, `get`, `animate`, and event payloads | `width`, `fill`, `corner_radius`, `opacity` |
| **Events are named for what happened**, with no `on_` prefix; registered with `on(event, handler)` | `click`, `pointer_down`, `key_down`, `focus` |
| **One name per concept**, between events and node kinds too | the typing event is `input`, not `text_input` (the kind) |
| **Booleans read as adjectives or states** | `visible`, `focusable`, `obscured`, `multiline` |
| **Units:** lengths are logical pixels with no suffix; durations end in `_ms`; angles in `_deg`; fractions are `0.0`–`1.0`; percentages are strings | `duration_ms`, `rotation_deg`, `"50%"` |
| **Enum values are lowercase `snake_case` strings** | `"horizontal"`, `"flex_start"`, `"ellipsis"` |
| **No abbreviations** beyond universal ones, and `a11y_` where a plain word would be ambiguous | `rgba`, `id`, `a11y_action`, `a11y_hidden` |
| **Colors are `(r, g, b, a)` tuples of ints 0–255**; parsing hex or theme names is the framework's job (R4) | `(0x67, 0x50, 0xA4, 0xFF)` |

## Design choices for review

R1–R8 are from revision 1; Tesserae's review agreed with all eight and
refined R3, R4, R5, and R8 (folded in below). R9–R12 are new in
revision 2.

| | Choice | Recommendation |
| --- | --- | --- |
| R1 | Paint naming | Every node paints `fill`, with optional `stroke_color`/`stroke_width`, replacing `background`, `foreground`, `border_color`, `border_width`. Renames names M90 introduced, once, in 0.3.5 |
| R2 | Creating nodes | Creation always makes a detached node; attach it with `add_child`/`insert_child`. No create-and-attach shortcuts |
| R3 | Propagation | Pointer, wheel, key, `click`, `secondary_click`, `focus`, `blur`, `input`, and `a11y_action` bubble from the target; `event.stop()` ends it. `pointer_enter`/`pointer_leave` are non-bubbling **subtree** events. `change`, `scroll`, `dismiss` don't bubble |
| R4 | Colors | Tuples only; `animate` interpolates them component-wise in sRGB; `get` returns exactly the tuple that was set |
| R5 | Remove vs destroy | `remove()` detaches and keeps the node alive; `destroy()` frees it; a detached node nothing refers to is freed automatically ([Lifetime](#lifetime)) |
| R6 | Setting properties | One atomic `node.set(**props)` |
| R7 | Headless testing (D9) | One `window.simulate(...)`, plus `window.advance(ms)` for deterministic time |
| R8 | Tab order | `tab_index`: a sort key within a focus scope; default is tree order; `-1` means focusable only programmatically; each open layer is its own scope |
| R9 | One creation entry point (item 15) | **`window.create(kind, **props)` only**, with no `create_box`/`create_text`/… beside it, since two ways to create would be duplicates. Every creation argument becomes a property (`create("text", text="Hi")`). That includes an image's pixels, so `push_frame`/`set_pixels` folds into atomic `set(rgba=..., pixel_width=..., pixel_height=...)` |
| R10 | Focus-within (answer b) | `focus` and `blur` **bubble**, with `event.target` naming the node that actually gained or lost focus; no separate `focus_in`/`focus_out` events |
| R11 | Typing event name (item 13) | `input` |
| R12 | Scrims and focus rings | `tre` draws neither. A modal's scrim is a window-sized `box` the framework puts in the layer; focus indication is drawn by the framework from `focus`/`blur` |

## Classes and functions

| Name | Role | Status |
| --- | --- | --- |
| `App` | `App()`, `add_window(window)`, `run(max_frames=None)`, `thread_handle()` | exists, unchanged |
| `LoopHandle` | `call_soon(fn)`, thread-safe | exists, unchanged |
| `Window` | Owns a node tree: creation, content, layers, window properties and events, batching, time, text measurement, clipboard, docking, `simulate` | reshaped |
| `Node` | A handle to one node: `set`, `get`, `get_target`, `animate`, `stop_animation`, `on`/`off`, structure, focus, pointer capture | reshaped |
| `Event` | The payload every handler receives | extended |
| `Painter` | The drawing surface of a `canvas` node's `draw` callback (was `CanvasContext`) | renamed, extended |
| `register_font(data)` | Registers font bytes; returns family names | exists, unchanged |
| `MONOSPACE_FONT_FAMILY` | The bundled monospace family name | exists, unchanged |

Removed: `View`, `Component`, `Theme`, `Signal`, `Computed`, `Effect`,
`ViewModel`, `batch`, `untrack` (D5, D7).

## Node kinds

Created with `window.create(kind, **props)` (R9). `window.root` is the
window's root `box`.

| Kind | Required properties | What it is |
| --- | --- | --- |
| `"box"` | none | A layout container painting an optional fill, stroke, corners, and shadows. Replaces `Rect` and `Container` (D3) |
| `"text"` | `text` | Shaped, non-editable text |
| `"text_input"` | none | Editable text: single or multi-line, selection, clipboard, IME, syntax spans, folding, whitespace glyphs, password obscuring (D2). Replaces `TextField` and `add_code_editor` |
| `"image"` | `rgba`, `pixel_width`, `pixel_height` | Decoded RGBA8 pixels; video is repeated `set(rgba=..., ...)` (D6) |
| `"path"` | `data` | A vector path: fill, stroke, trim, morph. Replaces `Icon` (D4) |
| `"canvas"` | `draw` | Immediate-mode drawing through a `Painter`; `node.redraw()` requests a new frame |
| `"scroll_view"` | none | Clips and scrolls exactly one child |
| `"virtual_list"` | `item_count`, `materialize` | Materializes only the visible items |
| `"terminal"` | `shell`, `cols`, `rows` | A PTY-backed terminal emulator (D1) |

## Node

### `set`, `get`, `animate`

- **`node.set(**props)`** is **atomic** (R6): every property is validated
  before any is applied. An unknown property, one not valid for this
  kind, or a bad value raises `ValueError` naming the property, and for an
  unknown name it lists the valid ones for this kind. Nothing changes on
  error.
- **`node.get(name)`** returns the **current** value; mid-animation, that's
  the value on screen. **`node.get_target(name)`** returns the value an
  animation is heading to, equal to `get` when idle. A reconciler diffs
  against it.
- **`node.animate(name, to, duration_ms, easing="linear",
  on_complete=None)`** eases an animatable (A) property **from its
  current value**, so re-animating mid-flight never jumps. `on_complete`
  runs once, when the target is reached; an animation replaced by another
  doesn't call it.
- **`node.stop_animation(name)`** stops at the current value.
- Easing is `"linear"` or a cubic bezier `(x1, y1, x2, y2)`; the MD3
  named curves become bezier values in the framework (M95).

### Properties

| Group | Properties |
| --- | --- |
| **Size and position** | `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`: each a number, `"auto"`, or a percentage string. `aspect_ratio`; `position` (`"relative"`, `"absolute"`), `x`, `y` |
| **Flex layout** (`box`) | `flex_direction`, `flex_wrap` (`"no_wrap"`, `"wrap"`), `align_items`, `justify_content`, `gap`, `padding`, `padding_top`/`_right`/`_bottom`/`_left` |
| **As a flex child** (every kind) | `flex_grow`, `flex_shrink`, `flex_basis`, `align_self`, `margin`, `margin_top`/`_right`/`_bottom`/`_left` |
| **Paint** (R1) | `fill` (A), `stroke_color` (A), `stroke_width` (A), `corner_radius` (A): one number, or `(top_left, top_right, bottom_right, bottom_left)`. `opacity` (A) |
| **Shadows** (M95, replaces `elevation`) | `shadows` (A): a list of `(color, offset_x, offset_y, blur, spread)`; MD3's two-shadow elevation is a two-item list |
| **Transform** | `translate_x` (A), `translate_y` (A), `scale` (A), `rotation_deg` (A) |
| **Visibility, clipping, order** | `visible`, `clip_children`, `z_index` |
| **Text** (`text`, `text_input`) | `text`, `font_family`, `font_weight`, `font_style` (`"normal"`, `"italic"`), `font_size`, `line_height`, `letter_spacing`, `text_align`, `wrap` (`"word"`, `"none"`), `max_lines`, `overflow` (`"clip"`, `"ellipsis"`) |
| **Text input** | `multiline`, `obscured`, `placeholder`, `placeholder_fill`, `caret_color`, `selection_fill`, `selection` (`(start, end)` byte offsets), `show_whitespace`, `syntax_spans`, `folded_ranges` |
| **Path** | `data` (A: animating it morphs, M95), `view_box`, `trim_start` (A), `trim_end` (A) |
| **Image** | `rgba`, `pixel_width`, `pixel_height` (set together), `fit` (`"cover"`, `"contain"`, `"fill"`) |
| **Scroll view** | `orientation`, `scroll_offset` (A: needed to animate carousel snapping), `scrollbar_fill`, `scrollbar_width` |
| **Virtual list** | `item_count`, `materialize`, `item_extent`, `size_hint` |
| **Terminal** | `cols`, `rows`, `scrollback_lines`, `font_size`, `palette` (the 16 ANSI colors plus foreground, background, cursor, and selection), `selection` (`(start_row, start_col, end_row, end_col)`) |
| **Interaction** | `focusable`, `tab_index` (R8), `cursor` (`"default"`, `"pointer"`, `"text"`, `"grab"`, …), `hit_testable` |
| **Accessibility** | `role`, `label`, `value`, `value_min`, `value_max`, `value_step`, `checked`, `selected`, `expanded`, `disabled`, `level` (headings), `live` (`"off"`, `"polite"`, `"assertive"`), `a11y_hidden` |

Accessibility roles: `button`, `checkbox`, `radio`, `switch`, `slider`,
`progressbar`, `link`, `textbox`, `tab`, `tablist`, `tabpanel`, `menu`,
`menuitem`, `dialog`, `alert`, `list`, `listitem`, `tree`, `treeitem`,
`heading`, `img`, `group`, `none`.

**Read-only via `get`:** `focused`, `layer_placement` (for a shown layer),
and `layout_x`, `layout_y`, `layout_width`, `layout_height`, the computed
box for anchoring and drag math. Reading a `layout_*` value runs any
pending layout first, even inside `window.batch()` (layout is computed,
painting stays deferred), so the value always matches the current tree.

#### Painting rules

- The colors `tre` paints are exactly the color properties in the table
  above: `fill`, `stroke_color`, `shadows`, `placeholder_fill`,
  `caret_color`, `selection_fill`, `scrollbar_fill`, and `palette`.
  Nothing has a hidden default theme color. `tre` draws no focus ring and
  no scrim (R12).
- `stroke_width` is paint-only and never changes layout; the stroke sits
  inside the node's box.
- `opacity` is group opacity: it fades the node and its subtree as one
  layer.

### Structure

| Method | Does |
| --- | --- |
| `add_child(child)` | Appends `child`; moves it if already attached elsewhere |
| `insert_child(index, child)` | Inserts at `index`; moves it if already attached (the keyed-reorder primitive) |
| `remove()` | Detaches this node; it stays alive and reattachable (R5) |
| `destroy()` | Frees this node and its subtree |
| `children()` | This node's children, in order |
| `parent()` | Its parent, or `None` |

### Lifetime

- A node attached to a window lives while it's attached.
- A detached node (never attached, or `remove()`d) lives while any Python
  `Node` handle to it exists. When the last handle goes, the node is
  destroyed automatically, so a forgotten `destroy()` never leaks.
- Dropping handles and calling `destroy()` are safe from any thread: `tre`
  hands the actual free to the event-loop thread. This covers Python's
  cyclic garbage collector freeing handles off-thread
  ([issue #10](https://github.com/mindderivative/tre/issues/10)).

### Events

`node.on(event, handler)` registers, `node.off(event)` removes. A handler
receives an `Event` (below), or nothing if it takes no parameters.

| Event | Fired when | Bubbles (R3) |
| --- | --- | --- |
| `pointer_enter`, `pointer_leave` | The pointer enters or leaves the node's **subtree**; not when it moves between the node and its descendants | no |
| `pointer_down`, `pointer_move`, `pointer_up` | Button press, pointer movement, button release | yes |
| `click` | Primary press and release on the same node, or keyboard activation | yes |
| `secondary_click` | Secondary-button click (context menus) | yes |
| `wheel` | Wheel or trackpad scroll | yes |
| `key_down`, `key_up` | A key, with modifiers, to the focused node | yes |
| `input` | Committed text arrives for the focused `text_input` (R11) | yes |
| `focus`, `blur` | A node gains or loses keyboard focus; `event.target` is that node (R10) | yes |
| `change` | A `text_input`'s text was changed by the user | no |
| `scroll` | A `scroll_view`'s offset changed | no |
| `dismiss` | A layer was dismissed by an outside click or Escape | no |
| `a11y_action` | An assistive technology invoked `action`: `"activate"`, `"increment"`, `"decrement"`, `"expand"`, `"collapse"`, `"dismiss"`, `"scroll_into_view"`, or `"set_value"` | yes |

**Pointer capture:** `node.capture_pointer()` during `pointer_down`
routes every later pointer event to this node until `pointer_up` or
`release_pointer()`. While captured, the capturing node is the target,
and events still bubble from it.

**Focus:** `node.focus()` moves focus to the node.

### Event

| Field | Present for |
| --- | --- |
| `type` | every event (was `kind`) |
| `target` | every event: the node the event is about (was `node`) |
| `current` | bubbling events: the node whose handler is running |
| `x`, `y` | pointer and wheel events, node-local |
| `window_x`, `window_y` | pointer and wheel events |
| `button` | pointer events (`"primary"`, `"secondary"`, `"middle"`) |
| `delta_x`, `delta_y` | `wheel`, `scroll` |
| `key` | `key_down`, `key_up`: full key names (`"a"`, `"enter"`, `"arrow_left"`, `"f5"`, …) |
| `shift`, `ctrl`, `alt`, `meta` | pointer and key events |
| `text` | `input` |
| `old_value`, `new_value` | `change` |
| `action`, `value` | `a11y_action` (`value` for `"set_value"`) |
| `stop()` | bubbling events: ends propagation |
| `cancel()` | cancellable events (`close_requested`): prevents the default |

## Window

| Group | API |
| --- | --- |
| **Construction** | `Window(width, height, title)` **(exists)** |
| **Nodes** | `root`; `create(kind, **props)` (R9) |
| **Content** | `set_content(node)`: show any kept-alive subtree as the window's content (replaces `show_view`) |
| **Properties** | `set(title=...)`; read-only `get("width")`, `get("height")`, `get("scale_factor")` |
| **Events** | `resize` (`width`, `height`); `color_scheme` (`dark`), since `tre` no longer themes anything itself (D7); `scale_factor` (`scale_factor`); `close_requested` (cancellable with `event.cancel()`); `closed`; `dock_target`, `dock_drop` |
| **Layers** (M96) | [below](#layers) |
| **Batching** (M96) | `with window.batch():` defers layout and paint until the block ends; reading `layout_*` still runs layout |
| **Time** (R7, M96) | `advance(ms)`: headless, moves animations and layout forward by exactly `ms`, so animated widgets are testable on CI, where `App.run()` renders no frames |
| **Text measurement** (M96) | `measure_text(text, font_family, font_size, font_weight=400, font_style="normal", letter_spacing=0, line_height=None, max_width=None)` → `(width, height)`; replaces `get_monospace_cell_size` |
| **Clipboard** | `read_clipboard()` → `str` or `None`; `write_clipboard(text)` → `bool`. Text inputs handle Ctrl+C/X/V/A themselves and never copy from an `obscured` input |
| **Docking** (D10) | [below](#docking-bare-bones-d10) |
| **Testing** (R7) | `simulate(event, node=None, **fields)` for every event; `resize(width, height)` **(exists)** |

### Layers

`show_layer(node, anchor=None, placement="below", modal=False,
dismissible=True)` and `hide_layer(node)` replace the six open/close
pairs, `build_menu`, and `set_context_menu`.

- **Stacking:** layers stack in the order shown, newest on top.
- **Placement:** `placement` is the preferred side of `anchor`
  (`"below"`, `"above"`, `"start"`, `"end"`). Near a window edge, `tre`
  flips or shifts the layer to fit, and reports the final side as
  `node.get("layer_placement")`.
- **Modal:** input outside the layer is blocked and focus is trapped
  inside it; `hide_layer` restores focus to the node that held it before
  the layer opened. `tre` draws no scrim (R12).
- **Focus and keys:** each open layer is its own focus scope (R8). Key
  events inside a layer bubble to the layer's node and stop there, never
  reaching the tree underneath.
- **Dismissal:** with `dismissible=True`, an outside click or Escape
  fires `dismiss` on the layer's node; the framework decides whether to
  call `hide_layer`.

### Docking (bare bones, D10)

| API | Does |
| --- | --- |
| `add_dock_zone(side, box, size)` | Makes `box` the container for `side` (`"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`) **(exists)** |
| `dock_panel(side, panel)` | Docks `panel` into a zone **(exists)** |
| `set_active_panel(side, index)` | Which docked panel in a zone is shown (was `set_active_tab`) |
| `start_panel_drag(panel)` | Starts dragging `panel`; call it from the framework's own drag handle's `pointer_down` |
| window event `dock_target` | While dragging: the zone under the pointer, or `None`; the framework draws its own highlight |
| window event `dock_drop` | The drag ended: `panel` and the `side` it was docked into, or `None` if cancelled |

Removed: `set_dock_handle`, `set_drop_zone_highlight` (presentation),
`drag_panel_over`, `drop_panel_at` (covered by `simulate`), `build_shell`.

## Path data

`data` uses SVG path syntax, the `d` attribute: `M`, `L`, `H`, `V`, `C`,
`S`, `Q`, `T`, `A`, `Z`, absolute and relative. Its coordinates are in
`view_box` `(min_x, min_y, width, height)`, which scales to the node's
layout box. A Material Symbols SVG drops straight in: pass its `d` and
its `viewBox`.

**Morphing:** animating `data` from one path to another works for any
two closed paths, or any two open ones, because `tre` resamples both to a
common set of points. It looks most faithful when both paths wind the
same way and start at corresponding points. An open path doesn't morph
into a closed one.

## Painter (canvas)

`fill_rect`, `fill_circle`, `stroke_path` **(exist)**; `fill_path`,
`draw_text` (new); `set_hit_test_circle`, `set_hit_test_path` **(exist)**.
Same color rule (R4). `color` stays the parameter name here because a
painter call has no node to carry a `fill`.

## Rebuilding the MD3 widgets from building blocks

Every removed widget kind maps to building blocks; M94–M96 add exactly
the ones not yet present.

| Widget | Built from |
| --- | --- |
| Buttons | `box` + `text` (+ icon `path`); `click` bubbles from the label to the button; `role="button"` |
| Checkbox | `box` + check `path` with animated `trim_end`; `click`; `role="checkbox"`, `checked` |
| Radio button | `box` with `corner_radius`; inner dot animated by `scale`; `click`; `role="radio"`, `selected` |
| Switch | `box` track + thumb `box` animated by `translate_x`; `click`, plus pointer capture for dragging; `role="switch"`, `checked` |
| Slider | track and thumb `box`es; `pointer_down` + `capture_pointer` + `pointer_move`; `key_down` for arrows; `a11y_action` `"increment"`/`"decrement"`/`"set_value"`; `role="slider"`, `value`, `value_min`/`_max`/`_step` |
| Linear and circular progress | `path` with animated `trim_start`/`trim_end`; `role="progressbar"`, `value`, `value_min`/`_max` |
| Loading indicator | `path` morphing between shapes by animating `data`, re-triggered from `on_complete` |
| Time picker dial | `path`s, `text`, pointer drag with capture |
| Carousel | `scroll_view` + drag + animated `scroll_offset` for snapping |
| Splitter | `box` handle + capture drag + setting neighbours' `width`/`flex_basis`; `cursor="grab"` |
| Link | `text` + `click`; `role="link"`, `cursor="pointer"` |
| Ripple / state layer (D8) | `box` with `clip_children` + a circle `path` animated by `scale` and `opacity`, positioned from `pointer_down`'s `x`/`y`; hover from the `pointer_enter`/`pointer_leave` subtree events; `a11y_hidden` |
| Elevation | `shadows`: MD3's key and ambient shadow for each level, supplied by the framework |
| Sheets, split buttons | asymmetric `corner_radius` 4-tuples |
| Text fields, search bars | `text_input` with `placeholder`, `placeholder_fill`, `caret_color`, `selection_fill`, `obscured` for passwords; the wrapping widget tracks focus through bubbling `focus`/`blur` |
| Chip groups | `flex_wrap="wrap"` |
| List items, app-bar titles | `max_lines` + `overflow="ellipsis"` |
| Dialogs, menus, tooltips, snackbars, sheets, drawers, context menus | `show_layer` (`modal`, focus trap, `dismissible`, `anchor`, flip/fit); a scrim `box`; `dismiss`; `live="polite"` for snackbars; `secondary_click` for context menus |
| MD3 theme (D7) | the framework computes colors (HCT) and `set`s them; `color_scheme` window event for OS light/dark; `letter_spacing` and `font_style` for the type scale |

## Tesserae's needs

| Need | Covered by |
| --- | --- |
| Declarative layer, reconciler, binding evaluator, reactivity | Move to Tesserae (D5, M98) |
| Reorder and move children | `insert_child` |
| Show a kept-alive subtree | `set_content`, `remove` (R5) |
| Set and read every property | atomic `set` (R6), `get`/`get_target`, exact color readback |
| Batched updates | `window.batch()` |
| Resize, OS light/dark, scale factor, close | window events |
| Focus, focus-within, Tab order | bubbling `focus`/`blur` (R10), `tab_index` (R8) |
| Flex layout stays | Flex properties, including `flex_wrap`, `align_self`, percentages, and `aspect_ratio` |
| Text measurement | `measure_text` |
| Stable, safe node handles | `Node` handles **(exist)**; [Lifetime](#lifetime) |
| Clip, z-order, opacity | `clip_children`, `z_index`, group `opacity` |
| Deterministic time in tests | `window.advance(ms)` |
| Fonts, images, threads | `register_font`, `"image"` nodes, `App.thread_handle` **(exist)** |

## Migration table

Every current public name, and what it becomes.

### `Window` — creation

| Today | Target |
| --- | --- |
| `add_rect`, `Container` kind | `create("box")` |
| `add_text` | `create("text")` |
| `add_text_field`, `add_code_editor` | `create("text_input")` |
| `add_image_from_bytes`, `add_video` | `create("image")`; frames via `set(rgba=..., ...)` |
| `add_image(path)` | removed (D6) |
| `add_icon` | `create("path")` (icon data from the framework) |
| `add_canvas` | `create("canvas")` |
| `add_scroll_view` | `create("scroll_view")` |
| `add_virtual_list` | `create("virtual_list")` |
| `add_terminal` | `create("terminal")` |
| `add_dock_zone` | unchanged |
| `add_button`, `add_icon_button`, `add_fab`, `add_extended_fab`, `add_segmented_button`, `add_chip`, `add_menu_item`, `add_badge`, `add_linear_progress`, `add_circular_progress`, `add_loading_indicator`, `add_time_picker_dial`, `add_card`, `add_divider`, `add_tooltip`, `add_dialog`, `add_snackbar`, `add_side_sheet`, `add_navigation_rail`, `add_navigation_drawer`, `add_top_app_bar`, `add_toolbar`, `add_split_button`, `add_button_group`, `add_tabs`, `add_search_bar`, `add_search_view`, `add_list_item`, `add_list`, `add_accordion_header`, `add_tree_node`, `add_date_picker_day`, `add_time_input_field`, `add_period_selector`, `add_popover`, `add_link`, `add_spin_box`, `add_pagination`, `add_status_bar`, `add_checkbox`, `add_radio_button`, `add_switch`, `add_slider`, `add_carousel`, `add_splitter`, `add_node_graph`, `add_graph_node` | framework |

### `Window` — everything else

| Today | Target |
| --- | --- |
| `from_view`, `show_view` | `set_content` |
| `theme`, `set_theme` | framework (D7) |
| `build_menu`, `open_menu`, `close_menu`, `open_dialog`, `close_dialog`, `open_snackbar`, `close_snackbar`, `open_side_sheet`, `close_side_sheet`, `open_navigation_drawer`, `close_navigation_drawer` | `show_layer`, `hide_layer` |
| `build_shell`, `begin_container_transform`, `end_container_transform` | framework |
| `get_monospace_cell_size` | `measure_text` |
| `resize_terminal` | `terminal.set(cols=, rows=)` |
| `copy_terminal_selection` | `terminal.get("selection")` + `write_clipboard` |
| `click`, `hover`, `focus`, `scroll`, `right_click`, `press_key`, `type_text`, `press_ctrl`, `copy`, `cut`, `paste`, `select_all` | `simulate` |
| `copy_to_system_clipboard`, `cut_to_system_clipboard`, `paste_from_system_clipboard` | `read_clipboard`, `write_clipboard`; text inputs handle the keys themselves |
| `resize` | unchanged |
| `dock_panel` | unchanged |
| `set_active_tab` | `set_active_panel` |
| `start_panel_drag(handle)` | `start_panel_drag(panel)` |
| `set_dock_handle`, `set_drop_zone_highlight`, `drag_panel_over`, `drop_panel_at` | removed; `dock_target`/`dock_drop` events and `simulate` |
| `set_virtual_list_window` | `virtual_list.set(...)`; exact property decided in M94 |
| `redraw_canvas(canvas)` | `canvas.redraw()` |

### `Node`

| Today | Target |
| --- | --- |
| `animate(property, to, duration_ms, on_complete)` | `animate(name, to, duration_ms, easing, on_complete)`; plus `stop_animation`, `get_target` |
| `get(property)` | `get(name)` |
| `set_layout(...)`, `set_text`, `set_checked`, `set_selected`, `set_clip_children`, `set_syntax_spans`, `set_folded_ranges`, `set_terminal_selection` | `set(...)` |
| `get_text`, `get_checked`, `get_selected`, `is_focused` | `get("text")`, `get("focused")`; checked/selected become framework state |
| `set_on_click`, `set_on_hover_enter`, `set_on_hover_exit`, `set_on_change`, `set_on_focus_enter`, `set_on_focus_exit` | `on("click")`, `on("pointer_enter")`, `on("pointer_leave")`, `on("change")`, `on("focus")`, `on("blur")` |
| `set_context_menu` | `on("secondary_click")` + `show_layer` |
| `enable_interaction` | framework (D8) |
| `add_child` | unchanged |
| `remove` | `remove` (now keeps the node alive, R5); `destroy` to free |
| `push_frame` | `set(rgba=..., pixel_width=..., pixel_height=...)` (R9) |
| `set_carousel_index`, `get_carousel_index`, `get_carousel_position`, `set_carousel_scroll`, `get_carousel_scroll`, `set_time_picker_dial_time`, `get_time_picker_dial_time`, `set_time_picker_dial_mode`, `get_time_picker_dial_mode` | framework |

### Properties

| Today | Target |
| --- | --- |
| `background`, `foreground` | `fill` (R1) |
| `border_color`, `border_width` | `stroke_color`, `stroke_width` (R1) |
| `corner_radii_override` | `corner_radius` 4-tuple |
| `elevation` | `shadows` |
| `transform` (tuple) | `translate_x`, `translate_y`, `scale` |
| `rotation` (icons) | `rotation_deg` (every node) |
| `shape` (MD3 shape library) | a `path`'s `data` |
| `check_progress`, `select_progress`, `toggle_progress`, `value` | framework-owned animation state |
| `opacity`, `corner_radius`, layout names | unchanged |

### Other

| Today | Target |
| --- | --- |
| `View` (`node`, `poll_reload`, `reconcile`, `set_stylesheet`, `set_theme`, `instantiate`, `click`, `hover`, `focus`, `right_click`), `Component` (`node`, `instantiate`, `remove`) | removed; the declarative layer moves to Tesserae (M98) |
| `Theme` (`role`, `is_set`, `shape`, `elevation`, `typography`) | removed; theming is the framework's (D7) |
| `Signal`, `Computed`, `Effect`, `ViewModel`, `batch`, `untrack` | Tesserae (D5) |
| `CanvasContext` | `Painter` |
| `Event.kind`, `Event.node` | `Event.type`, `Event.target` |
| MD3 named motion curves | cubic bezier values (framework) |
| `App`, `LoopHandle`, `register_font`, `MONOSPACE_FONT_FAMILY` | unchanged |
