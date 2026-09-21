# LOG — M44: Widen Live-Bindable Properties (`background` Color Bindings)

- User's own governing instruction: after M43 closed, asked "what do
  you recommend we look at next?" -- two real recommendations were
  given: (1) widen live-bindable properties beyond opacity/corner_
  radius/checked/text; (2) richer reactivity (Computed/Effect/batch/
  untrack). User replied "scope both 1 and 2, let's start with 1" --
  scoped both via a formal plan (`EnterPlanMode`/`ExitPlanMode`,
  `/home/phil/.claude/plans/reflective-sleeping-falcon.md`), implemented
  only item 1.
- Investigation, in order, all direct reads before writing any code:
  `crates/engine-py/src/node.rs`'s `Node::animate()` (already supports
  `background`/`transform`/`shape` as composite values, and numeric
  properties well beyond opacity/corner_radius, imperatively) and its
  own `extract_color` (only ever accepted an `(u8,u8,u8,u8)` tuple, no
  string parsing); `crates/engine-py/src/view.rs`'s `apply_binding_
  value` (the real, single gap: dispatched on `Value`'s own runtime
  type, not on `property` -- `Bool` always -> `set_checked`, `Str`
  always -> `set_text`, `Value::Handle` rejected outright);
  `crates/engine-spec/src/binding.rs`'s `Value` enum; `crates/engine-py/
  src/binding.rs`'s `PyViewModelResolver` (`to_value`/`store_handle`/
  `to_pyobject` -- confirmed the exact Handle round-trip mechanics,
  and that both real call sites in `view.rs` already hold a live
  `resolver` in scope when calling `apply_binding_value`); `crates/
  engine-spec/src/build.rs`'s `resolve_color` (the real static-YAML
  color parser precedent -- MD3 token first, else `peniko::color::
  parse_color`); confirmed `peniko` is already a direct `engine-py`
  dependency, already `use`d in `node.rs`, so string color parsing
  needed zero new dependencies.
- `crates/engine-py/src/binding.rs`: `PyViewModelResolver::to_pyobject`
  widened `private` -> `pub(crate)`, so `view.rs` can recover a
  `Value::Handle`'s real Python object -- the same "two real call sites
  justify widening" precedent `collect_bindings`/`collect_handlers`
  established for M43.
- `crates/engine-py/src/view.rs`: new, pure, GIL-free `fn
  parse_background_color(raw: &str) -> Result<(u8,u8,u8,u8), String>`
  -- deliberately `Result<_, String>`, not `PyResult`, so it needs no
  Python interpreter and gets a real, unconditional Rust `#[test]`
  rather than only indirect pytest coverage (the same GIL-needed/
  GIL-free test-surface split `View::new`'s own existing tests already
  established in this file). `apply_binding_value` rewritten:
  `checked`/`text` stay non-`animate()` special cases but are now
  gated by `property`, not `value`'s type (a binding declared on
  `checked` that resolves to the wrong shape is now a real, specific
  type-mismatch error); every other property forwards to `animate()`
  unchanged; a `Value::Str` resolved for `property == "background"`
  calls the new helper; a `Value::Handle` is recovered via `resolver.
  to_pyobject` and forwarded to `animate()` verbatim. Both real call
  sites (`BindingCallback::__call__`, `attach_bindings_and_handlers`)
  updated to pass `&resolver` -- both already constructed one
  immediately before calling `evaluate`, so this needed no lifetime
  restructuring, just one new parameter/argument at each site.
- **Real, incidental side effect, named not silently claimed as the
  headline feature:** because the `Value::Handle` recovery path is
  property-name-agnostic (it just forwards whatever real Python object
  it recovers to `animate()`), `transform`/`shape` bindings are now
  reachable too, not just `background` -- `animate()` already validates
  each one's own expected shape.
- New Rust unit tests (`view.rs`'s own `#[cfg(test)] mod tests`, +4):
  `parse_background_color_accepts_a_hex_string`,
  `parse_background_color_accepts_hex_with_alpha`,
  `parse_background_color_accepts_a_css_named_color`,
  `parse_background_color_rejects_nonsense` (asserts the real bad input
  string appears in the error message). All passed on the first run
  after fixing one design choice mid-implementation: the helper
  originally returned `PyResult`, but constructing/`Display`-ing a
  `PyErr` from a GIL-free `#[test]` (no `pyo3::prepare_freethreaded_
  python()` called, matching this module's own established no-GIL test
  convention) raised a real doubt about whether `PyErr::to_string()`
  itself needs the GIL -- resolved by keeping the helper's own return
  type 100% pyo3-free (`Result<_, String>`) and moving the `PyValueError`
  construction to `apply_binding_value`'s own call site, where a `py:
  Python<'_>` token is already in scope. Cleaner separation, not just a
  workaround -- matches this crate's own stated "pure Rust logic gets a
  real Rust unit test" split precisely, with zero ambiguity.
- New pytest tests (`tests/test_view_binding.py`, +6):
  `test_background_binding_accepts_a_hex_color_signal` and
  `test_background_binding_accepts_an_rgba_tuple_signal` -- both prove
  the binding applies and survives re-evaluation without raising,
  verified indirectly via a co-bound `opacity` read on the same node
  (a real, honestly-stated limitation: `tre` has no Python-facing
  getter for a node's currently-applied `background` color at all --
  `Node.get` only returns `f64`, confirmed by grep of `node.rs`/`_core.
  pyi` before writing the test this way, not worked around with a fake
  assertion). `test_background_binding_rejects_an_invalid_color_string`
  -- raises `ValueError` naming the bad string.
  `test_background_binding_rejects_a_boolean_value` -- real M44
  regression coverage: under the old dispatch, *any* `Bool`-resolved
  binding, regardless of declared property, was routed unconditionally
  to `set_checked`, which would raise a `Rect`-is-not-a-Checkbox error;
  the new dispatch rejects it directly with a message naming the actual
  property instead. `test_checked_binding_gives_a_clear_error_for_a_
  non_boolean_value`/`test_text_binding_gives_a_clear_error_for_a_non_
  string_value` -- confirms the message-quality tightening (both cases
  already errored before this milestone too, just via `animate()`'s own
  "unknown property" rejection or the wrong-widget-kind path through
  `set_text`/`set_checked` -- this is a clearer, more specific error,
  not a new accept/reject behavior). Two real fixture bugs caught
  immediately by running pytest, not anticipated: the `TextField`
  fixture needed both `style.background` and a `text:` block (`content`/
  `font_family`/`font_size`) to parse at all -- fixed by copying the
  exact shape `test_two_way_binding.py`'s own `TextField` fixture
  already uses.
- Confirmed via grep (`#[pymethods]`/`#[pyclass]` in `view.rs`/
  `binding.rs`) that none of `apply_binding_value`, `parse_background_
  color`, or the widened `to_pyobject` are Python-visible -- no `.pyi`
  change needed.
- New live example `examples/bindable_background.py` + `.yaml`: two
  `Rect` swatches, `hex_swatch` bound to a `Signal[str]` (hex color),
  `tuple_swatch` bound to a `Signal[tuple]` (`(r,g,b,a)`), a
  `cycle_button` handler advancing both Signals together through a
  real 3-entry palette via real dispatched clicks -- asserts the
  ViewModel's own state after each click (the same "no color getter"
  limitation as the pytest tests, so assertions check the ViewModel's
  own Signals, not the rendered node), then a genuine 60-frame
  `App.run()` via `Window.from_view`, matching `component_list.py`'s
  own established View-then-Window pattern. Ran clean on the first try.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (`engine-py` 15, up from 11, +4; every other crate's own count
  unchanged); `maturin develop --release` rebuilt; `pytest tests/` (606
  passed, up from 600, +6, 1 skipped, unchanged); all 81 examples run
  individually with zero failures; `demo/showcase.py` (all 5 phases,
  exit 0).
- `BUILD_TRACKER.md`: new Milestone 44 section, Top Metrics row, "Just
  closed" entry added; `tools/generate_tracker_artifact.py` confirmed
  44 milestones/136 phases/229 items (up from 43/135/228, the correct
  +1/+1/+1 for one new milestone with one phase with one step); Build
  Tracker artifact republished to the existing URL.

**M44 -- Widening Live-Bindable Properties: `background` Color Bindings
-- is now fully complete, single phase.** This closes the milestone --
per the standing "push only after a full milestone closes" convention,
a `git push` is now appropriate.
