# Log: M19 Phase 2 — Real View Composition via `include:` (§16.6), closing M19

Corresponds to `BUILD_TRACKER.md` M19 Phase 2, closing M19 entirely
(both phases). `WidgetSpec` gains a real `include:` field; loading
expands each included file's own `WidgetSpec` tree in place, before
validation, as ordinary children indistinguishable from inline ones —
with real path confinement, cycle detection, and a depth limit.

## Investigation before writing code

`WidgetSpec`'s own `#[serde(deny_unknown_fields)]` plus required `id`/
`kind` fields mean a bare `{include: "path"}` mapping can't
deserialize directly into it. Two real design options: widen
`WidgetSpec.children`'s element type to an untagged `Include | Widget`
enum, or expand `include:` markers on the *raw* `serde_yaml_ng::Value`
tree before ever deserializing into `WidgetSpec` at all. The
raw-`Value` route was chosen — it keeps `WidgetSpec`, `build.rs`, and
`reconcile.rs` completely unchanged, and matches §16.6's own text
exactly: "expanded during loading, *before* validation."

`Reconciler::load`/`Reconciler::reconcile` already took two additive
`Option<&T>` parameters (`sheet`, `scheme`) — widened with a third,
`base_dir: Option<&Path>`, matching that exact existing precedent. 9
existing call sites (7 in `engine-spec`'s own tests, 2 in
`engine-py::view.rs`) needed one more argument each — a bounded,
mechanical change, all updated.

## What happened

New `crates/engine-spec/src/include.rs` (mirroring `watch.rs`'s own
precedent of one small, focused module per capability):
`parse_view_with_includes(yaml, base_dir: Option<&Path>) ->
Result<WidgetSpec, SpecError>` parses into a raw `Value`, calls a
recursive `expand_includes`, then deserializes the fully-expanded tree
— `deny_unknown_fields`'s own real validation runs on the *final*
tree, exactly as specified. A mapping whose *sole* key is `include`
(any other key alongside it is a real, stated error) resolves its
path via `Path::canonicalize`-based confinement (rejects an absolute
path or a real `../` escape; resolves symlinks too, so one can't evade
it), reads+parses the target, recurses (the target's own parent
directory becomes the new `base_dir` for *its* includes, pushed onto a
`visited` stack for cycle detection), and splices the result in place
of the marker. `base_dir: None` is real and valid — content with no
`include:` at all parses exactly as plain `parse_view` would; an
`include:` actually encountered with no `base_dir` fails clearly
(`SpecError::IncludeNoBaseDir`), not silently. `MAX_INCLUDE_DEPTH = 8`
guards a genuinely unbounded chain. `SpecError` gained six new
variants covering every real failure mode.

`Reconciler::load`/`Reconciler::reconcile` gained `base_dir: Option<
&Path>` and now always call `parse_view_with_includes` (not
conditionally). `engine-py::view.rs`: `View::new`/`poll_reload` pass
`Some(parent directory of self.path)` — the first real callers besides
tests. **Real, additional fix caught while wiring `View::new`:** the
second, separate `parse_view(&yaml)` call there (used only to collect
`bindings`/`declared_handlers`/`two_way`) also needed to become
include-aware — otherwise a binding or handler declared inside an
*included* file would never be collected at all, a real, silent gap
that would have shipped alongside the main feature had it not been
caught during implementation, not by a test.

New `crates/engine-spec/src/include.rs` tests (9, all passed first
run): a real two-file include splices in correctly; nested includes
resolve relative to their own file, not the top-level `base_dir`; no
`base_dir` fails clearly; an absolute path is rejected; a real `../`
escape outside `base_dir` is rejected; a real self-including cycle is
rejected; a chain past the depth limit is rejected; `include:`
alongside another key is rejected; content with no `include:` at all
parses byte-for-byte the same as plain `parse_view`. New
`tests/test_view_composition.py` (2 tests): a real `View` loads a
two-file `view.yaml` and reaches the included widget's own `id`
through the real FFI path; an absolute include path fails loudly
there too, not silently. New `examples/view_composition.py` +
`view_composition.yaml` + `confirm_dialog.yaml`.

Full `cargo test --workspace --release` (`engine-spec` 46, up from
37)/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo fmt
--check` all clean — every prior test passed unmodified. `maturin
develop --release` + full `pytest tests/` (168 passed, up from 166, 1
skipped) and all twenty-seven examples confirmed clean.

M19 — Declarative Authoring Completeness is now fully complete.
