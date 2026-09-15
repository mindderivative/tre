# Demo: Phase 15 Step 15.4 -- Word/Line Caret Jumps

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase15_step15_4
../../.venv/bin/python demo.py
```

**What this proves.** Recommendation 13's word/line jump half.
`tre.EditableText` gains `move_caret_word_left`/`move_caret_word_right`
(Ctrl+arrow) and `move_caret_line_start`/`move_caret_line_end`
(Home/End). All four take the same `extend: bool` parameter Step 15.3
already established, sharing the identical `apply_caret_move`
selection-anchor rule -- Ctrl+Shift+arrow and Shift+Home/End work for
free, with no new anchor logic.

**Word jumps are real Unicode UAX #29 segmentation**, via a new
`tre_text::word` module wrapping `unicode-segmentation`'s
`unicode_word_indices` (already a transitive dependency at 1.13.3;
added directly since `tre-text` now calls it itself). Pure string
logic -- no shaping/layout at all, the same as `move_caret_left`/
`right`. `unicode_word_indices` (not `split_word_bound_indices`) was
picked deliberately: it already filters out pure whitespace/
punctuation segments, so Ctrl+Right lands on the end of the next real
word directly, skipping intervening punctuation/whitespace entirely
rather than stopping at each one.

**Line jumps need real layout** (via `compute_layout`, the same as
`move_caret_up`/`down`), plus a new shared helper, `current_visual_line`
-- the largest line index whose own `byte_range.start <= caret`.
Derived directly from `lines` alone (no shaped positions needed), it's
equivalent to (and replaces) the old "scan shaped positions for an
exact match, fall back to `line_of`" logic `move_caret_vertically`
used before this step, at a real boundary tie it resolves the same way
`move_caret_vertically` already did: toward the *later* tied line.

**A real, disclosed design subtlety caught before it became a bug**:
`move_caret_line_end` does **not** simply use `lines[N].byte_range.end`.
That value includes any real trailing whitespace/newline the line's own
wrap consumed (see `wrap_lines`' own doc comment) -- which is *also*
line N+1's own start byte, the exact real tie `move_caret_vertically`
always resolves toward the *later* line. Landing `End` exactly there
would make a following `move_caret_up`/`down` treat the caret as
already on the *next* line, not the one `End` was just pressed on.
Fixed by trimming real trailing whitespace from the line's own text
before measuring its end, landing right before the line's own `\n`
instead -- at the real, disclosed cost that deliberately-typed trailing
whitespace (rare) is skipped too, matching how most real editors
already visually collapse it. `move_caret_line_start` needed no such
fix: `byte_range.start` is never itself the tied value in that
direction.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-text` gains 5 new
`word.rs` unit tests -- real UAX #29 boundaries over plain strings, no
synthetic glyph data needed at all, unlike `caret.rs`'s own tests).
`--release` clean apart from the same 5 pre-existing, already-flagged
`debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`,
unrelated to this step).

`demo.py`, run via `maturin develop --release`: Ctrl+Right/Left across
`"the quick brown fox"` land at the exact hand-computed word
boundaries (`[3, 9, 15, 19]` forward, `[16, 10, 4, 0]` back), then real
no-ops past either end; word jumps correctly treat a real 2-byte `é`
and a mid-word apostrophe as part of one word while skipping `", "`
entirely; Ctrl+Shift+Right extends a selection word by word from one
fixed anchor; Home/End on a real hard-wrapped middle line land at its
own exact content boundaries, then real no-ops; and the disclosed tie-
avoidance fix is proven directly: pressing End on a non-last line then
Down moves to the *next* line, not two lines down.

**Real, disclosed remaining scope**: advanced line-breaking (real UAX
#14 punctuation/hyphenation/CJK-ideograph break opportunities, beyond
`wrap_lines`' current whitespace-boundary-only rule) is recommendation
13's other half, tracked separately.
