# Plan: M19 Phase 2 — Real View Composition via `include:` (§16.6)

Corresponds to `BUILD_TRACKER.md` M19 Phase 2, closing M19 entirely.
`WidgetSpec` gains a real `include:` field; loading expands each
included file's own `WidgetSpec` tree in place, before validation, as
ordinary children indistinguishable from inline ones — with real path
confinement, cycle detection, and a depth limit.

## Investigation before writing code

- `WidgetSpec`'s own `#[serde(deny_unknown_fields)]` + required `id`/
  `kind` fields mean a bare `{include: "path"}` mapping can't
  deserialize directly into it (missing required fields, an unknown
  key) — confirmed via direct read of `spec.rs`. Two real design
  options: widen `WidgetSpec.children`'s own element type to a new
  untagged enum (`Include | Widget(WidgetSpec)`), or expand `include:`
  markers on the *raw* `serde_yaml_ng::Value` tree before ever
  deserializing into `WidgetSpec` at all. The untagged-enum route
  would ripple `children`'s type through `build.rs`/`reconcile.rs`
  everywhere it's walked; the raw-`Value` route keeps `WidgetSpec`
  itself completely unchanged — `build.rs`/`reconcile.rs` need zero
  changes — and matches §16.6's own text exactly: "expanded during
  loading, *before* validation... indistinguishable from inline ones
  once loaded." Chosen.
- `serde_yaml_ng::Value` (confirmed via direct source read of the
  vendored crate) is `{Null, Bool, Number, String, Sequence, Mapping,
  Tagged}`; `Mapping::get<I: Index>` accepts a plain `&str` key
  (`impl Index for str`), and `Mapping` is `IntoIterator` over owned
  `(Value, Value)` pairs — enough to walk and rebuild a tree generically
  without a second, hand-rolled YAML representation.
- `Reconciler::load`/`Reconciler::reconcile` already take two
  additive `Option<&T>` parameters (`sheet`, `scheme`) — widening both
  with a third, `base_dir: Option<&Path>`, matches that exact existing
  precedent rather than inventing a new shape. `None` means "no base
  directory known" — an `include:` encountered with no `base_dir`
  fails with a clear error, not a silent no-op. 9 existing call sites
  (7 in `engine-spec`'s own tests, 2 in `engine-py::view.rs`) need one
  more `None` argument each — a bounded, mechanical change.
- Real security requirements, named explicitly in ARCHITECTURE.md's
  own §16.6 text: path confinement (no `../` escaping the view
  directory), cycle detection (a file cannot transitively include
  itself), a depth limit. `Path::canonicalize()` resolves symlinks too
  (a real, not superficial, confinement check) and requires the target
  to actually exist — acceptable, since the file has to be read anyway.

## Design

- New `crates/engine-spec/src/include.rs` module (mirroring `watch.rs`'s
  own precedent of one small, focused file per capability):
  `expand_includes(value, base_dir, visited: &mut Vec<PathBuf>) ->
  Result<Value, SpecError>` — recursively walks every `Mapping`/
  `Sequence`. A mapping whose *sole* key is `include` (any other key
  alongside it is a real, stated error, not silently ignored) resolves
  its string value against `base_dir` via a confining `canonicalize`
  check, reads+parses the target file's own raw YAML, recurses into
  it (with the target's own parent directory as the new `base_dir` for
  *its* includes, and itself pushed onto `visited` for cycle
  detection), and splices the fully-expanded result in place of the
  include marker. `SpecError` gains new variants for a missing/absent
  `base_dir`, an out-of-bounds path, a cycle, the depth limit, and a
  wrapped file-read/parse failure.
- New `parse_view_with_includes(yaml, base_dir) -> Result<WidgetSpec,
  SpecError>`: parses into a raw `Value` first, calls `expand_includes`,
  then deserializes the fully-expanded `Value` into `WidgetSpec` via
  `serde_yaml_ng::from_value` — `deny_unknown_fields`'s own real
  validation runs on the *final*, expanded tree, exactly as specified.
- `Reconciler::load`/`Reconciler::reconcile` gain `base_dir: Option<
  &Path>` (a third parameter, after `scheme`); when `Some`, they call
  `parse_view_with_includes` instead of the plain `parse_view`.
- `engine-py::view.rs`: `View::new`/`poll_reload` pass `Some(parent
  directory of self.path)` — the first real caller besides tests.

## Verification plan

`cargo test --workspace --release`/`clippy -D warnings`/`fmt --check`;
new `engine-spec` tests: a real two-file include splices in correctly;
nested includes; `../` path-escape rejected; a real self-including
cycle rejected; the depth limit rejected; an `include:` with no
`base_dir` fails clearly, not silently; `include:` alongside another
key in the same mapping fails clearly. `maturin develop --release`;
new `tests/test_view_composition.py` proving a real `View` loads a
two-file `view.yaml` correctly through the real FFI path; every
example re-run (a new `examples/view_composition.py` + two YAML
files, matching the established per-feature convention); `LOG.md`/
`BUILD_TRACKER.md`/tracker artifact/commit/memory — closing M19
entirely (both phases).
