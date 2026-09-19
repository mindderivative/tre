# LOG — M35 Phase 1: Toolbars

- User instruction "Start on the next milestone" was ambiguous
  (the vello_hybrid fork documented last turn was the only concrete
  candidate on the tracker) -- clarified via `AskUserQuestion` before
  acting: the user wanted a genuinely fresh milestone, not the fork.
- Real investigation, the same discipline M30/M32 were originally
  scoped with: delegated research cross-referenced TRE's existing
  52-method `add_*` catalog against MD3's own current official
  catalog (local scraped mirror at `/home/phil/pyDev/projects/
  pyCopper/M3-References/*.md` -- `m3.material.io` itself is
  JS-rendered, unreachable via WebFetch) and pyCopper's own real
  widget set. Ruled out three false candidates: Navigation Bar/Bottom
  Sheet/Bottom App Bar (M30 already deliberately excluded these,
  mobile-only patterns); a Menu container (already exists, `build_
  menu`/`open_menu`); a full Date Picker container (the day-cell
  primitive + grid math already exist, only chrome missing). Five
  real ranked gaps remained; presented via `AskUserQuestion`. User
  chose Button Groups + Split Button + Toolbars.
- Real anatomy read directly from the local spec mirror before
  designing anything: `COMPONENT_TOOLBARS.md`, `COMPONENT_SPLIT_
  BUTTONS.md`, `COMPONENT_BUTTON_GROUPS.md`. Real, honest phase
  ordering established: Toolbars first (pure composition, direct
  sibling of the already-built `add_top_app_bar`), Split Button
  second (composes existing Button/build_menu, reuses the existing
  shape-morph (M7 Phase 4) + Animated<Affine> transform (M5 Phase 1)
  mechanisms), Button Groups last (the Standard variant's real
  "one button's state reflows its siblings' widths" mechanic is
  novel at the UI level but has a real, directly reusable
  architectural precedent -- `Tree::sync_carousel_layouts`, M30 Phase
  9 Step 5's "container state drives every child's real layout_style"
  shape) -- avoided a genuinely-novel-capability `AskUserQuestion`
  pause since a real, concrete precedent exists to ground the design
  in, mirroring M33's own precedent for when NOT to pause.
- Implemented `Window.add_toolbar` in `window_factory.rs`, directly
  after `add_top_app_bar` -- read that function first as the real
  structural template: `resolve_fab_colors`'s own `match variant {
  ... other => Err(PyValueError) }` pattern, `role(name, fallback)`
  theme-resolution closure, `positioned_style` construction. Real
  color tokens confirmed present via grep before use:
  `surface_container`/`primary_container` both real, already-verified
  MD3 roles in `engine_md3::color`.
- Real, deliberate API design: no specialized children-list parameter
  -- `add_toolbar` returns a plain container `Node`; the caller
  composes any already-built node in via the existing, generic `Node.
  add_child` (M6 Phase 1), matching MD3's own real "a container with
  configurable slots" anatomy verbatim and the same "engine provides
  the primitive, app composes" split `clip_children` (M32 Phase 3)
  established.
- Real, honest gap stated directly in the doc comment: the spec names
  "floating toolbars have elevation by default" with no discrete
  numeric token anywhere in the scraped pages -- reused `FAB_REST_
  ELEVATION_LEVEL` (3.0), this catalog's own closest real "floating,
  elevated chrome" reference, rather than inventing a number.
- One real, deliberate rejection: a vertical *docked* toolbar raises
  a clear `ValueError` instead of silently ignoring `orientation` --
  MD3's own anatomy has no such variant (docked toolbars are always
  full-width and horizontal).
- Compiled clean on the first `cargo check` attempt. One real clippy
  fix needed: `f64::from(FAB_REST_ELEVATION_LEVEL)` was a useless
  conversion since the constant is already `f64` -- fixed by removing
  the wrapper.
- Real, direct empirical script run before writing any pytest: docked
  default, floating/vertical/vibrant, composing an already-built
  `Button` in via `add_child`, and all three real validation errors
  (unknown variant/orientation/color) plus the vertical-docked
  rejection -- all passed on the first run.
- Wrote `tests/test_toolbar.py` (10 tests, modeled on `test_top_app_
  bar.py`'s own established style) and `examples/toolbar.py` -- both
  checked for filename collisions first (none).
- Full verification: `cargo check --all-targets`/`cargo clippy
  --all-targets -D warnings`/`cargo fmt --check` clean, `cargo test
  --workspace --release` clean (unchanged counts -- pure engine-py
  composition, no new engine-core/engine-render pure-logic surface),
  `maturin develop --release` rebuilt, `pytest tests/` 537 passed/1
  skipped (10 new, up from 527, zero regressions), all 72 examples
  (including the new `examples/toolbar.py`) and the showcase demo
  re-run clean, `mypy --strict` clean against `examples/toolbar.py`.
- Updated `BUILD_TRACKER.md` (M35 scoped with all 3 phases; Phase 1
  closed; Top Metrics row updated to 33%/in-progress) -- verified the
  parser's own reported item count before/after (34/110/200 ->
  35/113/203 when scoping, unchanged when closing Phase 1 since that
  only flips an existing item's own status marker), regenerated and
  republished the Build Tracker artifact. **This closes M35 Phase 1
  only -- M35 itself stays open, Phase 2 (Split Button) and Phase 3
  (Button Groups) remain.**
