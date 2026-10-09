"""0.5.6 (#159): `Window.set` and `Window.resize` from inside a callback that
runs under `Window.advance` or `Window.simulate` (they used to raise
`RuntimeError: Already borrowed`)."""

from tre import Window


def test_set_from_an_animation_callback_during_advance():
    window = Window(width=200, height=120)
    box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
    window.root.add_child(box)
    seen = []

    def done():
        window.set(title="finished")
        window.resize(300, 200)
        seen.append(window.get("title"))

    box.animate("opacity", 0.2, 100, on_complete=done)
    window.advance(200)
    assert seen == ["finished"]
    assert window.get("width") == 300


def test_set_from_a_listener_during_simulate():
    window = Window(width=200, height=120)
    box = window.create("box", width=60, height=40, fill=(255, 0, 0, 255))
    window.root.add_child(box)
    box.on("click", lambda e: window.set(title="clicked"))
    window.simulate("click", node=box)
    assert window.get("title") == "clicked"
