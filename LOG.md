# LOG — Milestone 86, Phase 3: Docs + Verification

- User-directed: "Start M86". Phase 1 (`babef68`) added `*_spec=` dict
  kwargs for themes and stylesheets; Phase 2 (`abcba45`) added
  `tre.register_font(bytes)`.

## What shipped

1. `docs/api/python/view.md`: `View(...)`/`set_theme` signatures and
   the `*_spec=` forms, with an example.
2. `docs/api/python/window.md`: `set_theme` signature and a
   `custom_theme_spec=` example.
3. `docs/api/python/index.md`: new "Module functions" table listing
   `register_font`.
4. `docs/guide/theming-and-accessibility.md`: new "Themes as data" and
   "Custom fonts" sections.
5. `docs/guide/declarative-views.md`: a note pointing stylesheet users
   at the data forms.
- Verification: `mkdocs build --strict` clean; `cargo test --workspace
  --release` 47 suites, 536 passed, 0 failed; `pytest tests/` 914
  passed, 2 skipped; all 88 examples plus `demo/showcase.py` run clean.

## Status

**M86 complete.** Every concern `tre` ingests now has a data-shaped
entry point. Tesserae's consuming half (its own M29, phase 4) is
unblocked. Nothing further is scoped on the `0.3.2` branch.
