# Log: M27 Phase 5 — Accessibility Pass & Polish, closing M27

A real, comprehensive keyboard-Tab-order sweep across every screen's
own real interactive controls — replacing the partial spot-checks
earlier phases individually did (gallery: 2 of 8 controls checked;
motion/data: 0 checked) with a full sweep of all 8/5/3 real controls
per screen, each confirmed to become focused in the exact order it was
attached. Plus real keyboard *operability*, not just reachability, on
one representative control per screen via a genuine `Enter` key press
— the same `DispatchOutcome::Activated` mechanism a real mouse click
already produces, routed through the identical registered `Click`
handler (not a hand-rolled duplicate of it): the gallery's `Checkbox`,
the motion screen's `Fade` trigger, the data screen's `Load next page`
button.

**Real finding, confirmed empirically before writing the sweep, not
assumed:** the node focused before a real screen swap is gone once its
whole screen is removed, and focus resets to none — the very next Tab
press after switching screens lands on the *first* focusable node in
the entire window again (nav button 1), not straight into the new
screen's own content. Every screen's own sweep accounts for this by
consuming the 3 nav-button Tab stops again first (not re-asserting
them — already proven reachable once, at the very start of `main()`).

**Real code-consistency polish:** the identical `label(text, x, y,
...)` closure all three screen builders each defined locally,
separately, was factored into one shared `make_label_fn(window,
screen)` factory — real duplication removed, not just noticed.

**A real, honest correction:** M27's own Phase 5 scoping text (written
before Phase 4's implementation settled) said "a final visual-
consistency pass across all *four* content screens" — the demo
actually has *three* (components, motion, data). Corrected here rather
than silently left wrong.

Also made the finished demo discoverable — nothing in the public docs
or README previously mentioned it existed: added a pointer in
`README.md`'s own "Getting started" section and `docs/index.md`'s own
"Where to go next" section, and corrected `docs/index.md`'s stale "25
milestones" claim to the real current 27 (M26/M27 landed since that
text was written).

Full `cargo test --workspace --release`/clippy `-D warnings`/fmt clean
(no Rust code changed this phase — pure Python + docs). `maturin
develop --release` + `pytest tests/` (187 passed, unchanged, 1
pre-existing skip), all 33 pre-existing examples, and the updated
demo — run 5 times in a row to rule out any flakiness in the new
keyboard-focus logic — all confirmed clean with the real display.
`mkdocs build --strict` clean.

M27 Phase 5 — Accessibility Pass & Polish is now complete, closing
Milestone 27 entirely. All 5 phases (shell & nav, MD3 gallery, motion
& custom drawing, data & layout, accessibility & polish) are real,
tested, and composed into one running showcase app.
