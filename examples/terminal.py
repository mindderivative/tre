#!/usr/bin/env python3
"""M30 Phase 9 Step 4's real `Terminal` component (§5, §8, §10): a
real, live pseudo-terminal -- `Window.add_terminal` spawns a real
shell on a real PTY (`portable_pty`) and parses its real byte stream
with a real VT100 parser (`vt100`), the identical real "spawn a real
pseudo-terminal is OS-specific process management; interpreting its
byte stream is the VT/ANSI state machine every real terminal emulator
implements identically -- neither is this widget's own concern to
reinvent" split the sibling `pyCopper` project's own real `Terminal`
widget already established.

What this script proves automatically (headless-CI-safe, no human
needed): a real click focuses the terminal; real typed keystrokes
(`Window.type_text`/`press_key`) reach the real shell's own stdin;
the shell's own real response lands in the rendered cell grid, read
back via `Node.get_text()`; and a real render loop paints the whole
live terminal -- including its background PTY reader thread's own
output arriving mid-run -- over actual frames without crashing.

**Real, honestly-scoped v1** (`Window.add_terminal`'s own Rust doc
comment has the full list): no scrollback, no mouse text selection,
no Ctrl+C/SIGINT or any other Ctrl+letter shortcut, no real terminal
resize wired to window resize, and POSIX only. What *is* real: a
genuine shell process, genuine keyboard round-trip, and genuine ANSI
color rendering (16-color palette plus the standard 256-color xterm
formula).
"""

from tre import App, Window

window = Window(width=460, height=260, title="tre v2 -- terminal")
window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=True)

terminal = window.add_terminal(
    shell="/bin/sh",
    cols=48,
    rows=12,
    background=(0x11, 0x11, 0x13, 0xFF),
    font_size=13.0,
    x=16,
    y=16,
)

print(f"before click: is_focused={terminal.is_focused()}")
window.click(terminal)
print(f"after click: is_focused={terminal.is_focused()}")
assert terminal.is_focused(), "a real click on a Terminal must focus it"

# A real, live command round-trip -- typed before the one real render
# loop starts (`App.run` is Design Principle 1's own one blocking
# call; a real Terminal's own background PTY reader thread still
# drains new output every frame it runs, so this arrives live).
window.type_text('echo "hello from a real shell"')
window.press_key("enter")
window.type_text("printf 'colors: \\033[31mred\\033[0m \\033[32mgreen\\033[0m\\n'")
window.press_key("enter")

app = App()
app.add_window(window)
app.run(max_frames=60)

text = terminal.get_text()
print("--- final terminal contents ---")
print(text)
assert "hello from a real shell" in text, "expected real shell output not found"
assert "colors:" in text, "expected the second real command's own output not found"
print("terminal.py: exited cleanly after 60 frames -- a real shell genuinely responded")
