# PLAN — Milestone 86: Data-Not-Paths Ingestion for Themes, Stylesheets, and Fonts

*(Replaces the `0.3.2` branch-scaffold plan. Full scope, design
decisions, and step list live in `BUILD_TRACKER.md`'s own "Milestone
86" section — this file is the working plan for the phase in flight.)*

## Goal

User: "Tesserae should not be pushing files directly to tre. It should
be pushing spec information and handling the files itself." Themes,
stylesheets, and fonts are the last three concerns where `tre` only
accepts a file path. Give each a data-shaped entry point; keep the path
forms as bare-`tre` convenience layered on top.

## Phases

1. **Theme and stylesheet specs:** `default_theme_spec=`/
   `custom_theme_spec=` on `View(...)`, `View.set_theme`, and
   `Window.set_theme`; `stylesheet_spec=` on `View(...)`. Each is
   depythonized into `ThemeSpec`/`Stylesheet` through `pythonize`, the
   same path `View(spec=)` uses. Each spec kwarg is mutually exclusive
   with its path counterpart (`require_at_most_one_content_source`).
   `resolve_theme_layers` and `Window.set_theme` take already-resolved
   `ThemeSpec`s, so path and spec share one code path after input
   resolution.
2. **Font registration:** process-global registry in `engine-render`,
   `tre.register_font(data: bytes) -> list[str]`, live renderers
   sync by generation counter.
3. **Docs and verification.**

## Status

Phase 1 in progress.
