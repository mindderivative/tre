#!/usr/bin/env python3
"""Phase 14 Step 14.4 proof: `tre.TrayIcon`/`tre.Menu` (GUI-readiness
assessment recommendation #5, tray half) -- a real system tray icon and
native context menu, backed by `tray-icon`/`muda` via `libayatana-
appindicator`'s real D-Bus StatusNotifierItem protocol on Linux.

**Real, disclosed limit on what this demo can automate**: whether a
human can actually *see* the tray icon and click it needs a human at a
real desktop -- there is no way to script that click from this process.
What was independently, manually confirmed during development (see
`README.md`): running this exact binding under `dbus-monitor` showed a
real `RegisterStatusNotifierItem` D-Bus call and a matching
`StatusNotifierItemRegistered` signal from this machine's live
`org.kde.StatusNotifierWatcher`, followed by a clean
`StatusNotifierItemUnregistered` on process exit -- end-to-end proof the
icon really did register with the desktop shell. This demo itself
sticks to plain Python assertions (no `dbus-monitor` dependency) and
proves what it honestly can on its own:

1. `tray_init()` succeeds against this machine's real display server.
2. Building a real `Menu` with an item and a separator works, and the
   item gets a real, non-empty id.
3. `TrayIcon` construction succeeds with a valid RGBA icon and consumes
   the `Menu` passed to it -- a real single-owner transfer, not a copy:
   using that same `Menu` again afterwards raises a real `ValueError`.
4. Malformed icon data (wrong byte length for the given width/height)
   raises a real error rather than corrupting memory or silently
   truncating.
5. `tray_pump_events()`/`tray_poll_events()` run without error; with no
   human interaction, the event list is empty -- a real, honest baseline
   rather than a fabricated "click" event.
"""

import tre_python as tre


def check_tray_init_succeeds() -> None:
    tre.tray_init()
    print("tray_init() succeeded against the real display server -- OK")


def check_menu_building() -> tre.Menu:
    menu = tre.Menu()
    item_id = menu.add_item("Quit", True)
    assert item_id, "a real menu item must get a real, non-empty id"
    menu.add_separator()
    print(f"Menu built with 1 item (id={item_id!r}) + 1 separator -- OK")
    return menu


def make_rgba(width: int, height: int) -> bytes:
    return bytes([255, 64, 32, 255]) * (width * height)


def check_tray_icon_creation_consumes_the_menu() -> None:
    menu = check_menu_building()
    tray = tre.TrayIcon(make_rgba(8, 8), 8, 8, tooltip="tre Phase 14 Step 14.4 demo", menu=menu)
    print("TrayIcon created with a real 8x8 icon + tooltip + menu -- OK")

    try:
        menu.add_item("too late", True)
        raise AssertionError("expected ValueError: this Menu was already consumed by the TrayIcon")
    except ValueError:
        print("re-using a Menu already attached to a TrayIcon raises ValueError -- OK")

    tray.set_tooltip("updated tooltip")
    print("set_tooltip() after creation succeeded -- OK")


def check_malformed_icon_data_raises() -> None:
    # 8x8 RGBA needs exactly 256 bytes; give it half that.
    bad_rgba = make_rgba(8, 8)[: (8 * 8 * 4) // 2]
    try:
        tre.TrayIcon(bad_rgba, 8, 8)
        raise AssertionError("expected an error for mismatched rgba length")
    except RuntimeError:
        print("TrayIcon(<mismatched rgba length>) raises a real error -- OK")


def check_event_polling_runs_and_is_empty_without_interaction() -> None:
    tre.tray_pump_events()
    events = tre.tray_poll_events()
    assert events == [], f"expected no events with no human interaction, got {events!r}"
    print("tray_pump_events()/tray_poll_events() run cleanly; no events without interaction -- OK")


def main() -> None:
    check_tray_init_succeeds()
    check_tray_icon_creation_consumes_the_menu()
    check_malformed_icon_data_raises()
    check_event_polling_runs_and_is_empty_without_interaction()
    print("tre_python system tray (Phase 14 Step 14.4) demo: PASSED")


if __name__ == "__main__":
    main()
