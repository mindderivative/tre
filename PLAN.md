# Plan: M7 Phase 3 — Dynamic Color, Wired for Real (§7.1, completing §7.3)

Corresponds to `BUILD_TRACKER.md` M7 Phase 3, the milestone's own
scoping (three steps): (1) a `Window`-level theme concept in
`engine-py`, generated via the real `DynamicTheme::from_seed`; (2)
ripple/hover's hardcoded black tint becomes the real MD3 "on-surface"
scheme role; (3) real live theme switching — `winit`'s `ThemeChanged`
forwarded through the real `InputEvent`/dispatch path M4 already built.

## Investigation (facts verified before writing code)

- `engine-md3::color::ColorScheme` is a real, already-proven 49-field
  struct with a `role(&self, name: &str) -> Option<Color>` resolver and
  `DynamicTheme { light, dark }` with `DynamicTheme::from_seed(seed:
  Color) -> Self` — confirmed by reading `crates/engine-md3/src/
  color.rs` directly, including its own test suite proving it matches
  `material-colors`' native output.
- `engine-core::InteractionState` (`crates/engine-core/src/
  interaction.rs`) currently has `ripples`, `hover_opacity`,
  `focus_ring` — no color field. `Tree::interaction_mut` is the single
  lazy-creation choke point (`get_or_insert_with(InteractionState::
  new)`), confirmed via grep — most nodes never get one until a real
  opt-in call.
- `engine-render/src/lib.rs`'s ripple/hover paint arm (`paint_node`,
  ~line 405-439) hardcodes `Color::from_rgba8(0, 0, 0, 255)` twice (hover
  fill, ripple fill) — its own doc comment already forward-references
  this exact phase: "dynamic color... isn't wired into `paint_node`
  anywhere yet... that's a separate, larger, pre-existing gap, not
  solved here as a side effect [of M7 Phase 2]." `engine-render` cannot
  depend on `engine-md3` (§4's dependency diagram, re-confirmed in Phase
  2) — so it must receive an already-resolved plain `Color`, not an MD3
  concept.
- `engine-py::Node`'s single opt-in method is `enable_interaction()`
  (`node.rs`), a thin call into `Tree::interaction_mut` — the exact spot
  a real theme color needs to land at opt-in time.
- `Node` currently shares `handlers`/`context_menus` with its owning
  `PyWindow` via `Rc<RefCell<...>>` clones threaded through every
  construction site (`window.rs` ×4, `view.rs` ×3, confirmed via grep).
  A new shared theme handle follows the identical, already-established
  pattern.
- `View` (`view.rs`, YAML-driven) has its own separate, pre-existing
  color-resolution path (`engine_spec::build::resolve_color`, tries a
  scheme role name then falls back to a literal color) — untouched by
  this phase. `BUILD_TRACKER.md`'s own Phase 3 scope text says
  "Window/App-level theme concept" specifically — `View`'s nodes get a
  fresh, private (non-shared) theme handle at each of its 3 construction
  sites, so nothing about `View` changes behavior; only `Window`-created
  nodes ever see a real theme.
- `engine_core::InputEvent` (`input.rs`) already has a real precedent
  for "plumbing only, `Tree::dispatch` is a true no-op for it" —
  `Scroll` (M4 Phase 8, confirmed at `tree.rs:1174`,
  `InputEvent::Scroll { .. } => DispatchOutcome::None`). `ThemeChanged`
  follows the identical shape: a `Tree`-level no-op, meaningful only to
  `engine-py`'s own dispatch closure.
- `engine-platform`'s `on_input` closure (`lib.rs`) already translates
  `WindowEvent::CursorMoved`/`MouseInput`/`KeyboardInput`/`MouseWheel`
  into `InputEvent` variants, right before a final `_ => {}` catch-all
  (confirmed at `lib.rs:470-519`) — the exact, already-real mechanism
  the milestone's own scope text names.
- Verified directly against the pinned `winit = "0.30.13"` source
  (`~/.cargo/git/checkouts/winit-.../src/event.rs`): `WindowEvent::
  ThemeChanged(Theme)` is real, `Theme` is `{ Light, Dark }`. Its own
  doc comment: "Platform-specific: iOS / Android / X11 / Wayland /
  Orbital: Unsupported." This means live OS theme switching will not
  fire on this machine's own Linux/X11 or Wayland session — a real,
  worth-stating platform limitation, not a bug in this wiring. `Window.
  set_theme()` itself (steps 1/2) works identically on every platform;
  only the *automatic* OS-driven switch (step 3) is unsupported here.
- `engine-py::App::run`'s `WindowSetup`/`WindowRuntime` structs
  (`app.rs`) already extract `tree`/`handlers`/`context_menus`/`dock`
  from each `PyWindow` once, up front, into plain `Rc<RefCell<...>>`
  clones the `winit` closures capture — a shared theme handle threads
  through identically, extracted from `window.theme.clone()`.

## Design

- New `engine-core::interaction::InteractionState.tint: peniko::Color`
  field, defaulting to real black (`Color::from_rgba8(0, 0, 0, 255)`) in
  `::new()` — byte-for-byte the current hardcoded behavior when no
  theme is ever set, so this is additive, not a behavior change by
  itself.
- New `Tree::set_all_interaction_tints(&mut self, tint: Color)` —
  iterates every node's `Option<InteractionState>`, updates `tint` on
  each `Some`. The mechanism live theme switching (step 3) and
  `Window.set_theme` (step 1) both reuse to update *already-opted-in*
  nodes retroactively.
- New `InputEvent::ThemeChanged { dark: bool }` variant; `Tree::dispatch`
  gets one new no-op arm (`DispatchOutcome::None`), matching `Scroll`'s
  own precedent exactly.
- `engine-render::paint_node`'s two hardcoded
  `Color::from_rgba8(0, 0, 0, 255)` ripple/hover fills become
  `interaction.tint` — no other change to the paint code's shape.
- `engine-platform`: one new `WindowEvent::ThemeChanged(theme) =>` arm
  in the existing translation match, calling `on_input(window_id,
  InputEvent::ThemeChanged { dark: theme == winit::window::Theme::
  Dark })`.
- New `engine-py::window::ThemeState { theme: Option<DynamicTheme>,
  dark: bool }` plus `on_surface(&self) -> Color` (returns real black
  when `theme` is `None`, matching current default; otherwise the
  active scheme's real `on_surface` field) and `type SharedTheme =
  Rc<RefCell<ThemeState>>`.
- `PyWindow` gains `theme: SharedTheme`; new method `set_theme(seed:
  (u8,u8,u8,u8), dark: bool = false)` builds `DynamicTheme::from_seed`,
  stores it + the given mode, then calls `Tree::
  set_all_interaction_tints` with the freshly resolved `on_surface` —
  so nodes that already opted into interaction before the theme was set
  pick it up immediately, matching what a real live switch must also do.
- `Node` gains `theme: SharedTheme`; `enable_interaction` reads
  `theme.borrow().on_surface()` and sets it on the just-opted-in
  `InteractionState` immediately, so opting in *after* a theme was set
  doesn't wait for another theme-change event to pick up the real color.
- All `Node { ... }` construction sites (`window.rs` ×4) pass
  `self.theme.clone()`; `view.rs` ×3 pass a fresh, private `Rc::new(
  RefCell::new(ThemeState::default()))` each — `View` keeps its
  existing, separate, untouched color story.
- `App::run`'s `WindowSetup`/`WindowRuntime` gain `theme: SharedTheme`,
  extracted from `window.theme.clone()` the same way `dock`/
  `context_menus` already are. The `on_input` closure gets one new
  match arm on the raw `event` (alongside the existing dock-drag match):
  `InputEvent::ThemeChanged { dark } =>` updates `runtime.theme`'s
  `dark` flag, recomputes `on_surface()`, and calls `Tree::
  set_all_interaction_tints` — the real live-switch path.

## Verification plan

- `cargo test --workspace --release`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --check`.
- New Rust tests: `InteractionState` defaults to black tint (no
  behavior change); `Tree::set_all_interaction_tints` updates only
  opted-in nodes, leaves not-opted-in nodes' `interaction` as `None`;
  `Tree::dispatch(ThemeChanged)` returns `DispatchOutcome::None` and
  touches nothing else; a pixel-level `engine-render` test proving a
  non-black tint actually paints (mirroring `elevation_shadow.rs`'s own
  render-to-texture-then-readback discipline).
- `engine-platform`: a unit test on the translation function/match arm
  proving `Theme::Dark`/`Theme::Light` map to `dark: true`/`false`
  (matching this crate's own existing `translate_pointer_button`/
  `translate_key` test-coverage precedent), without needing a real
  window.
- `maturin develop --release` + `python -m pytest tests/ -q` + all
  examples. New `examples/theme.py`: `Window.set_theme(seed, dark)`,
  a rect with `enable_interaction()` + `click()`, proving the call
  chain compiles/runs end-to-end (pixel-level tint proof stays in the
  Rust test, matching every other example/test split in this codebase).
