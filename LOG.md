# Log: M3 Phase 6, Step 12 — `engine-spec`, Full (§14 step 12)

Corresponds to `PLAN.md` / `BUILD_TRACKER.md` M3 Phase 6, step 12 of 1 --
closes Phase 6. The largest bundled build-order step so far (§16.2 +
§16.3 + §16.4 together); split into three separately-committed stages,
each independently verified.

## Stage A — §16.3 Stylesheet cascade + MD3 token resolution

`WidgetSpec` gained `classes: Vec<String>`. `engine_spec::cascade`
implements the exact precedence order (baseline → `kind:` → `classes:`
→ `id:` → inline `style:`) as a per-field merge, not whole-struct
replacement -- proven by a test asserting two rules setting different
fields both survive in the final resolved style. `classes:` matching is
a subset match, applied in ascending specificity order so a
two-class rule wins over a one-class rule that also matches.

`engine_md3::ColorScheme::role(name)` centralizes MD3 token lookup in
the crate that owns the roles. `engine-spec`'s color resolution tries a
token lookup first, falls back to `peniko::color::parse_color` --
`load_view` (no cascade, no tokens) kept working unchanged for its one
existing caller; `load_styled_view` is the new full path. Real
end-to-end test: `background: primary` resolves to the *exact* color a
real `DynamicTheme`'s own `ColorScheme.primary` holds.

## Stage B — §16.4 Reconciliation & hot-reload

`engine_core::Tree::remove(id)`: real recursive subtree removal --
`taffy::TaffyTree::remove` only detaches one node and orphans its
children (confirmed directly in its own doc comment), so this crate's
own version recurses first.

`engine_spec::Reconciler` implements the keyed diff (matched by `id`
plus `NodeKindSpec`): an unmatched-props node is patched via the new
`build::patch_node` (id/parent/children/access/interaction untouched,
so focus and in-flight animations on those survive); a new id is
inserted fresh; a disappeared id is actually removed, not just
untracked. **Found and fixed a real ordering bug during testing**: the
old id→NodeId mapping was being overwritten by the new node's mapping
*before* the old node was removed on a kind change, so the cleanup pass
ended up removing the freshly-inserted node instead of the stale one --
fixed by removing the old node first, at the point of detection.

`engine_spec::ViewWatcher` wraps a real `notify::RecommendedWatcher`,
verified against an actual file write on disk (a bounded polling loop,
not a fixed sleep, to avoid flakiness while still failing definitively
if detection breaks). Not wired into a live running `engine-py` `App`
this stage -- that needs "this app was loaded from a `view.yaml`" as
new `App` surface nothing currently needs.

**Stated scope limit:** a child whose `id` keeps its match but moves to
a different position in a reload is patched in place but not reordered
within `Tree`'s own `children` list -- needs a `taffy` child-order
mutation API this stage doesn't verify, and no test scenario requires.

## Stage C — §16.2 `BindingResolver` + expression grammar + `ViewModel`

`engine_spec::binding`: a real hand-written recursive-descent parser
for the exact whitelisted grammar §16.2 names (attribute access,
indexing, comparison, arithmetic, boolean logic, zero-arg method
calls) -- no parser-generator dependency for a grammar this small. The
generic `BindingResolver` trait is the same dependency-inversion shape
as `AppHandler`; primitive-only binary operations are computed directly
by `evaluate()` itself, delegating to the resolver only when a
`Value::Handle` (a non-primitive object) is involved. Tested with a
fake in-crate resolver -- 8 tests -- proving the grammar/evaluator with
zero `pyo3` knowledge, before `engine-py` implements the real one.

`engine-py::PyViewModelResolver` implements `BindingResolver` via real
`pyo3` attribute/index/method-call access against a live `Py<PyAny>`
ViewModel -- `bool` checked before `i64` when unwrapping a Python value
(a Python `bool` is an `int` subtype and would otherwise extract
successfully, and wrongly, as one).

`WidgetSpec` gained back `bindings`/`handlers: HashMap<String, String>`
(raw strings -- omitted since step 5, exactly as that step's own doc
comment predicted: "additive... when step 12 actually needs them").

`engine-py::View`: `View(path)` loads a `view.yaml` into its own
`Tree`; `View._attach(viewmodel)` is the real §16.2 inversion point --
validates every handler eagerly (`getattr` + callable check, raising at
attach time on a bad name, not on first click), then for every binding:
parses it, evaluates it once inside a recording scope
(`begin_recording`/`end_recording`, a thread-local `Vec<Py<PyAny>>`),
applies the initial value by constructing a temporary `Node` and
calling its own `animate()` dispatch verbatim (not reimplemented), and
subscribes a `BindingCallback` onto every `Signal` the evaluation
actually touched.

**Real finding, not obvious from `animate`'s own contract**: the first
pytest run of the binding tests failed -- `node.get("opacity")` kept
returning the static YAML value, never the bound one. `animate(...,
duration_ms=0)` only *registers* a zero-duration `ActiveAnimation`;
`Animated::animate_to` never eagerly writes `current` itself, only the
next `tick_all` call does. A `View` with no running render loop attached
never calls that, so a binding's "instant" value would never actually
become observable. Fixed by having `apply_binding_value` call
`Tree::tick_all` immediately after registering -- `duration_ms=0` now
means what it says regardless of whether a frame loop happens to be
running.

Python side (`python/tre/__init__.py`): a real `Signal` (`.get()` calls
a Rust `_record_read` function that's a no-op outside an active
recording scope; `.set()`/`.update()` notify subscribers) and
`ViewModel` (`__init__` calls `view._attach(self)`).

**Real, end-to-end proof** (`tests/test_view_binding.py`, 7 new tests,
via `maturin develop` + pytest, same discipline as `test_engine_py.py`):
a binding's initial value applies correctly; writing the *exact* Signal
it read via `.set()` re-applies it automatically; the same via
`.update()` with a full arithmetic expression (`level.get() + 0.1`); a
Signal the binding never read has zero effect when written (proving
dependency tracking is genuinely selective, not "re-run everything on
any write"); a missing handler method raises at attach time; a present
one doesn't.

**Scoped narrower than §16.2's full picture, stated explicitly**:
handler wiring stops at eager validation -- actually *firing* one needs
real `InputEvent`/`AppHandler` pointer dispatch, which still doesn't
exist anywhere in this codebase (the same finding steps 7/9/11 already
made). Binding application supports `opacity`/`corner_radius` only --
`background` needs a color-string/MD3-token parse a bound value hasn't
gone through yet, real additive work for whenever a binding actually
needs a dynamic color. A binding's dependency set is captured once, at
its first evaluation, not re-tracked per write -- correct for every
binding shape §16.2's own examples show.

## Verification

```
$ cargo test --workspace                                  # all green
$ cargo clippy --workspace --all-targets -- -D warnings    # clean
$ cargo fmt --check                                        # clean
$ maturin develop && python -m pytest tests/ -v
13 passed (6 pre-existing + 7 new in test_view_binding.py)
$ python examples/animate_rect.py
animate_rect.py: exited cleanly after 60 frames            # no regression
```

## Next

`BUILD_TRACKER.md` updated: Phase 6 (step 12, all three stages) done --
M3 now 6 of 7 phases complete. Next: Phase 7 (§14 steps 13-15) --
overlay mechanism (one dropdown menu, §11.3), multi-window (a second
`PyWindow`, §11.1), then docking (§11.4) + virtualization (§11.7),
sequenced last since both build on the overlay mechanism and the
accepted multi-window model. This is M3's final phase.
