# Demo: Phase 15 Step 15.1 -- Real Multi-Line/Word-Wrap Text Rendering

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase15_step15_1
../../.venv/bin/python demo.py
```

**What this proves.** The GUI Readiness assessment's recommendation 9
("multi-line text editing") turned out, on direct inspection of
`tre_engine::text::flatten_text`, to require building real multi-line
*rendering* from scratch first -- an earlier claim (in Step 13.5's own
docs and the readiness artifact) that multi-line label rendering
already existed was wrong; `flatten_text` shaped every `Text` shape as
one straight pen line with no `\n` handling at all. This step is that
missing rendering primitive: `tre.Text(..., wrap_width=...)` now
supports real hard-wrap (`\n`) and real greedy word-wrap by pixel width
-- the project owner's own explicit choice (over hard-wrap-only) when
asked how far v1 should go.

**Two real, previously-latent bugs found and fixed while building
this**, neither part of the new code itself:

1. **`descent`'s real sign.** `skrifa::Metrics::descent` is a signed
   OpenType value (this machine's default cascade font reports
   `ascent=1069, descent=-293`) -- negative, extending below the
   baseline. The first line-height formula written here,
   `ascent + descent + leading`, silently under-counted (adding a
   negative value shrinks the total), producing visibly overlapping
   lines. Confirmed via a real debug print against the real font before
   fixing to `ascent - descent + leading` -- exactly what the original
   Phase 15 plan had specified; the bug was in the code drifting from
   its own plan, not in the plan's own reasoning.
2. **`rustybuzz` cluster offsets across bidi paragraphs.** A real,
   pre-existing bug in `tre_text::shape::shape_run`, only now exposed
   because nothing before this step ever shaped genuinely multi-
   paragraph (`\n`-containing) text and relied on absolute `cluster`
   values across paragraph boundaries. `unicode_bidi::BidiInfo` treats
   `\n` as a real paragraph separator, so `segment_runs` already splits
   multi-line text into one run per line -- but `shape_run` shaped each
   run through its own fresh `rustybuzz::UnicodeBuffer`, and
   `rustybuzz` reports `cluster` relative to *that buffer*, not the
   original full string. Every run after the first therefore reported
   `cluster` values reset near zero instead of its own true absolute
   byte offset -- diagnosed by rendering `"one\ntwo\nthree"` and finding
   the third line rendered as a stray 6px sliver instead of a full real
   line. Fixed by offsetting `info.cluster` by `run.text_range.start` in
   `shape_run`. This also silently affected `caret_positions`/
   `EditableText.hit_test` for any pre-existing single-line caller that
   happened to pass multi-paragraph text through -- a real correctness
   fix, not scoped only to this step's own new code.

**Architecture**: new `tre_text::wrap::wrap_lines` (pure logic, unit-
tested with synthetic glyph data, no font/Python dependency, mirroring
`caret.rs`'s own testing precedent) does the real line-breaking:
`\n` always hard-breaks; word-wrap-by-width is applied within each such
segment only when a width is given (`None` degrades cleanly to hard-
wrap-only, identical to passing `f32::INFINITY`) -- one algorithm
handles both cases, not two diverging code paths. `tre_engine::Text`
gains a `wrap_width: Option<f32>` field (`None`, the default, keeps
every existing caller's rendering byte-identical); `flatten_text`
branches on it, calling `wrap_lines` and drawing each real line via
`RenderingCanvas::draw_text` with that line's own glyph sub-slice,
advancing `pen.y` by a real `line_height` between lines.

**Real, disclosed v1 scope limits** (all inherited from `wrap_lines`'
own doc comment): LTR/single-script text only; whitespace-boundary
word-wrap only (no UAX #14 hyphenation/punctuation/CJK-ideograph
breaks); left-aligned only; a single word wider than `wrap_width` still
gets its own line rather than being split further.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-text` gains 8 new
`wrap_lines` unit tests plus 2 new 2-D caret-function tests, all
passing; the pre-existing `shape`/`caret`/`fallback` suites still pass
unchanged after the `shape_run` cluster-offset fix). `--release` clean
apart from the same 5 pre-existing, already-flagged `debug_assert!`
failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this
step) -- one additional `tre-atlas` failure observed once under full-
workspace parallel load was confirmed flaky (passed in isolation and on
a clean rerun), not a real regression from this step's changes.

`demo.py`, run via `maturin develop --release`, against real GPU
hardware: `wrap_width=None` still renders as exactly one real ink band
(a byte-identical regression check against the pre-Phase-15 behavior);
three `\n`-separated lines render as exactly 3 real ink bands with
consistent measured spacing (44px at `px_size=32` on this machine's
real default cascade font); and a long sentence's own measured natural
width, force-wrapped at roughly 40% of that width, produces multiple
real lines whose spacing matches that same measured `line_height`
exactly -- one consistent layout, not two diverging code paths, proven
against real rendered pixels, not just "it runs."
