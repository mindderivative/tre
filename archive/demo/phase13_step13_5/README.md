# Demo: Phase 13 Step 13.5 -- Editable Text (`tre.EditableText`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase13_step13_5
../../.venv/bin/python demo.py
```

**What this proves.** The still-pending "plan out editable text" item
from the project owner's own earlier directive -- now a real,
single-line text-editing implementation, not just a plan.

- **Real caret hit-testing** (`crates/tre-text/src/caret.rs`, new this
  step): `caret_positions`/`hit_test` use the exact same pen-advance
  formula (`scale = px_size / units_per_em`, `pen.x += glyph.x_advance *
  scale`) `tre_engine::text::flatten_text` itself renders with -- read
  directly from that function's own source before writing this, not
  assumed -- so a caret computed here lands exactly where the matching
  glyph actually renders. The pure math has exact, hand-computed unit
  tests in Rust using synthetic glyph data; this demo proves the other
  half, real integration against a real system font, including a
  monotonicity check (`hit_test` results only increase as the click
  x-position increases, across a real shaped string).
- **Real edit operations**: `insert`/`delete_selection`/
  `delete_backward`/`set_caret`/`set_selection`, all maintaining a real
  UTF-8 char-boundary invariant on the caret -- proven with `"café"`
  (`"é"` is a real 2-byte character): `delete_backward` removes exactly
  one real character, not one byte, and `set_caret` raises `ValueError`
  for an offset that would split it.
- **Real IME wiring**: `handle_ime` dispatches the same
  `ImeEnabled`/`ImePreedit`/`ImeCommit`/`ImeDisabled` events already
  forwarded end to end since Step 12.7 -- `ImePreedit` updates a
  separate composing string without touching the real committed text;
  `ImeCommit` splices its own text in at the caret and clears it. A
  small, genuinely useful addition made to enable this: `tre.WindowId`
  gained a real `#[new]` (`tre.WindowId(0)`) so a caller (this demo, or
  any future test/automation) can synthesize a real `InputEvent` without
  a real window -- previously only a live `WindowedRenderer` could
  produce one.
- **Real render integration**: `to_text()` builds a real `tre.Text`
  (with any active `preedit` spliced in for real visual composing
  feedback) and this demo inserts it into a registry and actually
  renders it -- following the same real cache-miss/background-atlas
  retry pattern `demo/phase12_step12_3/demo.py` established (the first
  several `render()` calls are legitimate cache misses while
  `TextAtlas`'s own background thread rasterizes the glyphs).

**Full workspace verification performed:**

- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets
  -- -D warnings` / `cargo build --workspace --all-targets` / `cargo
  test --workspace` -- all clean in debug (`tre-text` 19 -> 22 tests:
  `caret_positions`'s exact per-glyph placement and px_size/units_per_em
  scaling, `hit_test`'s nearest-stop behavior including before-start and
  past-end clamping).
- In `--release`, the same 5 pre-existing, unrelated `tre-engine`/
  `tre-memory` test failures already disclosed in every prior step's
  README -- still flagged as separate follow-up work, not fixed here.
- `demo.py` run via `maturin develop --release`: exits 0, every
  assertion (including the real font/IME/render integration checks)
  passes.

**A real constraint found and worked around while building this
step**: `tre-python` is built with PyO3's `extension-module` feature,
which does not link against `libpython` (a real extension module gets
those symbols from the host interpreter that loads it at runtime) --
this means a native `cargo test` binary for `tre-python` cannot call any
GIL-acquiring function (`Python::attach`, `Py::new`, etc.) without a
real embedded interpreter to satisfy those symbols at link time. `Rust`
tests exercising `PyEditableText` that needed to construct real PyO3
objects were therefore dropped in favor of this demo's own Python-level
coverage -- the correct, real fix given the crate's build mode, not a
workaround.

**Real, disclosed scope limits**: single-line only, matching
`tre_engine::Text`'s own single-line-only scope as of this step. Multi-
line/word-wrap rendering and multi-line editing both shipped later, in
Phase 15 Steps 15.1/15.2 -- this corrects an earlier, INACCURATE claim
here that multi-line label rendering already existed at this step;
direct reading of `flatten_text`'s own source when Phase 15 was planned
confirmed it never had (a single straight pen line, no `\n` handling at
all). Caret/selection byte offsets are also LTR-only-correct, matching
`tre_text::caret_positions`'s own disclosed limitation (`ShapedRun`s are
in **visual**, not logical, order for bidi/RTL text).
