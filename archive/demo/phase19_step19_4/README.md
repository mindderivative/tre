# Demo: Phase 19 -- Focus / Keyboard Navigation

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase19_step19_4
../../.venv/bin/python demo.py
```

**What this proves.** GUI Readiness's gap table marked "Focus /
keyboard nav / tab order" fully missing -- the one remaining gap
that's unambiguously tre's own responsibility (layout/theming, also
missing, is deliberately scoped to pySilver per DESIGN.md). DESIGN.md's
own architecture diagram has a box, never implemented, labeled "Spatial
Hit Testing & Scene Tree Node Focus Manager," and `InputEvent` had no
focus-change variant of any kind. This phase closes both halves of
that gap, kept deliberately separate:

## Step 19.1: real OS-level window focus

`InputEvent.WindowFocused { window, focused }` mirrors winit's own
`WindowEvent::Focused(bool)` -- previously silently dropped by
`crates/tre-platform/src/winit_backend.rs`'s catch-all match arm. This
is a different concept from in-app *widget* focus: it fires even for
an app with no concept of a focused widget at all (alt-tab, clicking
another app).

## Step 19.2: `FocusManager` -- real in-app widget focus and Tab order

`tre_engine::focus::FocusManager` is pure, persistent in-memory state
(not per-frame-cleared, not `unsendable` -- no OS handle, unlike
`Clipboard`/`TrayIcon`/`A11yBridge`). `RenderingCanvas::tag_focusable`
mirrors `tag_accessibility_node`'s own real, transform-correct
world-space bounding-box logic (the corner-transform math was factored
out into a shared `transform_bounds` helper so neither method
duplicates it), reusing the same caller-assigned `AccessibilityNodeId`
identity rather than inventing a second, parallel node-id system for
the same widget tree.

**Tab order follows the real HTML `tabindex` convention** (WHATWG HTML
Standard Section 6.6.7) rather than an invented rule: a positive
`tab_index` is visited first (ascending), `None`/`0` falls back to
input/geometry order, and a **negative** `tab_index` is excluded from
sequential Tab navigation entirely -- HTML's own "focusable, but skip
me" escape hatch -- while remaining directly targetable via
`set_focus` (programmatic/click-to-focus). Citing an existing,
external standard beats inventing bespoke tie-break semantics nobody
outside this codebase would recognize.

**Deliberately not tracked here**: modifier-key (Shift/Ctrl/Alt) state
or raw `key_code` parsing for Tab. Matching the established boundary
(`InputEvent::KeyboardKey`'s own doc comment, and Phase 15's
`EditableText::move_caret_left(extend: bool)` precedent), the *caller*
recognizes Tab/Shift+Tab itself and calls `focus_next()`/
`focus_previous()` once it has decided a tab navigation should happen
-- this demo simulates that recognition directly rather than needing a
real, injected Tab keypress.

## Step 19.3: `tre.FocusManager`/`Canvas.tag_focusable`, wired into `tre-python`

Thin PyO3 wrappers (`crates/tre-python/src/focus.rs`, mirroring
`a11y.rs`'s own `register()` pattern): `tre.FocusableNode` (a plain
data mirror), `tre.FocusManager` (**not** `unsendable` -- pure logic),
and `Canvas.tag_focusable(node_id, x, y, width, height, tab_index=None)`
/ `Canvas.focusable_nodes()`.

## `demo.py`, run via `maturin develop --release`

Tags three real rects in one frame -- `tab_index=None` (geometry
fallback), `tab_index=1` (explicit priority), `tab_index=-1`
(excluded-but-settable) -- reads `focusable_nodes()` **before**
`render_canvas()` consumes the canvas (the same ordering rule Phase 18
Step 18.3 already established for `accessibility_nodes()`), then:

- Asserts the exact `focus_next`/`focus_previous` id sequences,
  including wraparound, with the `tab_index=-1` node never appearing.
- Calls `set_focus` on that same excluded node directly and asserts it
  took effect -- proving "excluded from Tab order" is not "not
  focusable."
- Polls real input events and asserts a real `WindowFocused` is
  observed on this real desktop.

**Real, disclosed scope limits:**

- `RenderingCanvas.focusable_nodes` is deliberately **not** threaded
  through `flatten()`/`FrameArena`/`SubCanvas` merging the way
  `accessibility_nodes` is -- that plumbing exists for multi-threaded
  canvas recording, and no real caller records focusable nodes off the
  main thread today. `focusable_nodes()` is read directly off the root
  `RenderingCanvas`.
- This sandbox has no input-injection or window-management tool (no
  `xdotool`/`wmctrl` -- confirmed absent) to synthesize another app
  stealing focus, so only the window-**gains**-focus half of
  `WindowFocused` is asserted unattended here. Fully automating the
  window-**loses**-focus half needs a second real window/process, or a
  human alt-tabbing away and back during a manual run of this same
  demo -- matching Phase 17's own "a human still needs to look at some
  real UI results" disclosure discipline.
- `tre-engine` tracks no modifier-key state and parses no raw key
  codes for Tab -- a real UI framework recognizes Tab/Shift+Tab (and
  tracks Shift) itself, matching the established boundary this project
  has held since `InputEvent::KeyboardKey` and Phase 15's
  `EditableText`.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug. `--release` clean apart
from the same 5 pre-existing, already-disclosed `debug_assert!`
failures. `demo/phase18_step18_3/demo.py` re-run unchanged as a
regression check on the `transform_bounds` refactor.
