# Log: M27 Phase 4 — Data & Layout Screen

`demo/showcase.py`'s "data" screen: a genuinely large virtualized list
(5,000 rows, real paging), a real minimal docking layout (a headless
drag-and-drop, the same `start_panel_drag`/`drop_panel_at` sequence
`tests/test_docking.py` already establishes), and a declarative YAML
`View` panel — the first place both authoring paths genuinely compose
in one real running app.

**Real, honest reframing of the milestone's own "embedded alongside"
language:** `View` has no rendering/render-loop concept of its own
(confirmed directly in `view.rs`'s own module doc comment) — nothing a
`View` builds is ever visually nested inside a `Window`. "Compose"
here means real, meaningful *data* flow instead: a click dispatched
through the `View`'s own real handler mechanism updates a `Signal`,
read back and reflected on an ordinary imperative `Window` label —
proven, not asserted. Also adopts M26's real stylesheet/token support:
the panel's own stylesheet resolves `background: primary` against a
real seed color and a real `id: bump_button` cascade rule
(`corner_radius: 12`) that must win over its own `kind: Rect` rule
(`corner_radius: 4`) — verified via the same no-pixel-needed
`Node.get("corner_radius")` check M26's own example established.

**A cluster of real bugs, every one found only by actually running the
screen, not assumed:**

1. **A stray `def bump(self, event):` in two docs pages *and* the
   actually-shipped `python/tre/__init__.py` docstring itself** — the
   exact zero-argument-handler bug Phase 1 already found and fixed
   three other instances of, missed here because that earlier grep
   sweep's pattern (`(event):`) doesn't match `(self, event):` (a
   space, not a paren, precedes `event`). Reproduced standalone before
   fixing; fixed all three real occurrences (the shipped docstring is
   the more important of the two doc copies, being what `help
   (ViewModel)` shows a real user). A fourth occurrence, in `tests/
   test_view_binding.py`, is not a bug — that test only exercises
   `_attach`'s own attach-time *validation* (existence/callability),
   never a real dispatched click, so the wrong arity never surfaces;
   left untouched.
2. **No event bubbling in this engine's hit-testing, a real,
   previously-unexercised fact:** a decorative `Text` label added as a
   button's own child, centered over the button (exactly where
   `Window.click()` always targets), silently absorbed every click —
   the label itself has no `Click` in its own `access.actions`, and
   nothing walks up to the parent's own handler. Broke both the nav
   rail's own new text labels and, independently, three of this
   screen's own labeled trigger buttons (fixed the same way: labels
   moved clear of each button's own geometric center, the identical
   "label above, not overlapping" pattern already safely used
   elsewhere in this file).
3. **`Window.add_virtual_list` takes no `x`/`y` at all**, unlike every
   other real `add_*` method (a real `TypeError`, not assumed) — fixed
   by wrapping it in an ordinary positioned container instead.
4. **A second real, genuine API gap**, the same class M27 Phase 2's
   `add_text` closed: nothing could update (or read back) a plain
   `Text` label's own content after creation — `Node.set_text`/
   `get_text` only ever handled `TextField`. Extended both with a
   second, simpler match arm for `NodeKind::Text` (no cursor, no
   `Change` — a label isn't interactive), the real, concrete need this
   screen's own dynamic "Declarative counter: N" readout surfaced.

Full `cargo test --workspace --release` (all pre-existing suites
unmodified and passing)/clippy `-D warnings`/fmt clean. `maturin
develop --release` + `pytest tests/` (187 passed, unchanged, 1
pre-existing skip), all 33 pre-existing examples, and the updated demo
confirmed clean with the real display. Updated `docs/api/python/
node.md` (`set_text`/`get_text`) and `docs/guide/imperative-api.md`
(`add_text`'s own missing table row). `mkdocs build --strict` clean.

M27 Phase 4 — Data & Layout Screen is now complete. M27 continues with
Phase 5 (accessibility pass + polish, closing the milestone).
