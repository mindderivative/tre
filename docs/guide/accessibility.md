# Accessibility

Every frame, Tesserae Engine builds an [AccessKit](https://github.com/AccessKit/accesskit)
tree from the node tree and hands it to the platform's screen reader. What
a node tells assistive technology is what you set on it: Tesserae Engine knows boxes
and paths, not buttons and switches, so a widget declares what it is.

## Role, name, and state

```python
save = window.create("box", width=96, height=40, corner_radius=20,
                     fill=(0x67, 0x50, 0xA4, 0xFF),
                     role="button", label="Save", focusable=True)
```

- `role` — what the node is: `"button"`, `"checkbox"`, `"switch"`,
  `"slider"`, `"textbox"`, `"menu"`, `"menuitem"`, `"dialog"`, `"tab"`,
  `"heading"`, `"list"`, `"listitem"`, and the rest listed on
  [`Node`](../reference/node.md#set-get-and-focus).
- `label` — the name read aloud. A text node's words aren't exposed on their
  own, so label the widget that shows them.
- State — `checked`, `selected`, `expanded`, `disabled`, and a heading's
  `level`. Keep them current as the widget changes: a switch sets
  `checked` each time it toggles. They are for assistive technology:
  `disabled` doesn't stop the node taking focus, clicks, or keys — the widget
  does that.
- Value — `value` (a string or number), with `value_min`, `value_max`, and
  `value_step` for a range such as a slider or progress bar.

A text input and a terminal are `"textbox"` from the start, and a text
input's value is its text.

## Focus

Screen readers follow keyboard focus, so everything a keyboard user can
operate should be `focusable=True` — see
[Focus and the keyboard](events-and-input.md#focus-and-the-keyboard). An
assistive technology's request to focus a node focuses it exactly as Tab
would, and `focus_visible` is `True` for it. However focus arrives — Tab,
a click, `node.focus()`, or an assistive technology — the scroll views
around the node scroll just enough to show it (0.4.2), so a screen reader
never lands on a node that's out of sight. See
[Revealing a node](nodes-and-layout.md#revealing-a-node).

## Actions

The actions a node offers follow from its role and state:

- focusable nodes offer focus;
- button-like roles (button, checkbox, radio, switch, link, menu item, tab,
  tree item) offer activation;
- a slider, or any node with a value range, offers increment, decrement, and
  set-value;
- a node with `expanded` set offers expand and collapse.

Activation arrives as an ordinary `click`, so a widget that handles clicks
already handles it. The rest arrive as `a11y_action`:

```python
volume = 0.5

def set_volume(value):
    global volume
    volume = round(min(1.0, max(0.0, value)), 2)
    slider.set(value=volume)          # keep what's announced current

def on_action(event):
    if event.action == "increment":
        set_volume(volume + 0.1)
    elif event.action == "decrement":
        set_volume(volume - 0.1)
    elif event.action == "set_value":
        set_volume(float(event.value))

slider.on("a11y_action", on_action)
```

`event.action` is `"increment"`, `"decrement"`, `"expand"`, `"collapse"`,
`"scroll_into_view"`, or `"set_value"` (with `event.value`). Tesserae Engine answers
`"scroll_into_view"` itself, after your listeners run: it scrolls every
scroll view around the node just enough to show it, as
[`node.scroll_into_view()`](nodes-and-layout.md#revealing-a-node) does, so a
widget needs no listener for it.

## Text, selection and links

A text node that is `selectable`, has a selection, or has a span with a `link`
(0.5.4) is exposed as a text container: if its `role` is still the default it
reads as a label, and its content is a run of text for each stretch between link
boundaries, with each link a `link` node holding its run and the span's `link`
string as its URL. A selection (`selection`, or the user's) is reported as a text
selection, so a screen reader can read it and move through the text, and it can
set the selection (`SetTextSelection`), which selects the text as the pointer
would. Following a link (the `click` action on the link node) fires the text
node's `link` event with `href`, as a mouse click does.

A run's bounds are the whole text node's box, and a wrapped paragraph is one run,
so a screen reader that places its cursor by position finds the node, not the
word. Plain text with none of these is unchanged: no children, and the role you
set. This is built and checked against AccessKit's own consumer; it has not been
tried with NVDA, VoiceOver or Orca.

## Announcing and hiding

`live="polite"` or `"assertive"` makes a node a live region: a screen reader
announces changes to it — a snackbar's message, a form's error. `a11y_hidden`
hides a node from assistive technology while it stays on screen, for purely
decorative parts; `visible=False` hides it from everyone.

## Testing

`window.simulate("a11y_action", node=..., action=..., value=...)` sends an
assistive request through the same path a screen reader's does, and every
property reads back with `get`:

```python
window.simulate("a11y_action", node=slider, action="set_value", value=0.75)
assert slider.get("value") == 0.75
```

An assistive technology's `scroll_into_view` request is testable the same
way: simulate it on a row, then read its scroll view's
`get("scroll_offset")`.
