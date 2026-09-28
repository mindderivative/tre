# LOG — Branch `0.3.5.2`: Milestones 105-107

- Tesserae filed issue #16 (its M52 hot reload can't drop a panel). The user:
  "Take a look at main branch issue #16 about undocking panels", then "yes"
  to M105 on a new `0.3.5.2` branch off `main`.
- Worked in a separate worktree (`../tre-0.3.5.2`, its own `.venv`), so the
  `0.4.0` checkout and its build were left alone.
- M105 committed (`db68531`).
- The user: "Check for more issues, I believe you have 2 more" -- #18 and
  #19. Answers: #18 "Read + live events", #19 name `tesserae-engine`, both
  on `0.3.5.2`.
- `winit` 0.30 reports the appearance on macOS and Windows only; Linux uses
  the XDG portal through `zbus` (already built for AccessKit). Checked end
  to end against a stand-in portal on a private bus.

## Status

**M106 complete (2026-09-28).** M107 next.
