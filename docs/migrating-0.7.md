# Upgrading to 0.7.x

0.7.0 changes one default: a text node with no size is the size of its text.
No name was removed or renamed, so code written for 0.6.x runs, but every
unsized text node lays out differently, and a layout that relied on the old zero
may move.

## 0.7.0

### Changed

- **An unsized text node measures to its content (#263).** Until 0.6.x a text
  node with no `width` or `height` was zero wide and zero tall, so a centred
  label wrapped one word to a line and an effect over it cropped the text.
  `width="fit-content"` and `height="fit-content"` (0.5.7) were the opt-in. Now
  unset, `"auto"` and `"fit-content"` are the same thing on a text node, on both
  sides: the size of its text.
- **A fit width is the text plus 2 pixels.** A box exactly as wide as the
  measured text can cut it off or wrap its last word. The extra pixels apply
  whenever a text node fits its width, including an explicit `"fit-content"`
  from 0.5.7, so a label you already opted in is 2 px wider. The height is
  exact. A long label is still narrowed to the space its parent offers and wraps
  there; the 2 pixels never make it overflow.
- **`get` reads `"fit-content"`.** `label.get("width")` and `get("height")` return
  `"fit-content"` for a text side that fits, where an unsized one read `"auto"`
  before. A box still reads `"auto"`.
- **`window.measure_text` is unchanged.** It returns the exact size with no extra
  pixels, so code that sizes text by hand from it keeps its numbers.

### What may move

- A text node in a `Row` takes its text's width instead of 0. Its height is still
  the row's when the row stretches.
- A text node in a centring or start-aligned box takes its text's size instead of
  0 by 0. In a stretching box it still fills the box's width, as in CSS.
- A passthrough effect over padded text now draws it whole.

### To get the old behaviour

Give the side a number: `width=0, height=0`, or any size you want. A percentage
or a `calc(...)` size also takes that side off fit.

```python
label = window.create("text", text="Hi", width=0, height=0)   # the old unsized text
```

### Check your project

Search for text nodes you create without a size and look at the ones inside a
`Row`, a centring box or an anchored layer. The tests that pinned the old zero
(for example a label asserted to be `layout_width == 0`) need the new rule.
