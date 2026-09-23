# LOG — M56: `Event.node` — a Real Live `Node` Handle

- User: "What do you recommend next?" -> recommended `Event.source` ->
  `Node`, the deferred design question both M54 and M55 explicitly
  named and left open. User: "Scope Event.source -> Node."
- Dispatched a dedicated Explore agent to trace every real `Event`-
  construction call site against `Node`'s own real constructor
  requirements before designing anything.
- **Key finding: needs zero `engine-core` changes.** Every field `Node`
  needs (`id`, `tree`, `handlers`, `context_menus`, `theme`,
  `completions`) is already reachable at every real `Event`-building
  call site; the only real gaps were a handful of `context_menus`/
  `theme`/`completions` fields not yet pulled into local scope at ~9
  call sites -- mechanical plumbing, not an architectural change.
- **Re-entrancy confirmed safe by existing precedent, not assumed:**
  `call_handler`'s own `make_event` closure always completes -- `Event`
  fully built, `Py<Node>` included -- *before* the real Python handler
  itself is invoked; no `Tree` borrow is ever held at that moment.
  Building a `Node` costs only cheap `Rc` clones, the same real
  "clone out, drop the borrow, then call" discipline this codebase
  already established everywhere else.
- Three real design forks resolved via `AskUserQuestion`: additive
  `Event.node: Node` field, `source: int` unchanged (not a breaking
  type change); eager construction, every dispatch (matches every
  other `Event` field's own existing shape); and -- the one non-
  default choice, overriding this session's own stated recommendation
  to leave it out of scope -- **also fix `paste_from_system_clipboard`'s
  own pre-existing `active`-bundle bypass this same milestone**, once
  the investigation found `copy_to_system_clipboard`/`cut_to_system_
  clipboard` share the identical real gap.
- Entered Plan Mode with this real, grounded scope before implementing.

## Phase 1 — `engine-py`: `Event.node` + threading through every real call site

- `Event` gained `node: Py<Node>` (`event.rs`), additive alongside the
  unchanged `source: u64` -- M56 scoping's own explicit choice, since a
  breaking type change would have broken M54's own already-shipped
  `source: int` contract (test-asserted in `tests/test_event_
  payload.py`). Built via a new `Event::build_node(py, id, ctx)`
  helper, reusing the identical 6-field construction pattern every
  real `add_*` factory already uses.
- New `pub(crate) struct NodeContext<'a>` (`event.rs`): bundles the 5
  real handles every `Node` needs besides its own `id` -- justified via
  this codebase's own "Rule of Three" precedent (the same real
  threshold `HandlerMap`'s own doc comment already used to justify
  factoring out a shared type), since 6+ real functions now need this
  exact 5-tuple to travel together. All 4 `Event::*` constructors
  (`click`/`hover`/`change`/`focus_transition`) widened to accept `py`
  and `&NodeContext`, returning `PyResult<Self>` (previously infallible
  `Self`) -- fallible only because `Py::new` (building the live `Node`)
  is.
- `run_dispatch_outcome`/`fire_focus_transition`/`cut_focused_
  selection_to_clipboard`/`paste_clipboard_into_focused` (`dispatch.rs`)
  all widened with `context_menus`/`theme`/`completions` parameters
  (`#[allow(clippy::too_many_arguments)]`, matching `window_factory.rs`
  's own established 20+-use convention for functions needing many
  real, necessary parameters), each building a local `NodeContext` and
  passing `&ctx` into their own `Event::*` calls.
- Every real call site threading the three new arguments through, one
  file at a time, each `cargo check -p engine-py` run driving the next:
  - `app.rs`: the main winit `on_input` dispatch closure; the `Cut`/
    `PasteRequested` `InputEvent` arms; the AccessKit access-request
    closure's own `Action::Click`/`Action::Focus` arms (its local
    destructure widened to also pull `context_menus` from `active`,
    `theme`/`completions` straight off `runtime`, matching `PyWindow`'s
    own real "not part of the `active` swap" shape for those two).
  - `node.rs`'s 4 setters (`set_checked`/`set_selected`/`set_on`/
    `set_text`), each now building a local `NodeContext` from `self.*`
    fields before its own `call_handler` call.
  - `window_input.rs`'s 7 dispatch methods (`click`/`hover`/`scroll`/
    `focus`/`resize`/`press_key`/`type_text`) -- the 4 that already
    destructure through `self.active` (`click`/`hover`/`scroll`/
    `focus`) gained `context_menus` there; `resize`/`press_key`/
    `type_text`, which read `self.tree`/`self.handlers` directly (never
    through `active`), read `self.context_menus`/`self.theme`/`self.
    completions` the same direct way, for consistency with what they
    already do. `cut()`'s own direct `Event::change` call updated too.
  - `view.rs`'s 4 dispatch methods (`click`/`hover`/`focus`/
    `right_click`), reading `self.context_menus`/`self.theme`/`self.
    completions` directly -- `View` has no `active`-swap bundle at all.
- **The approved adjacent fix, done as part of this phase, not
  deferred:** `copy_to_system_clipboard`/`cut_to_system_clipboard`/
  `paste_from_system_clipboard` (`window_input.rs`) rewritten to read
  `tree`/`handlers`/`context_menus`/`root` through `self.active.
  borrow()`, matching every other real `Window` method's established
  post-`show_view` convention, instead of the plain `self.tree`/`self.
  handlers`/`self.root` fields they used to read directly -- a real
  staleness risk after a real `show_view` switch, the user's own
  explicit choice to fix here rather than leave named-and-deferred.
- Full chain green, first full pass after the mechanical fix-through:
  `cargo check --workspace --all-targets`/`clippy --workspace --all-
  targets -- -D warnings`/`fmt --check` all clean (zero `engine-core`
  changes, exactly as the investigation predicted -- confirmed, not
  just assumed, by the fact `cargo test -p engine-core` stayed at
  exactly 227, unchanged from M55). `cargo test --workspace --release`
  green across every crate.
- `maturin develop --release`; a standalone smoke script (`tests/`-
  adjacent, not pytest -- this session's own established pre-formal-
  test habit) ran 11 real checks before writing formal tests:
  `event.source` still a plain `int`; `event.node` is a real `Node`
  instance; a mutation made *through* `event.node` (`set_checked`) is
  visible on the originally-registered handle -- the real identity
  proof used, since `Node` has no Python-facing `__eq__`/`id` to
  compare more directly (confirmed via `_core.pyi`, not assumed); a
  handler calling `.animate()`/`.get_checked()`/`.set_on_hover_enter()`
  back on `event.node` immediately, with no re-entrant-borrow panic;
  `event.node` resolves correctly across `HoverEnter`/`HoverExit`/
  `FocusEnter`/`Change`; a single handler shared across two real
  `Checkbox`es correctly tells them apart via `event.node`'s own live
  state; the fixed `copy_to_system_clipboard` runs without panicking.
  All 11 passed.
- `pytest tests/`: 776 passed, 2 skipped -- byte-for-byte unchanged
  from before this phase, confirming zero regression.

## Phase 2 — Python-Facing API, Tests, Example, Docs

- `python/tre/_core.pyi`: new `Event.node: Node` field stub; `source`'s
  own doc comment updated to name `node` as the real live-handle
  counterpart, replacing the now-stale "deliberately deferred" claim.
- 8 new pytest tests (`tests/test_event_node.py`): `event.node` is a
  real `Node` instance; a mutation made *through* `event.node` is
  visible on the originally-registered handle (the real identity proof
  used, since `Node` exposes no `__eq__`/`id`); a handler calling
  `.animate()`/`.get_checked()`/`.set_on_hover_enter()` back on `event.
  node` immediately does not panic; `event.node` correct for a real
  `Click`/`HoverEnter`/`FocusEnter`/`Change`; a shared handler across
  two `Checkbox`es tells them apart via `event.node`'s own live state.
  **Real, honest finding, not glossed over:** a testable repro for the
  fixed `copy_to_system_clipboard` staleness case specifically through
  a real `show_view` switch does not exist through the current public
  API -- `select_all`/`press_key`/`copy`/`cut` (needed to create a real
  text selection to copy) all still read `self.tree`/`self.handlers`
  directly, unaffected by `show_view` (which only ever swaps `self.
  active`), a real, separate, pre-existing limitation explicitly out of
  this milestone's own approved scope. No misleading test was written
  to paper over that gap. Also corrected `test_event_payload.py`'s own
  now-stale docstring, which still claimed `source` was "not a live
  `Node` handle this milestone."
- New section in `examples/event_payload.py`: a single handler shared
  across three real `Checkbox`es, toggling and reading back through
  `event.node` directly -- the real scenario `Event.source`'s bare
  opaque id couldn't serve without a caller-maintained id lookup of its
  own -- with a real functional assertion that only the clicked node's
  own state ever changes.
- `BUILD_TRACKER.md`: new Milestone 56 section (both phases), Top
  Metrics row added at 100%, fresh "Just closed"/"Up next" front-matter
  pair prepended (matching this session's own established convention
  for the tracker generator's own first-match `_grab()` behavior).
  Regenerated: 56 milestones/167 phases/316 items/1 known gap/20 fixed
  gaps. Artifact republished at the existing URL.
- Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
  clean (no Rust changes this phase); `cargo test --workspace
  --release` (unchanged). `maturin develop --release`; `pytest tests/`
  (784 passed, up from 776, +8, 2 skipped unchanged); all 88 examples
  (+1, zero failures); `demo/showcase.py` (all 5 phases, exit 0).

## Status

**M56 is complete -- both phases.** The real capability gap this
milestone exists to close -- a handler shared generically across
several nodes having no way to know which one just fired without a
live handle -- is closed, additively, with zero breaking changes to
`Event.source`'s own already-shipped `int` contract and zero
regression to any of the 776 pre-existing tests or 87 pre-existing
examples. Needed zero `engine-core` changes, exactly as the
investigation predicted. One real, adjacent, pre-existing bug fixed
along the way at the user's own explicit choice (overriding this
session's own stated recommendation): `copy_to_system_clipboard`/
`cut_to_system_clipboard`/`paste_from_system_clipboard`'s own real
`active`-bundle bypass. One honest limitation found and named, not
worked around: the fixed clipboard staleness case has no real testable
repro through the current public API without also touching `select_
all`/`press_key` (out of scope). Committing Phase 2 locally now; push
deferred pending explicit user confirmation, per this session's own
established, unwavering convention.
