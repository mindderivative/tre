# Log: M26 Phase 1 — Wire Stylesheet Cascade & MD3 Token Resolution into `View` (§16.3)

Closed M26 entirely (single phase).

Confirmed via direct read before writing any code: `engine-spec`'s
`Reconciler::load`/`Reconciler::reconcile` already accepted
`sheet: Option<&Stylesheet>`/`scheme: Option<&ColorScheme>` and already
threaded both correctly through the include-aware `build_tree` path.
`engine-py::view.rs`'s `View::new`/`poll_reload` were the only two real
call sites, and both always passed `None, None`. Genuinely zero new
`engine-spec` logic was needed — this closed as a pure `engine-py`
wiring change, exactly as scoped.

`View`'s constructor gained `stylesheet: Option<String>` (a path, read
and parsed via the already-exported `engine_spec::parse_stylesheet`,
the same direct way `path` itself is already read — not routed through
`include:`'s own path-confinement machinery, since a constructor
argument is as trusted as `path` already is, unlike a path embedded
inside YAML content) and `theme_seed: Option<(u8, u8, u8, u8)>`/`dark:
bool` (built into a `DynamicTheme::from_seed`, resolving `light`/`dark`
the identical way `Window.set_theme` already does). Both stored on
`View` and threaded through both `Reconciler::load` (construction) and
`Reconciler::reconcile` (`poll_reload`), so a hot-reload keeps
resolving against the same stylesheet/scheme rather than silently
reverting to the literal-only path.

New `examples/stylesheet_tokens.py` (+ `stylesheet_tokens.yaml` +
`stylesheet_tokens_sheet.yaml`) proves both halves end to end without
any pixel readback: the real cascade precedence (baseline → `kind:` →
`classes:`) verified via the already-real `Node.get("corner_radius")`
getter (a `kind: Rect` rule sets `8`, a `classes: [accent]` rule
overrides it to `16` for one widget — both read back correctly); MD3
token resolution verified via a real before/after contrast — the
identical stylesheet's `background: primary` still fails to parse as a
literal color with no `theme_seed` given (the real, unchanged fallback
path), and resolves successfully once a real `theme_seed` is supplied.
No new Python-facing getter was needed for either proof.

Full `cargo test --workspace --release` (all pre-existing suites
unmodified and passing — the new fields are additive, `None`/no
stylesheet is byte-for-byte the prior behavior), `cargo clippy
--workspace --all-targets -- -D warnings`, `cargo fmt --check` all
clean on the first run. `maturin develop --release` + `pytest tests/`
(187 passed, unchanged, 1 pre-existing skip) and all 33 examples
(including the new one) run with the real display — all pass.

Updated `docs/guide/declarative-views.md` (replaced the "not wired up
yet" admonition with the real feature's own documentation, including
the cascade-precedence example and the no-theme/with-theme contrast)
and `docs/api/python/view.md` (new constructor signature). `mkdocs
build --strict` clean.

M26 — Declarative View Styling: Stylesheets & MD3 Color Tokens is now
fully complete.
