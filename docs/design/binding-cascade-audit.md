# Binding and cascade audit

The declarative layer leaves `tre` in 0.3.5 (M98). Before it goes, this page
records how its two semantic cores actually behave — the `{{ }}` binding
evaluator (`engine-spec/src/binding.rs`) and the style cascade
(`engine-spec/src/cascade.rs`, `resolve_style_layered`) — wherever that
differs from what their doc comments say, or from what a reader expecting
Python would assume. A framework porting either one can then decide, finding
by finding, whether to match `tre` or fix it.

Every finding is pinned by a test, named in brackets: `b…` and `c…` in
`crates/engine-spec/tests/audit.rs`, `p…` in `tests/test_binding_audit.py`.
`tre` doesn't fix these — M98 removes the code — so the tests hold the
record true until then.

## Doc comments that are wrong

| Where | The doc says | What happens |
| --- | --- | --- |
| `StyleRule` | A rule is "a selector (`kind`/`classes`/`id`, any or none of them set)" — read naturally, all of them must match. | The fields are never combined. A rule naming a kind and classes applies at the kind tier to every widget of that kind, whatever its classes, and at the class tier to every widget with those classes, whatever its kind. A rule naming an id and a kind styles every widget of that kind. [`c1`, `c2`] |
| `cascade.rs` | "More classes beat fewer." | More class *names* beat fewer: a rule listing `[a, a, a]` outranks `[a, b]`. [`c4`] |
| `cascade.rs` | Styles are "resolved once, when a view is loaded or reconciled." | Also on every `View.set_theme`, `View.set_stylesheet`, and `View.poll_reload` — each patches every node through the cascade again. Still never per frame. |
| `binding.rs` | The grammar covers "arithmetic" and "comparison." | Only `+` mixes an int and a float; `-`, `*`, `/`, and every ordering comparison between them fail. There is no unary minus. [`b1`, `b7`] |
| `view.rs`, `attach_bindings_and_handlers` | "Only these three real event kinds are wired today." | Six are: `on_click`, `on_hover_enter`, `on_hover_exit`, `on_change`, `on_focus_enter`, `on_focus_exit`. Any other event name is accepted — its method is checked to exist — and never fires, so a misspelled event fails silently. [`p5`] |
| `binding.rs`, `Value` | A Python value that isn't an `int`, `float`, `str`, or `bool` stays an opaque handle. | `engine-py` converts anything `float()` accepts into a float, and an `int` too big for 64 bits arrives as a float, losing precision. [`p6`] |

## The binding grammar

Where `{{ }}` expressions differ from the same text read as Python:

| Expression | `tre` | Python |
| --- | --- | --- |
| `n - 0.5`, `n * 0.5`, `n / 0.5`, `n < 0.5`, with an int `n` [`b1`] | error | works |
| `1 == 1.0`, `True == 1` [`b2`] | `False` — equality is by type | `True` |
| `1 / 0` [`b3`] | "unsupported operation" error | `ZeroDivisionError` |
| `1.0 / 0.0` [`b3`] | `inf` | `ZeroDivisionError` |
| `3 / 2` [`b3`] | `1.5` | `1.5` — both true division |
| Integers [`b4`] | 64-bit: a larger literal is a parse error; overflow panics in a debug build and wraps in a release build (the published wheels) | unbounded |
| `'a' < 'b'`, `'a' * 2` [`b5`] | error | works |
| `True + 1`, `True < 2` [`b6`] | error — booleans aren't numbers | works |
| `-1`, `1 < n < 5`, `%`, `//`, `**`, `.5`, `x if c else y`, list and dict literals, calls with arguments or bare calls, string escapes [`b7`] | parse error | works |
| `None` [`b8`] | looked up on the view model like any name | a literal |
| `a and b`, `a or b` [`b8`] | return an operand, as in Python | the same |
| `"Count: {{ n }}"` [`b9`] | rejected — a binding is the whole value, never interpolated into text | — |

What works as Python does: attribute access, indexing, zero-argument method
calls (`clicks.get()`), `+` on two strings, `==`/`!=` on anything,
`not`/`and`/`or` with Python truthiness, parentheses, and `True`/`False`.

## Applying bindings

How a `View` applies a binding's value and keeps it current:

- **A `text` binding must be a string** [`p1`]. `{{ clicks.get() }}` on a
  `text` fails — and the grammar has no way to convert (`b5`, `b7`), so a
  number reaches a label only through a view-model method or `Computed` that
  returns a string.
- **A binding subscribes to the Signals its first evaluation read, once**
  [`p2`]. It doesn't re-track on later evaluations: in
  `{{ gate.get() and level.get() }}`, if `gate` starts falsy, `level` is never
  read the first time, and later changes to `level` never update the node —
  even once `gate` is truthy.
- **A failing re-evaluation raises from the `Signal.set()` that triggered
  it** [`p3`], after the Signal has already taken the new value.
- **One failing binding stops the rest** [`p4`]: the Signal's subscribers
  after it are never notified, so other nodes bound to the same Signal keep
  their stale values.

## The cascade

`resolve_style_layered` merges up to three stylesheets — default theme,
custom theme, the app's — then the widget's inline `style:`. Its behavior:

- **Each layer cascades on its own** [`c5`], and a higher layer always wins.
  A default theme's `id:` rule loses to an app stylesheet's selector-less
  baseline rule; specificity only counts within one layer.
- **Within a layer**, rules apply in tiers — baseline (no selector), `kind:`,
  `classes:`, `id:` — and a later tier wins each field it sets. Fields merge
  one by one, so a field no later tier sets survives from an earlier one.
- **Within a tier, the later rule wins** [`c3`]. Class rules are ordered by
  how many class names they list [`c4`], then by position.
- **Selector fields aren't combined** [`c1`, `c2`] — see the first table.

## For the port

Each is the framework's call; the tests show exactly what `tre` does today.
The ones most likely to surprise its users: compound selectors (`c1`, `c2`),
dependencies captured once (`p2`), one failing binding blocking others
(`p4`), silently ignored event names (`p5`), and the missing int–float
arithmetic and text conversion (`b1`, `p1`).
