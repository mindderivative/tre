# PLAN — Branch `0.4.3`: Milestones 14–20

*(Replaces the M11–M13 plan — `v0.4.2` is released. Every step is in
`BUILD_TRACKER.md`.)*

User (2026-09-30): "yes, push it and scope both as 0.4.3" -- Tesserae's two
observations on 0.4.2.

## M14 — Keyboard scrolling leaves shortcuts alone — done

With Ctrl, Alt, or Meta held, `keyboard_scroll` (engine-py `dispatch.rs`)
doesn't scroll; Shift still does, as in a browser. `listeners::modifiers()`
has all four; core's `KeyPressed` only Shift.
1. The rule and pytest cases (each modifier, plain, Shift).
2. Guide, events guide, 0.4.3 section in `docs/migrating-0.4.md`.

## M15 — `scroll_offset` clamped when set — done

Today `set(scroll_offset=5000)` reads back 5000 until layout clamps it (to
900 in a 1000/100 view), firing `scroll` twice: (0, 5000) then (5000, 900).
And `create(..., scroll_offset=500)` never becomes the event's baseline.
1. `Node.set`/`animate` of `scroll_offset`: layout, then clamp; one event;
   an animation eases to the real end. `create` unchanged, so an offset
   given before content is attached survives adding it.
2. The initial offset is `reported`'s starting value.
3. Property reference, guide, migration section.

## M16 — Grid: a bare number as a one-track list (issue #27) — done

`track_list` takes an int or float (not a bool) as `[n]`.

User (2026-09-30): "Scope 1, 2, and 3 for 0.4.3. Then we will look at
releasing if there is nothing else after those."

## M17 — Shift+wheel scrolls horizontal views

A horizontal view scrolls only on a wheel's x part; plain and Shift+wheel
both leave it at 0. In engine-py (which tracks modifiers): Shift held and
x == 0 -> deliver y as x to core dispatch. macOS's own conversion (x != 0)
untouched; terminal scrollback keeps y; `wheel` listeners see it as
delivered.
1. Mapping + pytest (horizontal view, vertical view, x already set, terminal).
2. Guide, events reference, 0.4.3 migration section.

## M18 — Unit tests for the scroll core

`engine-core` tests for `scroll_view_for_key`, `scroll_by_key`,
`scroll_into_view`, `take_scroll_changes`, `max_scroll` (cargo 377 since M12).

## M19 — Stub drift checked in CI

CI runs neither `mypy --strict` nor stubtest.
1. Stub: `Window.__new__`, `@final`, `Node.__eq__(self, other, /)`,
   `__all__`; allowlist `Color`; stubtest clean.
2. CI Linux job: install mypy; `mypy --strict python/tre`; stubtest.

## M20 — Release `0.4.3`

After M17–M19, if nothing else joins. PR (its Cargo.lock change also runs
PR #26's timed cache save for the first time), CI, merge, tag, PyPI (the
user approves), Tesserae.

PR, CI, merge, tag, PyPI (the user approves), Tesserae. Fold in PR #26 (the
CI cache timeout) if it has merged by then.

Each step: the full chain, docs, tracker, a local commit.
