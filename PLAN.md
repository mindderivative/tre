# PLAN — Branch `0.3.5`: Milestone 98, Remove the Declarative Layer

*(Replaces the M97 Phase 2 plan — Steps 1–5 are done and handed to Tesserae,
whose Step 6 run cleared M98. Every step is in `BUILD_TRACKER.md`.)*

## Goal

Take the declarative layer out of `tre`: the `engine-spec` crate, `View`,
`Component`, the `{{ }}` binding evaluator, and the reactivity layer
(`Signal`, `Computed`, `Effect`, `ViewModel`, `batch`, `untrack`), with their
tests, examples, and docs. Tesserae owns all of it now.

## Decision (the user, 2026-09-25)

`Window.set_theme` stays until M99, which removes it with the MD3 widgets it
themes. It's built on `engine-spec`'s theme types, so those move into
`engine-py` (without the View-only `styles:` part), and `serde_yaml_ng` and
`pythonize` — which parse its theme files and theme dicts — stay until M99
too. Only `notify` (the view-file watcher) goes now.

## Steps

1. **Rust.** Delete `crates/engine-spec`. In `engine-py`, delete `view.rs`,
   `component.rs`, and `binding.rs`, the dependency-recording functions,
   and `Window.from_view`/`show_view`; move the theme types `set_theme`
   needs into a new `theme_spec.rs`. Drop `engine-render`'s `spec_view`
   test. Workspace builds, clippy clean.
2. **Python, tests, examples, docs.** `tre/__init__.py` loses the
   reactivity layer; `_core.pyi` loses `View`, `Component`, and the
   recording functions. Delete the declarative tests, examples, their YAML,
   `tools/migrate_views_0_3_3.py`, and the showcase's declarative phase;
   trim mixed tests to their imperative cases. Remove the View, reactivity,
   and declarative-guide pages and fix every link to them. The design pages
   stay as the record, noting where their pinning tests went.
3. **Full standing chain,** then the tracker and memory.

## Status

Scoped (2026-09-25). Step 1 next.
