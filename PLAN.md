# PLAN — Branch `0.3.5.2`: Milestones 105-107

*(Replaces the M104 plan — M104 is complete and released as `v0.3.5.1`. Every step is in `BUILD_TRACKER.md`.)*

Three issues Tesserae filed, one patch release:

1. **M105, issue #16 — done.** `Window.undock_panel(panel)`; the take-out
   path keeps the shown panel when an earlier one leaves; a freed panel
   leaves its zone instead of panicking `add_child`.
2. **M106, issue #18 — done.** `window.get("dark")`; on Linux, where
   `winit` reports nothing, the XDG settings portal answers it and its
   changes fire `color_scheme`.
3. **M107, issue #19 — Phase 1 done.** Published to PyPI as
   `tesserae-engine` (`tre` is taken there; the import name stays `tre`),
   by a trusted-publishing `pypi` job in `wheels.yml`. Phase 2, the first
   upload, needs the user: register the pending publisher on pypi.org
   (owner `mindderivative`, repo `tre`, workflow `wheels.yml`, environment
   `pypi`), then release -- the `v0.3.5.2` tag uploads.

## Status

**Released (2026-09-28).** `v0.3.5.2` is out and on PyPI as `tesserae-engine` with
its 22 wheels; Tesserae told. PyPI rejected the sdist (no `LICENSE` packed) --
fixed on local branch `0.3.5.3`, to ship with the next release.
