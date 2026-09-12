#!/usr/bin/env python3
"""Phase 19 proof: real in-app widget keyboard focus and Tab-order
traversal (`tre.FocusManager`/`Canvas.tag_focusable`), and real OS-level
window focus (`InputEvent.WindowFocused`) -- the "Focus / keyboard nav /
tab order" gap the GUI Readiness assessment's own gap table marked
fully missing, and the last remaining gap that's unambiguously tre's
own responsibility (layout/theming, also missing, is deliberately
scoped to pySilver per DESIGN.md).

Two distinct kinds of "focus" are proven here, kept deliberately
separate rather than conflated:

1. In-app *widget* focus (`tre.FocusManager`, Phase 19 Step 19.2/19.3):
   pure in-memory Tab-order traversal over nodes tagged via
   `Canvas.tag_focusable`, following the real, well-known HTML
   `tabindex` convention (WHATWG HTML Standard Section 6.6.7) rather
   than an invented rule.
2. OS-level *window* focus (`InputEvent.WindowFocused`, Phase 19 Step
   19.1): real, winit-sourced `WindowEvent::Focused(bool)`, observed via
   a real window on this real desktop.

Matching this phase's own established boundary: `tre-engine` never
parses raw key codes or tracks modifier (Shift) state -- a real caller
recognizes Tab/Shift+Tab itself and calls `focus_next()`/
`focus_previous()` once it has decided a tab navigation should happen.
This demo simulates that recognition directly (as the "caller" would)
rather than needing a real, injected Tab keypress.
"""

import time

import tre_python as tre

WIDTH, HEIGHT = 300, 200

# Three real focusable rects, matching Phase 19's own tab-order rule:
# a positive tab_index is visited first (ascending), then None/0 in
# input order, and a negative tab_index is excluded from sequential
# Tab navigation but remains directly settable via set_focus.
NODE_NATURAL = 1  # tab_index=None -- geometry/input-order fallback
NODE_POSITIVE = 2  # tab_index=1 -- explicit priority
NODE_EXCLUDED = 3  # tab_index=-1 -- focusable, but skipped by Tab


def build_registry() -> "tre.ShapeRegistry":
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(tre.Rectangle(10, 10, 60, 40, tre.rgba8(200, 60, 60, 255)))
    registry.insert_rectangle(tre.Rectangle(90, 10, 60, 40, tre.rgba8(60, 200, 60, 255)))
    registry.insert_rectangle(tre.Rectangle(170, 10, 60, 40, tre.rgba8(60, 60, 200, 255)))
    return registry


def main() -> None:
    renderer = tre.WindowedRenderer("Phase 19 focus/keyboard-nav demo", WIDTH, HEIGHT)
    window = renderer.main_window
    canvas = tre.Canvas()
    registry = build_registry()

    canvas.tag_focusable(NODE_NATURAL, 10, 10, 60, 40, tab_index=None)
    canvas.tag_focusable(NODE_POSITIVE, 90, 10, 60, 40, tab_index=1)
    canvas.tag_focusable(NODE_EXCLUDED, 170, 10, 60, 40, tab_index=-1)

    # `focusable_nodes()` must be read BEFORE `render_canvas` -- it
    # deliberately consumes the canvas's own recorded content (the same
    # "read before render" ordering rule Phase 18 Step 18.3 already
    # established for `accessibility_nodes()`).
    nodes = canvas.focusable_nodes()
    assert len(nodes) == 3, f"exactly three tagged focusable nodes were expected, got {len(nodes)}"
    by_id = {n.node_id: n for n in nodes}
    assert by_id[NODE_NATURAL].tab_index is None
    assert by_id[NODE_POSITIVE].tab_index == 1
    assert by_id[NODE_EXCLUDED].tab_index == -1
    print("canvas.focusable_nodes() reports the exact tagged tab_index values -- OK")

    renderer.flatten_into(canvas, registry)
    renderer.render_canvas(window, canvas)
    print("real Vulkan render of all three rects submitted to the window's own swapchain -- OK")

    # --- Real Tab-order traversal (FocusManager.focus_next) ---
    # NODE_POSITIVE (tab_index=1) wins first per the HTML tabindex
    # convention; NODE_NATURAL (tab_index=None) is the only remaining
    # node in tab order; NODE_EXCLUDED (tab_index=-1) must never appear.
    forward = tre.FocusManager()
    forward_sequence = [forward.focus_next(nodes) for _ in range(3)]
    assert forward_sequence == [NODE_POSITIVE, NODE_NATURAL, NODE_POSITIVE], (
        f"focus_next must walk the real HTML tabindex order and wrap around, got {forward_sequence}"
    )
    assert NODE_EXCLUDED not in forward_sequence, "a negative tab_index node must never be Tab-reachable"
    print(f"FocusManager.focus_next sequence (with wraparound): {forward_sequence} -- OK")

    # --- The mirror-image Shift+Tab traversal (FocusManager.focus_previous) ---
    backward = tre.FocusManager()
    backward_sequence = [backward.focus_previous(nodes) for _ in range(3)]
    assert backward_sequence == [NODE_POSITIVE, NODE_NATURAL, NODE_POSITIVE], (
        f"focus_previous must walk the same real tab order backward and wrap around, "
        f"got {backward_sequence}"
    )
    assert NODE_EXCLUDED not in backward_sequence, "a negative tab_index node must never be Tab-reachable"
    print(f"FocusManager.focus_previous sequence (with wraparound): {backward_sequence} -- OK")

    # --- Programmatic/click-to-focus on the EXCLUDED node ---
    # A negative tab_index means "skip me during sequential Tab
    # navigation," not "not focusable" -- set_focus (a real click
    # handler's own operation) must still be able to target it directly.
    forward.set_focus(NODE_EXCLUDED)
    assert forward.focused() == NODE_EXCLUDED, (
        "set_focus must directly target a node excluded from sequential Tab order"
    )
    print("FocusManager.set_focus on the tab_index=-1 node took effect directly -- OK")

    # --- Real OS-level window focus (InputEvent.WindowFocused) ---
    # Real, disclosed scope limit: this sandbox has no input-injection
    # or window-management tool (no xdotool/wmctrl -- confirmed absent)
    # to synthesize another app stealing focus, so only the
    # window-GAINS-focus half is asserted unattended here. A human
    # confirms the window-LOSES-focus half by alt-tabbing away and back
    # during a manual run of this same demo -- matching Phase 17's own
    # "a human still needs to look at some real UI results" disclosure.
    seen_window_focused: list[bool] = []
    for _ in range(60):
        for event in renderer.poll_events():
            if isinstance(event, tre.InputEvent.WindowFocused):
                seen_window_focused.append(event.focused)
        time.sleep(1 / 60)
    assert True in seen_window_focused, (
        "expected at least one real WindowFocused(focused=True) event on window creation -- "
        f"observed: {seen_window_focused}"
    )
    print(f"real WindowFocused events observed via poll_events(): {seen_window_focused} -- OK")

    print("tre_python focus/keyboard-nav demo (Phase 19): PASSED")


if __name__ == "__main__":
    main()
