# Log: Phase 21 — Release Prep (v0.1.0 Alpha)

- **Crate count correction**: the plan assumed 15 crates in the workspace
  Cargo.toml `license.workspace = true` pass. `find crates -maxdepth 2
  -name Cargo.toml` before making any edit showed 16 (the plan's own
  workspace-member count in `Cargo.toml` was miscounted during planning).
  The `sed` pass and its verification (`grep -l`/`cargo metadata`) both
  confirmed all 16 crates, not 15 — no functional impact, just a plan-vs-
  reality correction worth recording rather than silently fixing.
- **No license anywhere, confirmed before choosing one**: a targeted
  Explore-agent grep (case-insensitive `MIT`/`Apache`/`licen[sc]e` across
  every `*.md` and `Cargo.toml`, excluding `target/`/`.venv/`/`site/`)
  found zero real hits before this phase — only false positives inside
  words like "per**mit**ted" in the old README. No `LICENSE*` file existed,
  and no `Cargo.toml` set a `license` field. This was a genuinely open
  decision, not an oversight to silently correct.
- **`mkdocs` not on the shell `PATH`** in this environment (`mkdocs build`
  via a plain shell command fails with `command not found`, even with
  `.venv` activated) — the `mkdocs-mcp` MCP server has its own bundled
  `mkdocs` and was used instead (`mkdocs_build` with `strict: true`),
  which succeeded cleanly (27 pages, zero warnings). The new `site_url` in
  `mkdocs.yml` did not introduce any build warning. This has no bearing on
  the new `.github/workflows/docs.yml` CI job, which installs its own
  `mkdocs` via `pip install mkdocs` inside a fresh runner — it doesn't
  depend on this sandbox's `PATH` at all.
- **Full workspace verification clean** after all `Cargo.toml`
  `license`/`license.workspace` additions: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo
  build --workspace` all passed with zero warnings — confirming the
  metadata-only change didn't affect compilation, as expected.
