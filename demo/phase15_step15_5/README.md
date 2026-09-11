# Demo: Phase 15 Step 15.5 -- Advanced Line-Breaking (real UAX #14)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase15_step15_5
../../.venv/bin/python demo.py
```

**What this proves.** Recommendation 13's line-breaking half.
`tre_text::wrap_lines` -- the algorithm behind `tre.Text(...,
wrap_width=...)` (Step 15.1) and `tre.EditableText`'s own layout (Step
15.2) -- is rebuilt on the real [`unicode-linebreak`][crate] crate,
implementing the actual [Unicode Standard Annex #14][uax14] line-
breaking algorithm. Step 15.1's own v1 hand-rolled its break
opportunities from `char::is_whitespace`/`\n` alone; this replaces that
with the real, established rule set (hyphens, punctuation attachment,
CJK-ideograph boundaries, and more), the same "use the real algorithm,
don't hand-roll UAX rule tables" precedent this crate's own bidi
(`unicode-bidi`) and script (`unicode-script`) handling already follow.

[crate]: https://docs.rs/unicode-linebreak
[uax14]: https://www.unicode.org/reports/tr14/

**The algorithm.** `wrap_lines` walks `unicode_linebreak::linebreaks`'s
own break-opportunity stream (`Allowed` or `Mandatory`, at real byte
positions) instead of a hand-rolled Word/Whitespace/Newline tokenizer.
It greedily accepts each break as a *candidate* for closing the current
line, and only actually closes the line -- possibly retrying against an
*earlier* candidate first -- once a break's own content would overflow
`max_width`. `WrappedLine`'s public shape and `wrap_lines`'s public
signature are unchanged; only the internal break-opportunity source
changed, so `flatten_text` (Step 15.1) and `compute_layout` (Step
15.2/15.4) needed no changes at all.

**Two real design flaws were found and fixed during design, before any
code was written** (by hand-tracing the new algorithm against all 8 of
`wrap.rs`'s pre-existing unit tests):

1. A `Mandatory` break must not close the line unconditionally at its
   own position -- if the segment since the line's own start overflows
   `max_width`, an earlier accepted `Allowed` candidate must be used to
   close the line first. Traced concretely: `"aaaa bbbb"` at
   `max_width=45` would otherwise produce one 90-wide overflowing line
   instead of the correct two 40-wide lines. Fixed via a retry loop
   applied uniformly to both break kinds, not a special case for
   `Mandatory` alone.
2. When `\n` is a text's own final character, `unicode_linebreak` emits
   only ONE break at that shared position (`\n`'s own mandatory break
   coincides exactly with "end of text"), which would silently
   under-produce lines by one relative to this project's own established
   "N trailing newlines -> N+1 lines" convention (matching every real
   editor: pressing Enter at the end opens a new, currently-empty line).
   Fixed via a post-loop `text.ends_with('\n')` special case.

**A third real bug, found only once machine-verified** (not caught by
hand-tracing): `unicode_linebreak::linebreaks("")` yields nothing at
all -- its "always at least one final break" guarantee only holds for
non-empty input, contradicting what its own docs implied. This broke
the pre-existing `empty_text_produces_exactly_one_empty_line` test the
very first time `cargo test` ran against the new code. Fixed with a
direct early return for `text.is_empty()`, confirmed by re-reading the
crate's real behavior in an isolated scratch check
(`unicode_linebreak::linebreaks("")` returns `[]`) before writing the
fix, not by assumption.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-text` gains 4 new
`wrap.rs` unit tests beyond the 8 pre-existing ones, which all still
pass unchanged: two real UAX #14 capability tests -- a break right
after a hyphen with no whitespace at all, and a forbidden break between
a word and its own trailing `!` -- plus the empty-text fix's own
regression test and a test proving a `\n` glyph with a real nonzero
advance is still excluded from both lines it borders). `--release`
clean apart from the same 5 pre-existing, already-disclosed
`debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`,
unrelated to this step).

`demo.py`, run via `maturin develop --release` against a real headless
GPU renderer, reusing Step 15.1's own "scan several real `render()`
calls for real ink bands" technique:

- **The new capability, proven directly against real pixels**:
  `"wellknown-example"` has no whitespace anywhere, so it has exactly
  one real UAX #14 break opportunity -- right after the hyphen. Forcing
  `wrap_width` to half its own measured natural width produces exactly
  2 real ink bands, split at the hyphen. Step 15.1's own v1 algorithm
  could never wrap this text at all, at any width, since it had no
  whitespace boundary to break at.
- **A full regression suite**, reproducing Step 15.1's own three checks
  verbatim against the new algorithm: `wrap_width=None` still renders
  as exactly one real line; three `\n`-separated segments still render
  as exactly 3 real, evenly-spaced ink bands; and whitespace word-wrap
  of a long sentence still splits across multiple real lines at the
  same real line spacing as the hard-wrap case.

**Real, disclosed remaining scope**: still LTR/single-script only, still
left-aligned only (both inherited, unchanged limits from Step 15.1); no
hyphenation of an already-unbroken word (a single unbreakable token
wider than `max_width` still gets its own line); this step covers
*where* a line is allowed to break, not bidi-aware or justified layout,
which remain out of scope for this project's v1.

With this step, all five sections of GUI-readiness recommendations
9/12/13 -- multi-line rendering, multi-line editing, Shift+arrow
selection, word/line jumps, and advanced line-breaking -- are complete.
