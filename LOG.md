# Log: M15 Phase 3 — Selection & Two-Way Binding (§16.7)

Corresponds to `BUILD_TRACKER.md` M15 Phase 3, closing M15 entirely
(all 3 phases). Shift+arrow selection extension, real Backspace/
Delete/typing-with-selection, `TextField` declarable in `view.yaml`,
and real `two_way: text` binding sugar.

## Investigation before writing code

`dispatch_text_field_key` (M15 Phase 2) only ever took `key`, not
`shift` — `InputEvent::KeyPressed` already carries it, unused until
now. Real design question resolved before writing: a bare arrow key
pressed while a real selection is active must collapse to the
selection's own near edge (real desktop-editor behavior), not move one
more character from the focus end. `WidgetSpec.text: Option<TextSpec>`
already exists for `kind: Text` — confirmed via direct read, its four
fields are byte-for-byte what `TextFieldState::new` needs, so `kind:
TextField` reuses that same block rather than adding a third,
near-duplicate `WidgetSpec` field the way `checked`/`value` each
needed their own. `apply_binding_value`/`TwoWayCallback` already
special-case `"checked"`; `text` is the second non-numeric property,
and `engine_spec::Value::Str` was already real — both branches mirror
the existing shape exactly.

## What happened

`dispatch_text_field_key` widened to take `shift: bool`. `ArrowLeft`/
`ArrowRight`/`Home`/`End`: without `shift`, a real active selection
collapses to its near/far edge and clears; with `shift`, `selection_
anchor` seeds from the pre-move cursor the first time (left alone on
further extends), then the cursor moves normally. New shared `Tree::
delete_selection(state) -> bool`: deletes a real, non-collapsed
selection (`true`); a zero-width "selection" (`anchor == cursor`)
clears itself and reports `false`, a genuine no-op. `Backspace`/
`Delete`/`Space`/`TextInput` all call it first, replacing an active
selection instead of acting at a bare cursor.

`NodeKindSpec::TextField` added; `build.rs::node_kind_and_paint` gains
a matching arm requiring `style.background` + the reused `text:`
block, building `TextFieldState::new` from it. `apply_binding_value`
gains a `Value::Str` branch calling `set_text`; `TwoWayCallback::
__call__` gains a `"text"` branch calling `get_text`.

New `engine-core` tests (9, all passed first run): shift-arrow extends
the selection from the cursor; a bare arrow after a real selection
collapses to the correct near edge (moving left) and far edge (moving
right); shift+Home/End extend to the real edges; Backspace/Delete/
typing/Space each replace a real active selection instead of acting at
a bare cursor; a zero-width selection is treated as no selection at
all (still a genuine no-op). New `engine-spec` tests (4, one fixed
after a real assertion mistake caught by actually running it — "jane"
is 4 characters, not 5): `TextField` parses reusing the same `text:`
block `Text` already requires; `load_view` builds a real
`TextFieldState` from it, cursor seeded at the real content end;
missing background and missing `text:` each fail with the correct,
clear `MissingField` error.

New `tests/test_two_way_binding.py::test_two_way_text_field_writes_
the_signal_back_when_text_changes` (real round trip via `set_text`,
the only Python-reachable `Change` source for a `TextField` from a
`View` with no live window). New `tests/test_text_field.py` additions
(2 tests): shift+arrow selection via `Window.press_key(shift=True)`
followed by a real Backspace deleting the whole selected range;
typing over a real selection replaces it. Extended the existing
`examples/two_way_binding.py`/`.yaml` with a real `TextField` alongside
the existing Checkbox/Slider (a `name` `Signal`, both directions
proved), rather than a parallel duplicate example.

Full `cargo test --workspace --release` (`engine-core` 117, up from
108; `engine-spec` 37, up from 33)/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo fmt --check` all clean — every
prior test passed unmodified. `maturin develop --release` + full
`pytest tests/` (150 passed, up from 147, 1 skipped) and all
twenty-three examples confirmed clean.

M15 — TextField / Real Keyboard Text Entry is now fully complete: all
3 phases (state/paint, keyboard-driven editing, selection + two-way
binding) closed §16.7's own long-deferred component gap.
