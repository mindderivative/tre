# PLAN — M41 Phase 2: Correct Stale Documentation

## Goal
Close two real, cheap doc-hygiene bugs found by a dedicated v1-limits
inventory: `add_code_editor`'s doc comment still claimed three real
capabilities (syntax highlighting, line-number gutter, Tab-key
indentation) as deliberately deferred, even though all three were
built in full at M31; the early "Known gaps" list has several
un-struck-through bullets closed by later milestones. Also: remove the
confirmed-dead `AppHandler` trait.

## Steps
1. Verified each of the three `add_code_editor` claims directly against
   live source before touching the doc comment: `Node.set_syntax_spans`
   (real, `engine-py/src/node.rs:1070`); the real composed-gutter
   pattern (`examples/code_editor_gutter.py`, `engine-render/src/
   text.rs`'s own gutter-alignment test); real multiline-only `Tab`
   capture (`Tree::dispatch_text_field_key`). Rewrote the stale
   paragraph to name the real M31 phases that built each, kept the
   still-real deferred items (multi-cursor/minimap/bracket-matching/
   LSP) stated plainly.
2. Cross-checked four "Known gaps" bullets against this file's own
   later, authoritative sections before touching anything: culling/
   `VirtualList` scrolling (M8, all 3 phases), `ItemExtent::Variable`
   (M12 Phase 1), `material-colors` verification (M3 step 11), and a
   drop-zone-highlight bullet genuinely inconsistent with a later
   bullet in the same list that already recorded the M10 Phase 3 fix.
   Struck through and corrected all four.
3. Re-confirmed via grep immediately before deleting: `AppHandler`
   (`engine-core/src/input.rs`) has zero `impl`s and zero uses as a
   trait bound anywhere in the workspace -- a trait sketched at M4
   Phase 1 step 1's own original plan, superseded by the real
   `engine-py::dispatch.rs`'s `HandlerMap`/`call_handler` mechanism
   that actually shipped. Removed the trait, its crate-root re-export,
   and corrected every doc comment that cited it as the real current
   mechanism (`input.rs`'s own module doc, two `DispatchOutcome`
   variant docs, `Tree::dispatch`'s own doc, `lib.rs`'s own module
   doc) -- each now names the real mechanism instead. Deliberately left
   every *historical* mention of `AppHandler` untouched (e.g. M4 Phase
   1's own tracker entry, which accurately records it was declared at
   that step) -- only "still true today" claims were corrected.
4. Full verification chain: `cargo check`/`clippy -D warnings`/`fmt`/
   `cargo test --workspace --release` (unchanged counts -- pure doc/
   dead-code cleanup, no behavior change), `maturin develop --release`,
   full `pytest tests/`, all 77 examples, showcase demo.
5. `BUILD_TRACKER.md` Phase 2 (both steps... three steps) flipped to
   done. Artifact regenerated (41/131/224) and republished.

## Status
Complete. Full verification chain green (unchanged test/pytest/example
counts, as expected for a pure documentation and dead-code cleanup
pass). **M41 Phase 2 is done, and Phase 1 has since also closed: the
triggered `workflow_dispatch` run (`35486913410`) finished with all 12
jobs passing** (Linux manylinux, macOS × 5 Python versions, Windows ×
5 Python versions, `sdist`) -- the real cross-platform packaging
matrix still builds cleanly with today's full dependency set, 22
milestones after its last confirmed-passing run. **M41 -- Hardening
IV: Documentation Drift & Dead Code -- is now fully complete, both
phases.**
