# Demo: Phase 14 Step 14.4 -- System Tray + Native Menu (`tre.TrayIcon`)

```bash
python3 -m venv .venv   # once, from the workspace root
.venv/bin/pip install maturin numpy Pillow
.venv/bin/maturin develop --release -m crates/tre-python/Cargo.toml
cd demo/phase14_step14_4
../../.venv/bin/python demo.py
```

**What this proves.** The second half of recommendation #5 from the [tre
GUI Readiness assessment](https://claude.ai/code/artifact/2d7cafa8-bf78-4c65-ade1-a2f3c0362196):
a real system tray icon and native context menu, via `tray-icon`
(which re-exports `muda`'s menu types under its own `menu` module, so
`muda` is not a separate direct dependency) -- confirmed end to end
against this machine's real KDE Plasma session.

**A real, required integration constraint found during feasibility
testing, not assumed**: on Linux, `tray-icon` needs a real GTK event
loop pumped on the same thread that owns it. `tre`'s own windowed
rendering uses `winit`'s `EventLoop`, not GTK's, so `tre_platform::tray`
exposes `init()` (call once, before creating any `Menu`/`TrayIcon`) and
`pump_events()` (call once per frame, alongside `PlatformConnection::
poll_events()`, driving `gtk::events_pending()`/`gtk::main_iteration()`
manually) rather than assuming tre's existing event loop covers it.

**Independently, manually confirmed during development** (not part of
the automated `demo.py`, which avoids depending on an external tool):
running this exact binding under `dbus-monitor --session
"interface='org.kde.StatusNotifierWatcher'"` showed the real sequence
end to end --

```
method call ... member=RegisterStatusNotifierItem
    string "/org/ayatana/NotificationItem/tray_icon_tray_app"
signal ... member=StatusNotifierItemRegistered
    string ":1.403/org/ayatana/NotificationItem/tray_icon_tray_app"
... (on process exit)
signal ... member=StatusNotifierItemUnregistered
    string ":1.403/org/ayatana/NotificationItem/tray_icon_tray_app"
```

confirming the icon genuinely registers with and unregisters from this
machine's real desktop shell, not just that the Rust/Python calls
returned without error. This is why `libxdo` -- optional, X11-only,
used solely for `muda`'s *predefined* Copy/Cut/Paste/SelectAll menu
items (irrelevant here: a real caller already has `Clipboard` from Step
14.1 and builds regular `MenuItem`s) -- is disabled via
`default-features = false`, avoiding a real, unneeded system package
dependency on this Wayland session.

**Architecture**: `tre_platform::tray` (new `crates/tre-platform/src/
tray.rs`) exposes `init`/`pump_events`/`poll_events` as free functions
(matching `tray-icon`'s own global-channel event model, not a
per-instance callback) plus `Menu`/`TrayIcon` structs. `tre-python`
binds these as `tre.tray_init`/`tray_pump_events`/`tray_poll_events`
plus `tre.Menu`/`tre.TrayIcon`, both marked `unsendable` (matching
`PyClipboard`'s own precedent for platform state that isn't safely
`Send` -- GTK's thread-affinity requirement means these must stay
pinned to whichever Python thread created them). `Menu` ownership is a
real, single-owner transfer into `TrayIcon` at construction (matching
`TrayIconBuilder::with_menu`'s own contract): the Rust side tracks this
with an `Option<Menu>` that `.take()`s on consumption, and the Python
side raises a real `ValueError` if a caller tries to keep using a
`Menu` already attached to a `TrayIcon`.

**Verified:** `cargo fmt --all -- --check`/`cargo clippy --workspace
--all-targets -- -D warnings`/`cargo build --workspace --all-targets`/
`cargo test --workspace` all clean in debug (`tre-platform` gains 2 new
unit tests: building a real menu with an item + separator via real GTK
calls, and confirming `poll_events()` starts empty -- both run
repeatedly across multiple `cargo test` invocations to rule out thread-
affinity flakiness from GTK being initialized on a test-harness thread
rather than a designated "main" thread; all runs passed consistently).
`--release` clean apart from the same 5 pre-existing, already-flagged
`debug_assert!` failures (2 in `tre-engine`, 3 in `tre-memory`,
unrelated to this step). `demo.py`, run via `maturin develop --release`:
`tray_init()` succeeds against the real display server; a real `Menu`
builds with a non-empty item id; `TrayIcon` creation consumes the
`Menu` (reusing it afterwards raises `ValueError`); malformed icon data
raises a real error instead of corrupting memory; and event polling
runs cleanly with an honestly empty result (no human interaction
occurred).

**Real, disclosed limit on what an automated demo can prove**: whether
a human can actually see the tray icon and click it needs a human at a
real desktop -- there is no way to script that click from this process,
matching Step 14.3's own disclosed limit for file dialogs. Full click-
through verification (does clicking a menu item produce the expected
`TrayEvent.MenuItemClick` from `tray_poll_events()`?) is real, separate
manual verification a human running this demo must perform.

**Not yet done**: predefined system menu items (`Copy`/`Cut`/`Paste`/
`SelectAll`, `libxdo`-gated) are not exposed -- a real caller builds a
menu with `Clipboard` (Step 14.1)-backed items instead, matching this
project's own "smallest real slice" precedent. Tray icon image updates
after creation (`set_icon`, distinct from `set_tooltip`) are also not
yet exposed -- real, separate future work if a caller needs a dynamic
tray icon (e.g. reflecting unread-count state).
