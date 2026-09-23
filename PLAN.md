# PLAN — M56: `Event.node` — a Real Live `Node` Handle

*(Replaces the prior M55 plan in this file — M55 is complete, committed,
and pushed. This is a new milestone.)*

## Goal
M54 and M55 both explicitly deferred the same real design question:
should `Event.source` (a plain opaque id) become a live `Node` handle?
User: "What do you recommend next?" -> recommended this exact deferred
candidate. User: "Scope Event.source -> Node."

## Real investigation
A dedicated Explore agent traced every real `Event`-construction call
site against `Node`'s own real constructor requirements (`id`, `tree`,
`handlers`, `context_menus`, `theme`, `completions`). **Key finding:
this needs zero `engine-core` changes** -- every field `Node` needs is
already reachable at every real `Event`-building call site; the only
real gaps were a handful of `context_menus`/`theme`/`completions`
fields not yet pulled into local scope at ~9 call sites -- mechanical,
not architectural. Re-entrancy confirmed safe by existing precedent:
`call_handler`'s own `make_event` closure always completes -- `Node`
built, `Py<Node>` included -- *before* the Python handler itself runs;
building a `Node` costs only cheap `Rc` clones, never a `Tree` borrow.

## Three real design forks, resolved via `AskUserQuestion`
1. Additive `Event.node: Node` field, `source: int` unchanged -- zero
   existing code (including M54's own test) breaks.
2. Eager construction, every dispatch -- matches every other `Event`
   field's own existing shape.
3. Also fix `paste_from_system_clipboard`'s (and, found while tracing
   every call site, `copy_to_system_clipboard`'s/`cut_to_system_
   clipboard`'s own identical) pre-existing gap: they read `self.tree`/
   `self.handlers` directly instead of through the `active` bundle every
   other `Window` method already uses after a real `show_view` switch --
   a real, adjacent staleness risk, folded into this milestone at the
   user's own explicit choice, overriding this session's own stated
   recommendation to leave it out of scope.

## Design (2 phases -- no `engine-core` work needed)
1. **engine-py: `Event.node` + threading through every real call site.**
   `Event` gains `node: Py<Node>`; `Event::click`/`hover`/`change`/
   `focus_transition` widen to accept `py`/a new shared `NodeContext`
   and return `PyResult<Self>`; `run_dispatch_outcome`/`fire_focus_
   transition` widen to accept `context_menus`/`theme`/`completions`;
   every real caller updated; the approved adjacent clipboard fix done
   here too.
2. **Python-facing API, tests, example, docs.** `_core.pyi` stub;
   pytest coverage (identity, re-entrancy, all 4 `EventKind`s, the
   fixed clipboard staleness case); a new/extended example; `BUILD_
   TRACKER.md`/tracker regeneration/artifact republish.

## Explicitly out of scope, named not silent
Changing `Event.source`'s own type or removing it. Any new `EventKind`
variant. Any change to `Node`'s own GC-traversal contract (still
correctly needs none of its own -- a `Py<Node>` field on `Event`
introduces no new untracked edge beyond what already isn't tracked).

## Status

**Phase 1 complete.** Phase 2 (Python-facing API, tests, example, docs,
tracker) not yet started.

`Event` gained `node: Py<Node>` (`event.rs`), built via a new `Event::
build_node(py, id, ctx)` helper reusing every real `add_*` factory's own
6-field construction pattern. All 4 constructors (`click`/`hover`/
`change`/`focus_transition`) widened to `(py, node, ctx: &NodeContext,
...) -> PyResult<Self>` (previously infallible `Self`). New `pub(crate)
struct NodeContext<'a>` (`event.rs`) bundles the 5 real handles every
`Node` needs -- justified via this codebase's own "Rule of Three"
precedent (the same threshold `HandlerMap`'s own doc comment already
used), since 6+ real functions now need this exact 5-tuple.

`run_dispatch_outcome`/`fire_focus_transition`/`cut_focused_selection_
to_clipboard`/`paste_clipboard_into_focused` (`dispatch.rs`) all widened
with `context_menus`/`theme`/`completions` parameters (`#[allow(clippy::
too_many_arguments)]`, matching `window_factory.rs`'s own established
20+-use convention), each building a local `NodeContext` and passing
`&ctx` into their `Event::*` calls.

Every real call site threading these three new arguments through:
`app.rs` (the main winit dispatch closure, the `Cut`/`PasteRequested`
`InputEvent` arms, and the AccessKit access-request closure's own
`Action::Click`/`Action::Focus` arms -- 4 sites total); `node.rs`'s 4
setters (`set_checked`/`set_selected`/`set_on`/`set_text`), each now
building a local `NodeContext` from `self.*` fields; `window_input.rs`'s
7 dispatch methods (`click`/`hover`/`scroll`/`focus`/`resize`/
`press_key`/`type_text`) plus `cut()`'s own direct `Event::change` call;
`view.rs`'s 4 dispatch methods (`click`/`hover`/`focus`/`right_click`),
reading `self.context_menus`/`self.theme`/`self.completions` directly
(no `active`-swap bundle on `View`).

**The approved adjacent fix, done as part of this phase:**
`copy_to_system_clipboard`/`cut_to_system_clipboard`/`paste_from_
system_clipboard` (`window_input.rs`) rewritten to read `tree`/
`handlers`/`context_menus`/`root` through `self.active.borrow()`,
matching every other real `Window` method's established post-`show_
view` convention, instead of the plain `self.tree`/`self.handlers`/
`self.root` fields they used to read directly (stale after a real
`show_view` switch) -- the real, pre-existing gap this milestone's own
investigation found and the user explicitly chose to fix here rather
than defer.

Full chain green: `cargo check`/`clippy -D warnings`/`fmt --check`
clean across the whole workspace (zero `engine-core` changes, exactly
as the investigation predicted); `cargo test --workspace --release`
(`engine-core` unchanged at 227, every `engine-py`/`engine-md3`/
`engine-spec`/`engine-render`/`engine-platform` suite green -- zero
regression). `maturin develop --release`; a standalone smoke script (11
checks) confirmed every real path end to end before writing formal
tests: `event.source` still a plain `int`; `event.node` is a real
`Node` instance; a mutation made *through* `event.node` is visible on
the originally-registered handle (the real identity proof, since `Node`
has no `__eq__`/`id` to compare more directly); a handler can
immediately call `.animate()`/`.get_checked()`/`.set_on_hover_enter()`
back on `event.node` with no re-entrant-borrow panic; `event.node`
resolves correctly across `HoverEnter`/`HoverExit`/`FocusEnter`/
`Change`; a single handler shared across two `Checkbox`es correctly
tells them apart via `event.node`'s own live state; the fixed `copy_to_
system_clipboard` runs without panicking post-`active`-bundle fix.
`pytest tests/` (776 passed, 2 skipped -- byte-for-byte unchanged from
before this phase).
