# PLAN — Branch `0.4.3`: Milestones 14–17

*(Replaces the M11–M13 plan — `v0.4.2` is released. Every step is in
`BUILD_TRACKER.md`.)*

User (2026-09-30): "yes, push it and scope both as 0.4.3" -- Tesserae's two
observations on 0.4.2.

## M14 — Keyboard scrolling leaves shortcuts alone

With Ctrl, Alt, or Meta held, `keyboard_scroll` (engine-py `dispatch.rs`)
doesn't scroll; Shift still does, as in a browser. `listeners::modifiers()`
has all four; core's `KeyPressed` only Shift.
1. The rule and pytest cases (each modifier, plain, Shift).
2. Guide, events guide, 0.4.3 section in `docs/migrating-0.4.md`.

## M15 — `scroll_offset` clamped when set

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

## M17 — Release `0.4.3`

Held until more fixes join 0.4.3 (user, 2026-09-30).

PR, CI, merge, tag, PyPI (the user approves), Tesserae. Fold in PR #26 (the
CI cache timeout) if it has merged by then.

Each step: the full chain, docs, tracker, a local commit.
