# PLAN — M68: Documentation Refresh: Full Content Pass

*(Replaces the prior M67 plan in this file — M67 is complete,
committed. First of two milestones from the "wrap up before Tesserae"
request; see this file's own Milestone 68 section in `BUILD_TRACKER.md`
for the full real investigation.)*

## Goal
Bring all documentation -- the MkDocs site under `docs/`, `README.md`,
and `ARCHITECTURE.md`'s stale operational claims -- current with the
real scope of the project. The MkDocs site's content was frozen at
roughly milestone 27; the project is now at M67, with a 56-factory MD3
component catalog where the docs covered 5.

## Real investigation
Two parallel Explore agents: one audited every page under `docs/`
against the current codebase (recorded in full in `BUILD_TRACKER.md`'s
own M68 section), the other investigated the release mechanism for the
companion M69. Real signatures for all 56 `Window.add_*` factories were
extracted programmatically via a Python regex pass over
`window_factory.rs`'s own `#[pyo3(signature = ...)]` attributes, not
hand-copied.

## Design (1 milestone, 2 phases)
1. Guide pages & the component catalog.
2. API reference, overview pages, root docs.

## Status

**Complete, both phases.**

`docs/guide/components.md` fully rewritten -- all 56 factories across
12 real usage categories, plus the previously-undocumented overlay
lifecycle methods. `docs/guide/declarative-views.md`'s `style:` table
widened for the real current `StyleSpec` (per-side spacing, flex/align,
border kwargs, token-ref `corner_radius`/`elevation`); its `kind:` list
verified already-correct against `NodeKindSpec` rather than assumed
stale. `docs/guide/theming-and-accessibility.md` gained real typography
and shape/elevation token sections. `docs/guide/docking-and-shell.md`
gained shell-composition widget coverage. `docs/guide/imperative-api.md`
and `docs/api/python/window.md`/`node.md` widened for the real factory
catalog, the real `Event` payload fields, and previously-missing
methods. `docs/index.md`/`README.md`/`docs/architecture.md` milestone
counts and feature lists corrected. `ARCHITECTURE.md` §13's stale
"later, not needed for early development" Packaging framing corrected
to state the wheel matrix has existed since M21.

**Two real errors caught and fixed during writing itself, not just at
plan time:** an early draft claimed `Event.source` was a
`"mouse"`/`"keyboard"`/`"synthetic"` string -- direct source read of
`event.rs` found it's actually a stable opaque `u64` node id. A second
early draft invented a `node.scroll_terminal(...)` method that doesn't
exist -- terminal scrolling is pure mouse-wheel dispatch with no
Python-callable method, and `resize_terminal`/`get_monospace_cell_size`/
`copy_terminal_selection` are real `Window`-level methods, not
`Node`-level. Both fixed before any of it was committed.

Verification: `mkdocs build --strict` -- caught 6 broken anchor links
in `window.md` pointing at `components.md`'s old per-component headers,
which the category-based rewrite removed; fixed, then clean with zero
warnings. Every non-trivial new Python snippet (buttons, overlays, the
search-bar 4-tuple return, code editor folding/syntax spans, terminal
spawn plus all 3 window-level terminal methods, carousel, badge, video
`push_frame`, `set_clip_children`, focus handlers, and the `Event`
payload's real fields) was actually run against the real built `tre`
module in this session -- all passed as documented. Docs-only
milestone, so the Rust/pytest chain doesn't apply. `BUILD_TRACKER.md`
updated (Top Metrics, full Milestone 68 section, Just-closed/Up-next
refreshed), tracker regenerated (20 milestones/59 phases/155 items/3
known gaps/25 fixed gaps), artifact republished. Committing locally
now.

Next: M69 (Release Engineering: automated publish + standalone `.so`,
v0.3.0) -- the last item before shifting focus to Tesserae.
