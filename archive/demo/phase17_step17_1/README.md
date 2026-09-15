# Demo: Phase 17 Step 17.1 -- Tray/Clipboard Follow-ups

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase17_step17_1
../../.venv/bin/python demo.py
```

**What this proves.** GUI Readiness recommendation 11, both halves:

> `tray-icon`'s predefined Copy/Cut/Paste/SelectAll menu items are gated
> behind `libxdo` (X11-only, disabled here) -- a real caller builds
> equivalent items backed by `Clipboard` instead, which already works.
> Tray icon image updates after creation (`set_icon`, distinct from
> `set_tooltip`) aren't exposed yet either -- real, separate future work
> if a caller needs a dynamic tray icon (e.g. reflecting unread-count
> state).

**Dynamic tray icon updates.** `TrayIcon.set_icon(rgba, width, height)`
(new, both `tre_platform::tray::TrayIcon` and `PyTrayIcon`) changes the
real tray icon after creation -- previously the only post-creation
mutator was `set_tooltip`. On this machine's real Linux/GTK
(`appindicator`) backend there is no in-memory icon API at all: the
backend writes each new icon to a real temporary PNG file on disk and
points the indicator at that path. `TrayIcon.set_temp_dir_path(path)`
(also new, `tray-icon`'s own real, Linux-only API) redirects where that
file lands, letting this demo verify `set_icon`'s real effect
automatically -- reading the actual written PNG back with Pillow and
comparing pixels -- rather than only checking that the call didn't
raise.

**A real, previously-undisclosed bug found while building this,
disclosed and fixed in Phase 14 Step 14.4's own docs/demo**:
`set_tooltip` is a genuine no-op on this machine's real Linux/GTK
backend -- `tray-icon` 0.19.3's own `platform_impl/gtk/mod.rs`
implements it as `pub fn set_tooltip<S>(&mut self, _tooltip: Option<S>)
-> Result<()> { Ok(()) }`, silently discarding the argument. Step
14.4's own demo originally treated a successful `set_tooltip(...)` call
as proof it "succeeded" -- never a valid inference on Linux. Corrected
there to assert only what's real (the call doesn't raise) and disclose
the no-op directly. This step's own `set_icon`, by contrast, genuinely
does something checkable on this platform, which is exactly why its
own real effect is verified here via the written PNG rather than just
"it didn't raise."

**Predefined clipboard menu items.** `tray-icon`'s own `libxdo`-gated
predefined items stay disabled (irrelevant on this Wayland session, a
real system-package dependency this project doesn't need). The real
answer recommendation 11 itself already gives -- a caller builds plain
`Menu` items backed by `Clipboard` instead -- had never actually been
demonstrated end to end before this step. This demo builds a real Menu
with Copy/Cut/Paste items, gets back real, distinct ids, and proves the
exact id-based dispatch a real click handler would use correctly
drives real `Clipboard.set_text`/`get_text` calls (including that an
unrelated id dispatches to nothing). The one thing this can't
automate -- a human physically clicking the item -- is the same
already-disclosed limit every tray/dialog demo in this project
discloses; what's proven here is that the dispatch logic a real click
would drive is itself correct, using `tre.TrayEvent.MenuItemClick`'s
own direct Python constructibility (confirmed the same PyO3 "complex
enum" pattern `PyMouseButton` already established) to drive it without
a real click.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug. `--release` clean apart
from the same 5 pre-existing, already-disclosed `debug_assert!`
failures (2 in `tre-engine`, 3 in `tre-memory`, unrelated to this
step). `demo/phase14_step14_4/demo.py` still passes after this step's
own changes (a real regression check -- `set_icon`/`set_temp_dir_path`
are purely additive to `TrayIcon`).

`demo.py`, run via `maturin develop --release` against this machine's
real display server: `set_icon(blue)` then `set_icon(green)` each write
a real, distinct PNG with the exact requested pixel content; malformed
icon data (wrong byte length) raises a real error; and the Copy/Cut/
Paste dispatch check round-trips real clipboard content correctly
(Copy sets it, Cut sets-then-clears the document, Paste appends it
back), with an unrelated item id correctly triggering no handler.

**Real, disclosed scope limits**: `set_icon`/`set_temp_dir_path`'s
"writes a real file to disk" mechanism is specific to this machine's
Linux/`appindicator` backend (confirmed by reading `tray-icon`'s own
platform-specific source, not assumed) -- other platforms' own
`set_icon` implementations may differ in mechanism while presenting
the identical public API; `set_temp_dir_path` is a real no-op on
non-Linux platforms, matching `tray-icon`'s own upstream contract.
Whether a human can actually see either update on their own desktop
still needs a human at a real desktop, the same limit Step 14.4 already
disclosed.
