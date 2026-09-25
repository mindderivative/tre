# PLAN — Branch `0.3.3`: Milestone 90, Consistent Property Naming

*(Replaces the M87 plan — M84 through M89 are complete, `v0.3.2` is
released. Full scope, the audit findings table, and the step list live
in `BUILD_TRACKER.md`'s "Branch: 0.3.3" and "Milestone 90" sections.)*

## Goal

A breaking rename so one concept has one name across the imperative
(`Window.add_*`, `Node`) and declarative (`WidgetSpec`) APIs: `foreground`
for glyph/text color, `background` only for real fills, MD3's own
`checked`/`selected` state names, `value` for a slider, one
`orientation` parameter, lowercase `snake_case` enum values, and
consistent text-string parameter names.

User's versioning call: this ships as `0.3.3`; `0.4.0` is reserved for
the `vello_hybrid` fork (issue #4).

## Phases

1. Renames in `engine-spec` and `engine-py`, with migration errors that
   name each replacement.
2. Tests, all examples, `demo/showcase.py`.
3. Docs, including a "Migrating to 0.3.3" page.
4. Full verification chain, then the Tesserae handoff.

## Status

**Scoped, not started.** Findings A–H are recommendations awaiting the
user's confirmation before implementation.
