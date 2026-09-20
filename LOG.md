# LOG — M41 Phase 2: Correct Stale Documentation

- User's own explicit instruction: "Yes" to my recommendation (re-run
  wheels CI against current main, then fix the two doc issues and the
  dead-code item as one small pass), continued by "Continue hardening
  TRE v2" when asked which direction "let's move on" should take.
- Delegated a real, thorough Explore agent to inventory every "real,
  stated v1 limit" still open across the whole codebase, cross-checked
  against current source (not trusted from the tracker text alone --
  the agent's own report explicitly re-verified each older claim
  against later milestones before including or excluding it). **Real,
  decisive finding: almost everything found is a deliberate,
  already-argued scope choice** (Loading Indicator's simplified shapes,
  Time Picker Dial's missing labels, Terminal's no-strikethrough, the
  Wayland/Windows resize follow-ups, etc.) -- each already has a real,
  stated reason on record. Reopening any of them would be second-
  guessing an already-made call, not closing a real gap, so none were
  scoped. Two genuinely real, cheap doc-hygiene bugs were found
  instead, plus one confirmed dead-code item.
- **`add_code_editor`'s own doc comment (`window_factory.rs`)**: re-
  verified each of its three stale claims directly against live source
  before touching anything -- `Node.set_syntax_spans` is real
  (`engine-py/src/node.rs:1070`); the real line-number gutter is a
  composed sibling-`Text`-node pattern, demonstrated in `examples/
  code_editor_gutter.py`, lining up with the editor's own real per-
  line Y positions *by construction* (both go through the identical
  `shaped_layout` path); real multiline-only Tab-key indentation
  capture is in `Tree::dispatch_text_field_key`. All three were built
  in full at M31 (Phases 4, 1, 2 respectively) and the doc comment was
  never corrected, even though two *later* gaps (M38/M39 scroll
  fixes) on the same comment block DID get their own correction
  paragraphs -- a genuine, understandable miss, not a pattern of
  neglect. Rewrote the stale paragraph to name the real capabilities
  and where to find their own real design; kept the genuinely still-
  deferred items (multi-cursor, minimap, bracket matching, LSP
  integration) stated plainly, matching pyCopper's own identical v1
  scope for the identical reasons.
- **The early "Known gaps" list (`BUILD_TRACKER.md`'s own pre-M6
  section)**: cross-checked four bullets against this file's own
  later, authoritative sections before touching anything (not assumed
  from the agent's own report alone) -- confirmed culling and
  `VirtualList`'s real scrollable viewport both closed at M8 (all 3
  phases); `ItemExtent::Variable` closed at M12 Phase 1; `material-
  colors` verification closed at M3 step 11 (129/129 tests, cross-
  checked field-by-field); and found a genuine internal inconsistency
  -- a drop-zone-highlight bullet still marked "Still open, stated not
  silent" even though a *later* bullet in the very same list already
  correctly recorded its M10 Phase 3 fix. Struck through and corrected
  all four, matching this file's own established strikethrough-plus-
  correction convention used throughout for every other resolved
  bullet in the same section.
- **`AppHandler` (`engine-core/src/input.rs`)**: re-confirmed via grep
  immediately before deleting anything -- zero `impl`s, zero uses as a
  trait bound, anywhere in the entire workspace. A trait sketched at M4
  Phase 1 step 1's own original plan (a generic dependency-inversion
  hook for `DispatchOutcome::Activated`'s own meaning-dependent
  handling), superseded before it was ever implemented by the real
  mechanism that actually shipped: `engine-py::dispatch.rs`'s own
  `HandlerMap`/`call_handler`/`run_dispatch_outcome` (a concrete,
  per-`(NodeId, EventKind)` Python callback registry, no generic trait
  needed since only `engine-py` ever calls `Tree::dispatch` in
  practice). Removed the trait itself, its crate-root re-export
  (`lib.rs`), and corrected every doc comment that cited it as if it
  were the real current mechanism: `input.rs`'s own module doc
  (claimed "`engine-py` is the crate that actually implements it" --
  false, it never was), the two `DispatchOutcome::Activated`/`Changed`
  variant docs, `Tree::dispatch`'s own doc comment (`tree.rs`), and
  `lib.rs`'s own module doc. **Real, deliberate scope boundary:** left
  every genuinely *historical* mention of `AppHandler` untouched across
  the codebase and `BUILD_TRACKER.md` (e.g. M4 Phase 1's own tracker
  entry, which accurately records the trait *was* declared at that
  step -- a true statement about the past, not something to revise)
  -- only comments making a "this is real and current" claim about it
  were corrected, preserving this project's own honest chronological
  record rather than rewriting history.
- Full verification chain, all green: `cargo check --workspace --all-
  targets`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo fmt` + `cargo fmt --check`; `cargo test --workspace --release`
  (unchanged counts across every crate -- pure documentation and dead-
  code cleanup, zero behavior change); `maturin develop --release`
  rebuilt; `pytest tests/` (581 passed, 1 skipped, unchanged); all 77
  examples + showcase demo clean.
- `BUILD_TRACKER.md`: Phase 2 (all three steps) flipped to done.
  Parser confirmed balanced (41 milestones, 131 phases, 224 items,
  +1/+2/+4 exactly matching the one new milestone/two new phases/four
  new steps this pass added); artifact regenerated and republished.

**M41 Phase 2 is complete. Phase 1 -- triggering `wheels.yml` against
current `main` and confirming it still passes with today's full
dependency set -- is still in progress**: a real `workflow_dispatch`
run was triggered (`gh workflow run wheels.yml --ref main`, run
`35486913410`); macOS (all 5 Python versions) and `sdist` have already
passed, Linux and the remaining Windows jobs were still running as
this entry was written. Will update once the run finishes.
