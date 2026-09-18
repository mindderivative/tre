# Plan: M26 Phase 1 — Wire Stylesheet Cascade & MD3 Token Resolution into `View` (§16.3), closing M26

Corresponds to `BUILD_TRACKER.md` M26 Phase 1.

## Investigation (already done, confirmed via direct read)

- `crates/engine-spec/src/reconcile.rs`'s `Reconciler::load`/`Reconciler::
  reconcile` already accept `sheet: Option<&Stylesheet>, scheme:
  Option<&ColorScheme>` and thread both correctly through `build_tree`
  (the include-aware path `View` already uses).
- `crates/engine-py/src/view.rs`'s `View::new`/`poll_reload` are the
  *only* two real call sites of those methods in this crate, and both
  always pass `None, None` today (confirmed via direct read of both).
- `engine_spec::{Stylesheet, parse_stylesheet}` are already re-exported
  at the crate root (`lib.rs`); `engine_md3::{ColorScheme,
  DynamicTheme}` are too. `engine-py`'s own `Cargo.toml` already
  depends on both crates — no new dependency needed.
- `DynamicTheme::from_seed(seed: Color) -> DynamicTheme { light:
  ColorScheme, dark: ColorScheme }` is the exact same real mechanism
  `Window.set_theme` already calls; picking `light`/`dark` by a plain
  `bool` mirrors `Window.set_theme(seed, dark=False)`'s own real
  parameter shape.
- The stylesheet path a `View(...)` call is given is a plain Python
  constructor argument — as trusted as `path` itself already is — so
  it's read directly via `std::fs::read_to_string`, the same way
  `path` already is, not through `include:`'s own confined-path
  resolution (that mechanism exists because `include:` paths are
  embedded inside YAML *content*, not supplied directly by the caller).

## What will change

`crates/engine-py/src/view.rs`:

- New imports: `engine_md3::{ColorScheme, DynamicTheme}`,
  `engine_spec::{Stylesheet, parse_stylesheet}` (added to the existing
  `engine_spec::{...}` import list), `peniko::Color`.
- `View`'s `#[new]` constructor gains
  `#[pyo3(signature = (path, stylesheet=None, theme_seed=None, dark=false))]`:
  `stylesheet: Option<String>` (a path to a stylesheet YAML file, read
  and parsed via `parse_stylesheet`), `theme_seed: Option<(u8, u8, u8,
  u8)>` (built into a `DynamicTheme::from_seed`, resolving `light` or
  `dark` per the `dark` flag into a real `ColorScheme`).
- `View` struct gains two new fields: `stylesheet: Option<Stylesheet>`,
  `scheme: Option<ColorScheme>` — both stored so `poll_reload` can
  reuse them on every future reconcile, not just the initial build.
- Both the constructor's `Reconciler::load(...)` call and
  `poll_reload`'s `self.reconciler.reconcile(...)` call pass
  `self.stylesheet.as_ref()`/`self.scheme.as_ref()` instead of the
  current hardcoded `None, None`.
- New `examples/` script proving both a real stylesheet cascade
  (`kind`/`classes`/`id` selectors) and a real MD3 token name
  (`background: primary`) resolve correctly through the real Python
  `View` API for the first time — the same "real, end-to-end proof"
  standard every other milestone this session has held itself to.
- Update `docs/guide/declarative-views.md`'s existing "Stylesheets and
  MD3 color tokens aren't wired up from Python yet" admonition — this
  phase closes that exact gap — and add the new constructor params to
  `docs/api/python/view.md`.

## Testing

- `cargo test --workspace --release`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `maturin develop --release`
- `pytest tests/ -v`
- Run the new example script plus the full example suite (real
  display, no env stripping).
- `mkdocs build --strict` after the docs updates.
