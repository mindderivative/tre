# PLAN — Branch `0.4.4`: Milestones 21–23

*(Replaces the M14–M20 plan — `v0.4.3` is released. Every step is in
`BUILD_TRACKER.md`.)*

User (2026-09-30): "scope 1 and 2 as 0.4.4".

## M21 — Scroll chaining — done

A view keeps a wheel along its axis even when it can't move (content fits,
or at that end); browsers pass it to the view outside. In core's wheel walk,
pass on when the view can't move in the wheel's direction; a view that can
move takes the whole wheel, clamped.
1. Wheel: core tests + pytest (fits, each end, mid-way, virtual list, nested).
2. Keys (the user agreed): `scroll_view_for_key` picks the nearest view
   that can move in the key's direction.
3. Guide, ARCHITECTURE §11.7a, 0.4.4 migration section.

## M22 — A Windows CI cache that saves

Windows cache 3.3 GB; its save hit the 5-minute limit on 0.4.3's PR. Restore
~2 min, cached build ~1 min. Repo caches ~8.8 GB of GitHub's 10 GB.
1. Measure: no cache, registry only, full cache; upload time.
2. Apply the cheapest shape that saves inside a limit; stay under 10 GB.

## M23 — Release `0.4.4`

PR, CI, merge, tag, PyPI (the user approves), Tesserae. Check the PR's
closing references after opening it (#29's "Closes #27" didn't link).

Each step: the full chain, docs, tracker, a local commit.
