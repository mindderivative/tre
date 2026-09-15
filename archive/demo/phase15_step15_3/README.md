# Demo: Phase 15 Step 15.3 -- Shift+Arrow Selection Extension

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase15_step15_3
../../.venv/bin/python demo.py
```

**What this proves.** Recommendation 12's first half from the GUI
Readiness assessment: real Shift+arrow selection extension. `tre.
EditableText` gains `move_caret_left`/`move_caret_right` -- real,
brand-new horizontal movement; this class had **no** horizontal
movement at all before this step, only `set_caret` with a caller-
supplied byte offset. Phase 15 Step 15.2's `move_caret_up`/
`move_caret_down` gain a new `extend: bool` parameter. All four share
one real selection-anchor rule, `apply_caret_move`: `extend=False`
always clears any active selection (matching `set_caret`'s own
convention); `extend=True` starts a new selection anchored at the
caret's own current position the first time it's used, then leaves
that anchor untouched on every subsequent extend, in either direction
-- the real Shift+arrow convention every desktop text field already
has (extend past the anchor and back, and the anchor never moves).

`move_caret_left`/`move_caret_right` are pure string operations --
UTF-8-char-boundary safe (the same safety `delete_backward` already
established), needing no shaping/layout at all, unlike `hit_test`/
`move_caret_up`/`down`. Moving right past the end of one visual line
crosses into the next for free, since this operates on the flat
underlying string, not per-line.

**Real, disclosed v1 design choice, not an oversight**: a plain
(non-extend) arrow always clears the active selection and moves one
real character from the *current* caret -- it does not "collapse to
the selection's own edge" the way some real editors do. This matches
`move_caret_up`/`down`'s own convention, already shipped and tested in
Step 15.2, rather than introducing a second, inconsistent behavior for
horizontal movement only. Collapse-to-edge is real, disclosed follow-up
work if a caller needs it.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug. `--release` clean apart
from the same 5 pre-existing, already-flagged `debug_assert!` failures
(2 in `tre-engine`, 3 in `tre-memory`, unrelated to this step). No
native Rust unit tests exist for these new `PyEditableText` methods --
matching Step 13.5's own already-established, disclosed constraint (the
`extension-module` PyO3 feature doesn't link `libpython`, so native
`cargo test` cannot call GIL-acquiring functions); real coverage lives
in `demo.py`.

`demo.py`, run via `maturin develop --release`: `move_caret_left`/
`right` are real UTF-8-char-boundary-safe (tested against `"café"`'s
own real 2-byte `é`, not a byte-unsafe skip) and real no-ops at the
very start/end of `text`; 5x `move_caret_right(extend=True)` from byte
0 anchors at 0 and selects exactly `"hello"`; reversing direction with
`move_caret_left(extend=True)` keeps the *same* anchor and shrinks the
selection correctly, including shrinking all the way back to an empty
selection at the anchor itself and then a real no-op past it; a plain
`move_caret_right()` afterward clears the selection and moves one real
character from the current caret; `move_caret_down(extend=True)` x2
lands the caret exactly at the next line's own start (a real,
zero-width selected segment there -- `selection_rects()` correctly
reports 2 rects, not 3, until one more real character is selected on
that third line, at which point it reports exactly 3); and
`move_caret_up(extend=True)` afterward keeps that same anchor while
shrinking the selection by one real line.

**Real, disclosed remaining scope limit**: `wrap_lines`' own v1 scope
(LTR/single-script only, whitespace-boundary word-wrap only, no UAX #14
hyphenation/punctuation/CJK-ideograph breaks, left-aligned only) still
applies, since this step builds directly on Step 15.1/15.2's existing
layout. Word-by-word (Ctrl+arrow) and line-start/end (Home/End) jumps
are real, separate follow-up work, not attempted here.
