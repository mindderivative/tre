#!/usr/bin/env python3
"""Phase 14 Step 14.1 proof: `tre.Clipboard` -- real system clipboard
text access, recommendation #4 from the GUI readiness assessment.
Binds directly to `tre_platform::Clipboard`, itself a thin wrapper over
`arboard::Clipboard` (the same crate the assessment itself named as "a
small, self-contained crate with no architectural entanglement with the
rest of tre").

Every check below round-trips through the REAL, live OS clipboard
service on this machine -- not a mock or an in-process stand-in.
"""

import uuid

import tre_python as tre


def check_basic_round_trip(clipboard: "tre.Clipboard") -> None:
    # A unique marker per run -- the real system clipboard is a shared,
    # stateful OS resource that may already hold unrelated content from
    # something else on this machine, so the test must never assume it
    # starts empty.
    marker = f"tre clipboard round-trip {uuid.uuid4()}"
    clipboard.set_text(marker)
    read_back = clipboard.get_text()
    assert read_back == marker, f"expected {marker!r}, got {read_back!r}"
    print(f"set_text/get_text round-trip through the real OS clipboard: {marker!r} -- OK")


def check_overwrite_replaces_previous_content(clipboard: "tre.Clipboard") -> None:
    first = f"tre clipboard first {uuid.uuid4()}"
    second = f"tre clipboard second {uuid.uuid4()}"
    clipboard.set_text(first)
    assert clipboard.get_text() == first
    clipboard.set_text(second)
    read_back = clipboard.get_text()
    assert read_back == second, f"a second set_text must fully replace the first, got {read_back!r}"
    assert read_back != first
    print("a second set_text() call fully replaces the previous clipboard content -- OK")


def check_real_unicode_text_round_trips_exactly() -> None:
    clipboard = tre.Clipboard()
    # Real multi-byte UTF-8 content: accented Latin, a CJK phrase, and a
    # real emoji (a 4-byte UTF-8 sequence) -- proving this isn't an
    # ASCII-only path through arboard/the platform clipboard service.
    marker = f"café 你好 🎨 {uuid.uuid4()}"
    clipboard.set_text(marker)
    read_back = clipboard.get_text()
    assert read_back == marker, f"real UTF-8 text must round-trip byte-exact, got {read_back!r}"
    print(f"real multi-byte UTF-8 text round-trips exactly: {marker!r} -- OK")


def main() -> None:
    clipboard = tre.Clipboard()
    check_basic_round_trip(clipboard)
    check_overwrite_replaces_previous_content(clipboard)
    check_real_unicode_text_round_trips_exactly()
    print("tre_python Clipboard (Phase 14 Step 14.1) demo: PASSED")


if __name__ == "__main__":
    main()
