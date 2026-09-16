# Log: M7 Phase 3 — Dynamic Color, Wired for Real (§7.1, completing §7.3)

Corresponds to `BUILD_TRACKER.md` M7 Phase 3. Three steps: a `Window`-
level theme concept in `engine-py`; ripple/hover's hardcoded black tint
becoming the real MD3 "on-surface" scheme role; real live theme
switching via `winit`'s `ThemeChanged`.

## Investigation before writing code

- `engine-md3::color::ColorScheme`/`DynamicTheme::from_seed` were
  already real and proven (M3 Phase 5 step 11) — confirmed by reading
  `crates/engine-md3/src/color.rs` directly, including its own test
  suite matching `material-colors`' native output.
- `engine-core::InteractionState` had no color field; `Tree::
  interaction_mut` is the single lazy-creation choke point. `engine-
  render`'s ripple/hover paint arm hardcoded black twice — its own doc
  comment (left during M7 Phase 2) already forward-referenced this exact
  phase.
- `engine-render` cannot depend on `engine-md3` (§4, re-confirmed in
  Phase 2) — any real color has to be resolved by `engine-py` and pushed
  down as plain data, matching Design Principle 6 exactly the way
  `PaintProperties.background` already works.
- `Node`'s single interaction opt-in is `enable_interaction()` — the
  exact spot a real theme color needs to land at opt-in time.
- `Node` already shares `handlers`/`context_menus` with its owning
  `PyWindow` via `Rc<RefCell<...>>` clones threaded through every
  construction site (`window.rs` ×4, `view.rs` ×3) — a new shared theme
  handle follows the identical pattern.
- `View` (YAML-driven) has its own separate, pre-existing color-
  resolution path (`engine_spec::build::resolve_color`) — left
  untouched; `BUILD_TRACKER.md`'s own scope text says "Window/App-level"
  specifically, so `View`'s 3 construction sites get a fresh, private,
  never-`Window`-linked theme handle instead, keeping its behavior
  byte-for-byte unchanged.
- `engine_core::InputEvent` already has a real "plumbing only, `Tree::
  dispatch` is a true no-op" precedent — `Scroll` (M4 Phase 8, confirmed
  at `tree.rs`). `ThemeChanged` follows the identical shape.
- `engine-platform`'s `on_input` closure already translates several
  `WindowEvent`s into `InputEvent` right before a final `_ => {}`
  catch-all — the exact, already-real mechanism the milestone names.
  Verified directly against the pinned `winit = "0.30.13"` source:
  `WindowEvent::ThemeChanged(Theme)` is real, `Theme` is `{ Light,
  Dark }`. Its own doc comment: unsupported on iOS/Android/X11/Wayland/
  Orbital — meaning live OS theme switching will not fire on this
  machine's own Linux session, a real platform limitation of this
  event, not a bug in the wiring. `Window.set_theme()` itself works
  identically on every platform regardless.
- `engine-py::App::run`'s `WindowSetup`/`WindowRuntime` already extract
  `tree`/`handlers`/`context_menus`/`dock` from each `PyWindow` once, up
  front, into the `winit` closures — a shared theme handle threads
  through identically.

## What happened

`engine-core`: `InteractionState.tint: Color` (defaults to real black —
byte-for-byte the old hardcoded value); `Tree::set_all_interaction_tints`
(updates every already-opted-in node, never lazily creates one);
`InputEvent::ThemeChanged { dark: bool }` plus one no-op `Tree::dispatch`
arm, mirroring `Scroll`.

`engine-render`: `paint_node`'s two hardcoded
`Color::from_rgba8(0, 0, 0, 255)` ripple/hover fills now read
`interaction.tint`.

`engine-platform`: new `translate_theme(winit::window::Theme) -> bool`
free function (matching `translate_pointer_button`/`translate_key`'s own
shape) plus one new `WindowEvent::ThemeChanged` translation arm.

`engine-py`: new `window::ThemeState { theme: Option<DynamicTheme>, dark
}` / `SharedTheme = Rc<RefCell<ThemeState>>`, with `on_surface()`
(returns real black when no theme is set) and `set_dark()`. `PyWindow`
gains `theme: SharedTheme` and `Window.set_theme(seed, dark=False)`,
which builds `DynamicTheme::from_seed`, stores the mode, and immediately
calls `Tree::set_all_interaction_tints` for nodes already opted in.
`Node` gains `theme: SharedTheme`; `enable_interaction()` now applies
the current on-surface tint immediately. All `Node` construction sites
updated (`window.rs` ×4 share the real `Window`'s theme; `view.rs` ×3
each get a fresh, private, always-black instance). `App::run`'s
`WindowSetup`/`WindowRuntime` gain `theme: SharedTheme`; the `on_input`
closure gains a `ThemeChanged` arm that updates the mode, recomputes
`on_surface()`, and pushes it via `set_all_interaction_tints` — the real
live-switch path.

New tests: `InteractionState` defaults to black (no behavior change);
`Tree::set_all_interaction_tints` updates only opted-in nodes, leaves
`None` nodes untouched; `Tree::dispatch(ThemeChanged)` is a true no-op;
`engine-platform::translate_theme` maps both real `winit::window::Theme`
variants; a new `engine-render` pixel test proves a real, non-black
tint actually paints a real hue (red-dominant overlay), not just a
darker gray, the way a hardcoded black tint could only ever produce.
New `examples/theme.py`: `Window.set_theme` + `Node.enable_interaction`
+ `click()`, proving the full call chain compiles and runs end-to-end.

Full `cargo test --workspace --release` clean (`engine-core` gains 3
tests, `engine-render` gains 1, `engine-platform` gains 1 — every prior
test still passes unmodified), `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo fmt --check` all clean. `maturin develop
--release` + full `pytest tests/` (78 passed, 1 skipped, unchanged — no
existing `engine-py` Python-facing behavior touched) and all twelve
examples (eleven existing + new `theme.py`) confirmed clean.
