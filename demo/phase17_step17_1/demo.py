#!/usr/bin/env python3
"""Phase 17 Step 17.1 proof: GUI-readiness recommendation #11, both
halves.

1. Dynamic tray icon updates: `tray.set_icon(rgba, w, h)` changes the
   real tray icon after creation (`tre.TrayIcon` previously had no way
   to do this at all -- only `set_tooltip`). On this machine's real
   Linux/GTK (`appindicator`) backend, there is no in-memory icon API;
   the backend writes each new icon to a real temporary PNG file on
   disk and points the indicator at it. `tray.set_temp_dir_path(...)`
   redirects where that file lands, letting this demo verify the
   update's real effect automatically -- reading the actual written PNG
   back and comparing pixels -- rather than only checking that the call
   didn't raise (the same real gap Phase 14 Step 14.4's own demo had
   for `set_tooltip`, corrected there this same round).

2. Predefined clipboard menu items: `tray-icon`'s own `Copy`/`Cut`/
   `Paste`/`SelectAll` predefined items are gated behind `libxdo`
   (X11-only, disabled on this Wayland session) -- the GUI Readiness
   assessment's own real answer is that a caller builds equivalent
   plain `Menu` items backed by `tre.Clipboard` instead. This demo
   proves that pattern actually works end to end: real, stable menu
   item ids dispatch correctly to real `Clipboard.set_text`/`get_text`
   calls. The one thing this can't automate -- a human physically
   clicking the item -- is the same already-disclosed limit every
   tray/dialog demo in this project discloses; what's proven here is
   that the DISPATCH LOGIC a real click would drive is itself correct.
"""

import glob
import os
import tempfile

from PIL import Image

import tre_python as tre

WIDTH, HEIGHT = 4, 4


def make_rgba(r: int, g: int, b: int) -> bytes:
    return bytes([r, g, b, 255]) * (WIDTH * HEIGHT)


def check_dynamic_icon_update_writes_real_pixels(tray: "tre.TrayIcon") -> None:
    temp_dir = tempfile.mkdtemp(prefix="tre_phase17_step17_1_")
    tray.set_temp_dir_path(temp_dir)

    tray.set_icon(make_rgba(0, 0, 255), WIDTH, HEIGHT)  # blue
    tre.tray_pump_events()
    written = sorted(glob.glob(os.path.join(temp_dir, "*.png")), key=os.path.getmtime)
    assert written, "set_icon must write a real PNG file to the redirected temp dir"
    pixel = Image.open(written[-1]).convert("RGBA").getpixel((0, 0))
    assert pixel == (0, 0, 255, 255), f"expected the real blue icon just set, got {pixel}"
    print(f"set_icon(blue) wrote a real PNG at {written[-1]!r} with pixel {pixel} -- OK")

    tray.set_icon(make_rgba(0, 255, 0), WIDTH, HEIGHT)  # green
    tre.tray_pump_events()
    written_again = sorted(glob.glob(os.path.join(temp_dir, "*.png")), key=os.path.getmtime)
    pixel2 = Image.open(written_again[-1]).convert("RGBA").getpixel((0, 0))
    assert pixel2 == (0, 255, 0, 255), (
        f"a second real set_icon() call must write a DIFFERENT real pixel, got {pixel2}"
    )
    assert written_again[-1] != written[0] or len(written_again) > len(written), (
        "the second update must produce its own new real file, not silently reuse the first"
    )
    print(f"set_icon(green) overwrote/added a new real PNG with pixel {pixel2} -- OK")


def check_malformed_icon_update_raises() -> None:
    tray = tre.TrayIcon(make_rgba(255, 0, 0), WIDTH, HEIGHT)
    bad_rgba = make_rgba(255, 0, 0)[: (WIDTH * HEIGHT * 4) // 2]
    try:
        tray.set_icon(bad_rgba, WIDTH, HEIGHT)
        raise AssertionError("expected an error for mismatched rgba length")
    except RuntimeError:
        print("set_icon(<mismatched rgba length>) raises a real error -- OK")


def check_menu_items_dispatch_to_real_clipboard_by_id() -> None:
    """Builds a real Menu with Copy/Cut/Paste items (the real, working
    answer to `libxdo`-gated predefined items), then proves the exact
    id-based dispatch a real click handler would use actually drives
    real `tre.Clipboard` calls correctly -- including that an
    unrelated id correctly dispatches to nothing."""
    menu = tre.Menu()
    copy_id = menu.add_item("Copy", True)
    cut_id = menu.add_item("Cut", True)
    paste_id = menu.add_item("Paste", True)
    menu.add_separator()
    ids = {copy_id, cut_id, paste_id}
    assert len(ids) == 3, f"Copy/Cut/Paste must each get their own real, distinct id: {ids}"
    print(f"Menu built with real Copy/Cut/Paste ids: copy={copy_id!r} cut={cut_id!r} paste={paste_id!r}")

    clipboard = tre.Clipboard()
    document_text = "the real text a caller's own EditableText selection would supply"

    def dispatch(event: "tre.TrayEvent", document: str) -> str:
        """Exactly the real handler a caller would wire to `tray_poll_events()`
        -- this demo drives it directly with a constructed event since a
        real physical click can't be scripted."""
        if not isinstance(event, tre.TrayEvent.MenuItemClick):
            return document
        if event.item_id == copy_id:
            clipboard.set_text(document)
        elif event.item_id == cut_id:
            clipboard.set_text(document)
            document = ""
        elif event.item_id == paste_id:
            document += clipboard.get_text()
        return document

    document_text = dispatch(tre.TrayEvent.MenuItemClick(item_id=copy_id), document_text)
    assert clipboard.get_text() == (
        "the real text a caller's own EditableText selection would supply"
    ), "Copy's real id must dispatch to a real Clipboard.set_text call"
    print("MenuItemClick(copy_id) dispatched to a real Clipboard.set_text -- OK")

    remaining = dispatch(tre.TrayEvent.MenuItemClick(item_id=cut_id), document_text)
    assert remaining == "", "Cut's real id must both copy AND clear the real document text"
    print("MenuItemClick(cut_id) dispatched to real Clipboard.set_text + cleared the document -- OK")

    restored = dispatch(tre.TrayEvent.MenuItemClick(item_id=paste_id), "")
    assert restored == "the real text a caller's own EditableText selection would supply", (
        f"Paste's real id must dispatch to a real Clipboard.get_text call, got {restored!r}"
    )
    print("MenuItemClick(paste_id) dispatched to real Clipboard.get_text -- OK")

    untouched = dispatch(tre.TrayEvent.MenuItemClick(item_id="not-a-real-item-id"), "unchanged")
    assert untouched == "unchanged", "an unrelated real id must dispatch to nothing at all"
    print("MenuItemClick(<unrelated id>) correctly dispatches to no handler -- OK")


def main() -> None:
    tre.tray_init()
    tray = tre.TrayIcon(make_rgba(255, 0, 0), WIDTH, HEIGHT, tooltip="Phase 17 Step 17.1 demo")

    check_dynamic_icon_update_writes_real_pixels(tray)
    check_malformed_icon_update_raises()
    check_menu_items_dispatch_to_real_clipboard_by_id()

    print("tre_python tray/clipboard follow-ups (Phase 17 Step 17.1) demo: PASSED")


if __name__ == "__main__":
    main()
