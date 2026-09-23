# LOG — M68: Documentation Refresh: Full Content Pass

- User-directed: before shifting focus to the separate Tesserae UI
  framework project, "I want all documentation updated to match the
  current scope of the project. I want the MkDocs updated to reflect
  the current scope of the project" -- clarified via `AskUserQuestion`
  as a full content pass, not a targeted currency fix. Two parallel
  Explore agents investigated first: one audited every page under
  `docs/` against the current codebase, finding the site's content
  frozen at roughly milestone 27 ("27 milestones... as of v0.2.0")
  while the project is now at M67 and the real MD3 component catalog
  has grown to 56 factories -- `docs/guide/components.md` covered 5,
  with Terminal, CodeEditor, and every overlay lifecycle method
  (`open_dialog`/`close_dialog`, `open_menu`/`close_menu`,
  `open_navigation_drawer`, `open_side_sheet`, `open_snackbar`)
  undocumented anywhere.

## What shipped (single milestone, both phases)

1. `docs/guide/components.md` fully rewritten: all 56 real
   `Window.add_*` factories, organized into 12 real usage categories
   (Buttons & Actions, Selection & Input, Text Fields/Code Editor/
   Terminal, Progress & Status, Navigation & Shell Composition,
   Overlays, Cards/Lists/Chips/Structural Rows, Search, Media &
   Graphics, Date & Time Pickers, Layout & Structure, Theming) instead
   of the prior 5 hand-picked "hero" components -- real signatures
   extracted programmatically via a Python regex pass over
   `window_factory.rs`'s own `#[pyo3(signature = ...)]` attributes (56
   matched), not hand-copied or invented. Also documents the overlay
   lifecycle methods that existed in the real source but appeared in
   no doc anywhere before this.
2. `docs/guide/declarative-views.md`'s `style:` table widened for the
   real current `StyleSpec` fields shipped across M48-M61 (per-side
   padding/margin, flex-grow/shrink/basis, align-items/justify-content,
   border kwargs, token-ref `corner_radius`/`elevation` with the real
   `engine_md3::shape` vocabulary) -- its `kind:` list (`Rect`,
   `Container`, `Text`, `Checkbox`, `Slider`, `TextField`, `Image`) was
   verified **already correct** against `engine-spec/src/spec.rs`'s
   real `NodeKindSpec` enum rather than assumed stale and widened
   incorrectly; the declarative YAML layer is deliberately scoped to 7
   primitive kinds, and new docs needed to not imply otherwise.
3. `docs/guide/theming-and-accessibility.md` gained two entirely new
   sections -- shape/elevation token theming and typography theming --
   that had zero coverage before, despite both being real, shipped
   systems (M61-M63).
4. `docs/guide/docking-and-shell.md` gained a "Shell chrome widgets"
   section covering Tabs/NavigationRail/NavigationDrawer/Toolbar/
   TopAppBar/StatusBar alongside the existing dock/splitter/
   container-transform content.
5. `docs/guide/imperative-api.md` and `docs/api/python/window.md`/
   `node.md` widened: the real primitive factory table (pointing to
   `components.md` for the full catalog rather than a third duplicate
   table), the real `Event` payload's actual fields, `on_focus_enter`/
   `on_focus_exit`, `set_syntax_spans`/`set_folded_ranges`/
   `push_frame`/`set_clip_children`, and a new Overlays + Terminal
   section on `window.md`.
6. `docs/index.md`/`README.md`/`docs/architecture.md`: stale milestone
   counts corrected, feature lists widened to name the real 56-factory
   catalog, typography/shape theming, and layout/styling API breadth
   that were built but unmentioned. `ARCHITECTURE.md` §13's stale
   "Packaging (later, not needed for early development)" heading
   corrected to state the wheel matrix has existed since M21 -- the
   *new* automated-release mechanism itself is left for M69 to
   document, once it exists.
- **Two real errors caught and fixed during the writing itself, not
  just at plan time** -- the same "verify before documenting" discipline
  this project applies to code, applied here to prose: an early draft
  claimed `Event.source` was a `"mouse"`/`"keyboard"`/`"synthetic"`
  string; a direct read of `crates/engine-py/src/event.rs` found it's
  actually a stable, opaque `u64` node identity, unrelated to input
  source (`.node`, a separate field, is the live `Node` handle). A
  second early draft invented a `node.scroll_terminal(...)` method;
  grepping the real source found no such method exists anywhere --
  terminal scrolling is pure mouse-wheel input dispatch, and
  `resize_terminal`/`get_monospace_cell_size`/`copy_terminal_selection`
  are real `Window`-level methods, not `Node`-level as the plan had
  assumed. Both caught and fixed before anything was committed.
- Verification: `mkdocs build --strict` caught 6 real broken anchor
  links in `window.md` pointing at `components.md`'s old per-component
  headers (`#checkbox`, `#slider`, etc.), which the category-based
  rewrite removed -- fixed to the new category anchors, confirmed via
  the built site's own generated `<h2 id=...>` values, then a clean
  rebuild with zero warnings. Every non-trivial new Python snippet
  (buttons, overlays, the search-bar 4-tuple return, code editor
  folding/syntax spans, terminal spawn plus all 3 window-level
  terminal methods, carousel, badge, video `push_frame`,
  `set_clip_children`, focus handlers, and the `Event` payload's real
  fields) was actually run against the real built `tre` module in this
  session -- every one passed exactly as documented. Docs-only
  milestone, so the Rust/pytest verification chain doesn't apply.

## Status

**M68 is complete, both phases.** All documentation -- the MkDocs
site, `README.md`, and `ARCHITECTURE.md`'s stale operational claims --
now reflects the project's real current scope, with every non-trivial
new code example verified against the real build rather than
eyeballed. Committing locally now; push deferred pending explicit user
confirmation.

Next: M69 (Release Engineering: automated publish + standalone `.so`,
v0.3.0) -- the last item before shifting focus to Tesserae.
