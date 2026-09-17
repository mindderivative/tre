# Plan: M15 Phase 3 — Selection & Two-Way Binding (§16.7)

Corresponds to `BUILD_TRACKER.md` M15 Phase 3's own scoping, closing
M15 entirely: shift+arrow selection extension and a real Backspace/
Delete-with-selection; `NodeKindSpec::TextField` + `WidgetSpec`
declarability in `view.yaml`; real `two_way: text` binding sugar.

## Investigation before writing code

- `Tree::dispatch_text_field_key` (M15 Phase 2) only ever took `key`,
  not `shift` — confirmed by direct read — so it had no way to
  distinguish "move" from "extend." `InputEvent::KeyPressed` already
  carries `shift: bool`, unused by that method until now.
- Real design question, resolved before writing: does a bare (non-
  shift) arrow key, pressed while a real selection is active, move the
  cursor one more character from the focus end, or collapse to the
  selection's own near edge? Every real desktop text editor does the
  latter — decided to match that, not the simpler-but-wrong former.
- `WidgetSpec.text: Option<TextSpec>` already exists for `kind: Text`
  — confirmed via direct read, `TextSpec`'s own four fields (`content`/
  `font_family`/`font_weight`/`font_size`) are byte-for-byte what
  `TextFieldState::new` needs. `Checkbox`/`Slider` each needed a new
  dedicated `WidgetSpec` field (`checked`/`value`) because neither had
  a matching sibling block to reuse; `TextField` does, so `kind:
  TextField` reuses the same `text:` block `kind: Text` already
  requires, rather than adding a third, near-duplicate field.
- `apply_binding_value`/`TwoWayCallback` (M14 Phase 3) already
  special-case `"checked"` for the one non-numeric property that
  exists so far — `text` is the second, and `engine_spec::Value::Str`
  is already a real variant (confirmed via direct read), so both
  branches need only mirror the existing `"checked"` shape, not a new
  mechanism.

## Design

- `Tree::dispatch_text_field_key(field, key, shift)`: `ArrowLeft`/
  `ArrowRight` — if `!shift` and a real selection is active, collapse
  to its near edge and clear it; else (extending or no selection) seed
  `selection_anchor` from the pre-move cursor the first time `shift`
  is held, then move normally. `Home`/`End` mirror the same
  anchor-seed-then-jump shape. New shared `Tree::delete_selection
  (state) -> bool`: deletes a real, non-collapsed selection and
  returns `true`; a zero-width "selection" (`anchor == cursor`) both
  clears itself and reports `false` — a real no-op, not an empty
  delete. `Backspace`/`Delete`/`Space`/`TextInput` all call it first,
  replacing an active selection instead of acting at a bare cursor —
  real desktop-editor behavior, not `engine-core` inventing a special
  case per key.
- `NodeKindSpec::TextField` (unit variant); `build.rs::node_kind_and_
  paint` gains a matching arm requiring `style.background` + `text:`,
  building `TextFieldState::new` from the reused `TextSpec`.
- `apply_binding_value` gains a `Value::Str` branch calling `Node.
  set_text`; `TwoWayCallback::__call__` gains a `"text"` branch
  calling `Node.get_text`.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-core` tests (shift-arrow extends from the cursor; a bare
arrow after a real selection collapses to the correct near/far edge;
shift+Home/End extend to the real edges; Backspace/Delete/typing/Space
each replace a real active selection; a zero-width selection is
treated as no selection at all); new `engine-spec` tests (`TextField`
parses reusing the same `text:` block `Text` uses; `load_view` builds
a real `TextFieldState` from it; missing background/missing `text:`
each fail with a clear, real error); `maturin develop --release`; new
`tests/test_two_way_binding.py`/`test_text_field.py` coverage (a real
`two_way: text` round trip; shift+arrow selection + Backspace/typing
reaching the real focused field via `Window.press_key(shift=True)`);
extend the existing `examples/two_way_binding.py`/`.yaml` with a real
`TextField` alongside the existing Checkbox/Slider, rather than a
parallel duplicate; every example re-run; `LOG.md`/`BUILD_TRACKER.md`/
tracker artifact/commit/memory — closing M15 entirely (all 3 phases).
