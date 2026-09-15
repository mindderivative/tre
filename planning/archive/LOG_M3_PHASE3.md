# Log: M3 Phase 3 — `engine-spec` Minimal (§14 step 5)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 3, the sole step of that phase.

## What happened

**`engine-spec::spec`** implements §16.1's `WidgetSpec` sketch, narrowed
to this step's own stated scope: `id`/`kind`/`style`/`children`, no
`bindings`/`handlers` at all (not stubbed -- nothing can resolve them
without `BindingResolver`, step 12). `StyleSpec` is literal-value-only
(`background: "#6750A4"`), deferring MD3 token names to step 11.
`background` is parsed via `peniko::color::parse_color` (already
supports hex and CSS named colors) rather than hand-rolling a parser.

**Real finding: `serde_yaml_ng` doesn't support serde's classic
external-tagging shape for data-carrying enum variants.** First attempt
modeled `NodeKindSpec::Text(TextSpec)` as a real Rust enum with data,
expecting `kind: {Text: {content: ..., ...}}` to parse (the same
single-key-map convention `serde_json` supports). It didn't: checked
`serde_yaml_ng`'s own `deserialize_enum` directly and found it only
accepts a bare scalar (unit variants) or YAML's native `!Tag` syntax for
a data-carrying variant -- never a plain mapping. Rather than asking
every `view.yaml` author to write `kind: !Text {...}` (an obscure YAML
convention with no reason to expect anyone to know it), restructured to
a flatter shape that sidesteps the issue entirely and reads more
naturally besides: `NodeKindSpec` is unit-only (`Rect`/`Container`/
`Text`), and `Text`'s own fields (`content`/`font_family`/`font_weight`/
`font_size`) live in a sibling `WidgetSpec::text: Option<TextSpec>`,
validated as required at build time when `kind: Text`.

**`engine-spec::build`** maps `WidgetSpec` -> `engine_core::Tree`:
`taffy::Style` from `StyleSpec`'s layout fields, `NodeKind`/
`PaintProperties` per kind. `background` is required (a clear
`SpecError::MissingField`, not a silent default) for `Rect`/`Text` --
an invisible, colorless shape being a real spec mistake worth failing
loudly on, matching this project's "fail loudly at the boundary"
discipline (§8's `EngineError`, `deny_unknown_fields` itself). A
`Container` without `background` defaults to fully transparent, matching
every hand-built `Container` this codebase has produced so far.

**Unit tests** (6, in `spec.rs`/`build.rs`): parsing a real nested tree
correctly, `deny_unknown_fields` rejecting a typo'd key with a clear
message, a deterministic layout-position test (exact taffy x-coordinates
from a real built `Tree`, same discipline as step 3's own row-layout
test), `MissingField` and `InvalidColor` error paths each naming the
actual offending widget `id`.

**A real, standalone `view.yaml`** (`crates/engine-spec/examples/
view.yaml`, not just an inline Rust string) -- a `Container` row with a
`#6750A4` `Rect` swatch and a "tre v2" `Text` label -- proven to render
through the real Phase 2 pipeline by a new `engine-render` integration
test (`tests/spec_view.rs`, `engine-spec` added as a dev-dependency):
parses the file, builds a `Tree`, computes layout, renders headlessly,
and reads back pixels -- the swatch's center pixel matches the YAML's
declared color exactly, and the label draws real ink in its own box.
Passed on the first real attempt once the enum-shape finding above was
fixed.

## Verification

```
$ cargo test --workspace
    ...
running 6 tests (engine-spec)
test spec::tests::parses_a_nested_widget_tree ... ok
test spec::tests::unknown_field_is_a_load_time_error_not_silently_ignored ... ok
test build::tests::load_view_builds_a_real_tree_matching_the_spec ... ok
test build::tests::rect_without_background_is_a_clear_error_not_a_default ... ok
test build::tests::invalid_color_names_the_offending_widget_and_value ... ok
test build::tests::container_without_background_defaults_to_transparent ... ok
test result: ok. 6 passed; 0 failed

     Running tests/spec_view.rs (engine_render)
test a_real_view_yaml_renders_through_the_real_pipeline ... ok
    ... (every other crate/test green, frame_budget still ignored as expected)

$ cargo clippy --workspace --all-targets -- -D warnings   # clean
$ cargo fmt --check                                        # clean
```

## Next

`BUILD_TRACKER.md` updated: Phase 3 (§14 step 5) done, M3 to 43% (3 of 7
phases). Next: M3 Phase 4 (§14 steps 6-7) -- wire `engine-py` (node
creation + one property setter, driving step 2's animation from a `.py`
script), then `accesskit` (confirm one button is correctly exposed to a
screen reader). First step to touch `pyo3` at all.
