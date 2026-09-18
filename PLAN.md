# Plan: M27 Phase 1 — Shell & Navigation Scaffold

Corresponds to `BUILD_TRACKER.md` M27 Phase 1.

## Investigation (already done, confirmed via direct read)

- `Window.build_shell(menu_bar=...)` returns `content`: a real node
  with `Style { flex_grow: 1.0, ..Default::default() }`. Confirmed
  directly in taffy 0.14.0's own source (`~/.cargo/registry/.../
  taffy-0.14.0/src/style/mod.rs`, `Cargo.toml`'s own `default =
  [..., "flexbox", ...]` feature list) that `Style::default()`'s
  `display` is `Display::Flex` and `FlexDirection::default()` is
  `Row` — so `content` is already a real `Flex Row` container with no
  extra style config needed: a nav rail added first and a screen area
  added second lay out side by side automatically.
- `Node.remove()` truly deletes a node and its subtree (`Tree::
  remove`), not a soft detach — re-showing a previously-shown screen
  means rebuilding it fresh, the identical real pattern `examples/
  navigation.py` already proves end to end (remove the old screen,
  build and attach a new one). This phase reuses that pattern verbatim
  for nav-driven screen switching.
- `Node.set_on_click` already adds `Action::Click` to a node's own
  `access.actions`, making it Tab-reachable with no extra call needed
  (confirmed in `node.rs`'s own doc comment) — every nav button is
  keyboard-operable for free the moment it gets a click handler.

## What will change

New `demo/showcase.py` (a fresh top-level location, not `examples/` —
`examples/` is explicitly "one real mechanism" per script; this is the
opposite, a consolidated multi-screen app, per the milestone's own
stated purpose):

- `Window` sized 720×480, `Window.build_shell(menu_bar=...)` for the
  chrome.
- A persistent left nav rail (`NAV_WIDTH` px) attached first to
  `content`, a screen area attached second — real side-by-side layout
  via `content`'s own default `Flex Row`, no extra style needed.
- One real nav button per showcased screen (`add_rect` + `set_on_click`
  + `enable_interaction`), highlighted (background swap) when its own
  screen is active.
- `show_screen(key)`: removes the currently-shown screen's subtree and
  builds+attaches the requested one fresh — the real, proven `examples/
  navigation.py` pattern, not a new mechanism.
- Phase 1's own two screens are placeholders (a labeled colored card
  each) — Phases 2–4 replace them with real content, added to the same
  file's own `SCREENS` registry.
- Real, headless-CI-safe proof: a dispatched `window.click()` on the
  second nav button actually switches the active screen (checked via
  the script's own tracked state, not just "didn't crash"); a real
  Tab-order check presses Tab once per nav button and confirms each
  becomes focused in the same order they were attached.

## Testing

- `cargo test --workspace --release` / clippy / fmt (no engine code
  changes expected this phase — a pure Python composition; run anyway
  to confirm no accidental regression)
- `maturin develop --release` + `python demo/showcase.py` directly,
  with the real display
- Full `pytest tests/` + all `examples/*.py` regression pass
