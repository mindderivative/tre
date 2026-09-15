# Demo: Phase 15 Step 15.2 -- Multi-Line Text Editing (`EditableText`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase15_step15_2
../../.venv/bin/python demo.py
```

**What this proves.** The editing layer on top of Step 15.1's real
multi-line/word-wrap rendering: `tre.EditableText(..., wrap_width=...)`
gains `line_count()`, `hit_test_2d(x, y)`, `move_caret_up()`/
`move_caret_down()` (real "sticky column" vertical movement), and
`selection_rects()`. No GPU renderer is needed for this demo --
everything exercised here is pure computation (shaping + line-breaking
via `tre_text::wrap_lines`), not an actual render.

`insert`/`delete_selection`/`delete_backward`/`set_caret`/
`set_selection`/`copy`/`cut`/`paste`/`handle_ime` are all deliberately
UNCHANGED by this step -- they only ever touch the flat byte string,
never line layout, so there is no duplicated editing logic for
multi-line text. The cut/copy/paste check in `demo.py` proves that
reuse holds correctly across a real multi-line selection, not just
single-line.

**Two more real bugs found and fixed while building this**, both in
brand-new code from this step:

1. **Reconstructing a pixel `y` to re-derive an already-known line.**
   The first `move_caret_vertically` routed through `hit_test_2d` by
   computing a target `y` at `(target_line + 0.5) * line_height` and
   handing it back to `hit_test_2d`'s own `y -> line` resolution.
   `f32::round()` rounds half *away from zero*, so `1.5.round() == 2.0`
   -- a `y` placed exactly at a line's own midpoint rounds to the line
   *below* it, silently skipping a real line on every other move. Since
   the target line was already known exactly, reconstructing a `y` for
   it at all was redundant; fixed by searching the already-known target
   line's own stops directly for the nearest `x`, with no `y`/rounding
   involved.
2. **`hit_test_2d`'s own `y -> line` formula was wrong for genuine
   mid-line `y` values, independent of bug 1.** It used
   `(y / line_height).round()`, which finds the line whose *top* is
   nearest -- not the line whose own `[n*line_height, (n+1)*line_height)`
   span actually contains `y`. That misattributes roughly the bottom
   half of every line's real vertical extent to the line below it. Every
   existing test happened to use `y` values sitting exactly on a line
   boundary, so this went uncaught until a genuine mid-line `y` was
   added. Fixed to `.floor()`, plus a new regression test using real
   mid-line `y` values (not just boundary-exact ones) to catch this
   class of bug going forward.
3. **A real, found line-tie-break inconsistency** (found via `demo.py`
   itself, a genuine `down, down, up, up` repro landing one line short
   of where it started): at a byte offset sitting exactly at a line
   boundary, `tre_text::line_of` always prefers the *earlier* of the two
   tied lines -- correct for its own "what line is my caret visually on"
   contract, but wrong for repeated vertical movement landing exactly on
   such a boundary (a fresh line's own first byte). An initial fix tied
   the tie-break to the *current* move's own direction (later for down,
   earlier for up), which seemed reasonable but was internally
   inconsistent between a `down` that arrives at a boundary and the
   following `up` that has to leave from it. The real, general rule: a
   caret at a boundary visually renders right before the *later* line's
   own first glyph, never tucked invisibly at the end of the earlier
   line -- so `move_caret_vertically` always prefers the later of any
   tied lines, for both `move_caret_up`/`move_caret_down`, independent
   of which direction the current move happens to be. This single, fixed
   rule stays self-consistent across any real sequence of moves.

**Architecture**: `PyEditableText` gains `wrap_width: Option<f32>`
(constructor param + field, `None` by default -- zero change to every
existing single-line construction) and a private `compute_layout` that
reshapes + rewraps `text` fresh each call, matching `hit_test`'s own
established "cheap, re-derive every time" precedent -- shared by all
four new methods rather than duplicated per method. `selection_rects()`
returns one `(x, y, width, height)` tuple per real visual line the
active selection spans, letting a Python caller build one `Rectangle`
per rect for real selection highlighting -- the exact design the
original Phase 13 plan already called for.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-text` gains 1 new
`hit_test_2d` regression test using genuine mid-line `y` values, on top
of Step 15.1's existing coverage). `--release` clean apart from the
same 5 pre-existing, already-flagged `debug_assert!` failures (2 in
`tre-engine`, 3 in `tre-memory`, unrelated to this step). No native
Rust unit tests exist for `move_caret_vertically`/the other new
`PyEditableText` methods themselves -- matching Step 13.5's own already-
established, disclosed constraint: the `extension-module` PyO3 feature
doesn't link `libpython`, so native `cargo test` cannot call GIL-
acquiring functions; real coverage of this logic lives in `demo.py`.

`demo.py`, run via `maturin develop --release`: `line_count()` on a
3-line hard-wrapped text is exactly 3; `move_caret_up`/`down` are real
no-ops at the first/last line; a straight column-0 traversal down and
back up an exact, hand-predicted byte-offset path (`0 -> 4 -> 10 -> 4 ->
0`, font-metric-independent since `x=0.0` always means "this line's own
first byte"); a real sticky-column move from the shortest line's own end
lands provably within the correct target line's own byte range;
`selection_rects()` returns exactly 2 rects for a selection spanning 2
real lines (with consistent `line_height` and positive width) and
exactly 1 for a single-line selection; `cut`/`paste` round-trip a
selection spanning a real line break byte-exact; and a real, finite
`wrap_width` drives both `line_count()` and `move_caret_down()`
correctly, reaching the last of N real wrapped lines in exactly N-1
steps -- proving Step 15.1's rendering primitive and this step's editing
layer share one real, consistent layout.

**Real, disclosed scope limits**: no Shift+arrow selection-extension via
vertical movement (`move_caret_up`/`down` always clear any active
selection, matching `set_caret`'s own convention) -- a real caller
wanting to extend a selection with the keyboard combines these with
`set_selection` itself. All the real, disclosed v1 scope limits from
Step 15.1's own `wrap_lines` (LTR-only, whitespace-boundary word-wrap,
no hyphenation, left-aligned only) apply here unchanged, since this step
builds directly on that same layout.
