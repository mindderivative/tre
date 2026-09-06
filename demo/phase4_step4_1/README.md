# Demo: Phase 4, Step 4.1 -- HarfBuzz & FreeType Integration

```bash
./demo/phase4_step4_1/run_text_shaping_demo.sh
```

**An all-pure-Rust font stack, a deliberate departure from
IMPLEMENTATION.md's literal "HarfBuzz"/"FreeType" wording** (see
`documentation/REVIEW.md`'s "Phase 4 Step 4.1 Implementation" section and
`planning/archive/PLAN_PHASE4_STEP4_1.md`): `rustybuzz` (a faithful port
of HarfBuzz's own shaping algorithm) replaces HarfBuzz, and `skrifa`
(Google Fonts' `fontations` project) replaces FreeType for outline
extraction -- zero new C library dependencies anywhere in this workspace,
whose only remaining C ABI boundary is Vulkan itself.

**Real bidi + script run segmentation, not just "call rustybuzz and hope"**
-- `tre-text` splits a mixed-direction, possibly-mixed-script string into
the direction- and script-uniform runs `rustybuzz::shape` actually needs
(via `unicode-bidi` for embedding levels and `unicode-script` for real
UAX #24 script lookups), in the correct *visual* order. This demo shapes
`"he" + Alef + Bet` (Latin followed by two Hebrew letters) and confirms
the Hebrew run's shaped glyphs come back in descending (visually reversed)
cluster order -- real RTL behavior, not assumed.

**Real `fontconfig`-driven fallback, not a hardcoded font list** -- the
demo discovers this machine's actual installed fonts (`sans-serif`,
`Noto Sans`, `emoji` generic families) and proves a real fallback: U+1F9E0
(the "brain" emoji), confirmed via `fc-query`'s own charset dump to be
absent from both likely `sans-serif` resolutions on this project's
development and CI machines (DejaVu Sans, Noto Sans) and present in Noto
Color Emoji, actually resolves to the cascade's emoji entry, with a real
(non-`.notdef`) glyph ID -- not a cosmetic no-op cascade that always picks
the primary font.

**Real glyph outline extraction, rendered as proof.** `tre-text` extracts
`'L'`'s true outline from whichever real font this machine's fallback
cascade resolved as primary, via `skrifa` (the same pure-Rust library
replacing FreeType). Deliberately a hole-free glyph (`'L'`, not `'O'` or
`'B'`) -- a glyph with a counter needs multi-contour winding this step
doesn't build (Step 4.2's MSDF approach handles that natively). The
extracted outline is flattened via `tre-svg`'s now-`pub`
`flatten_cubic`/`flatten_quad` (Phase 3 Step 3.3.1, reused rather than
reimplemented), triangulated by the existing ear-clipping tessellator, and
rendered through the unmodified flat-color pipeline -- no MSDF, no atlas,
no new shader; that's Step 4.2.

**Verified by an independently-computed point-in-polygon check, not just
"it compiled."** Since a real font's glyph shape isn't known in advance
the way a hand-authored SVG shape is, this demo computes its own even-odd
ray-casting containment test directly against the extracted-and-flattened
outline (rather than a pre-verified external script) before rendering,
confirms its chosen "inside the stroke" and "bounding-box center" probe
points land where expected, and only then asserts the real GPU-rendered
pixels match.

**A real shaped *word*, not just one letter.** A single glyph proves
outline extraction works, but not that shaping actually lays out a
sequence of glyphs correctly -- the part of "getting text right" that
matters most in practice. The demo additionally shapes the real word
`"TEXT"` (all four letters independently confirmed single-contour/
hole-free in this machine's real primary font before writing this
section) and renders it with every letter positioned purely by
`rustybuzz`'s own real per-glyph advances -- no hand-placed offsets. Seven
probes are checked, all computed independently (ray-casting against the
actual extracted-and-positioned polygons, not assumed): each letter's own
bounding-box center, plus the gap between every adjacent pair of letters
(checked against *every* letter's outline, not just its neighbors, so an
advance bug that made two letters overlap would be caught here too).
Written to a second PNG so the whole word is visible at once.
