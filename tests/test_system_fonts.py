"""0.5.4 (#111): `tre.set_system_fonts` -- the opt-in to installed fonts."""

import pytest

import tre
from tre import Window


@pytest.fixture(autouse=True)
def restore_default():
    yield
    tre.set_system_fonts(False)


def text_window(content):
    window = Window(width=260, height=60)
    window.root.set(fill=(255, 255, 255, 255), padding=0)
    label = window.create("text", text=content, font_family="Roboto", font_size=32,
                          width=250, height=50)
    window.root.add_child(label)
    return window


def test_it_is_off_by_default():
    assert tre.system_fonts() is False


def test_it_can_be_turned_on_and_off():
    tre.set_system_fonts(True)
    assert tre.system_fonts() is True
    tre.set_system_fonts(False)
    assert tre.system_fonts() is False


@pytest.mark.parametrize("value", [1, "yes", None])
def test_it_takes_only_a_bool(value):
    with pytest.raises(TypeError):
        tre.set_system_fonts(value)
    assert tre.system_fonts() is False


def test_off_is_hermetic_and_turning_it_off_again_restores_it_exactly():
    window = text_window("漢字かな 😀 שלום")
    hermetic = window.snapshot()
    assert window.snapshot() == hermetic
    tre.set_system_fonts(True)
    with_system = window.snapshot()
    tre.set_system_fonts(False)
    assert window.snapshot() == hermetic
    if with_system == hermetic:
        pytest.skip("this machine has no system font for these scripts")
    assert with_system != hermetic


def test_text_the_bundled_fonts_cover_is_the_same_either_way():
    window = text_window("Hello, world")
    plain = window.snapshot()
    tre.set_system_fonts(True)
    assert window.snapshot() == plain


def test_a_registered_font_fills_in_glyphs_the_nodes_family_lacks():
    """Hermetic fallback: runs in its own process, as registration can't be undone."""
    import subprocess
    import sys
    import textwrap
    from pathlib import Path

    font = Path(__file__).parent.parent / "crates/engine-render/assets/fonts/HackNerdFontMono-Regular.ttf"
    if not font.exists():
        pytest.skip("the repository's font assets aren't here")
    script = textwrap.dedent(f"""
        import tre
        from tre import Window
        def shot(text):
            w = Window(width=200, height=50)
            w.root.set(fill=(255, 255, 255, 255), padding=0)
            w.root.add_child(w.create("text", text=text, font_family="Roboto", font_size=32,
                                      width=190, height=40, fill=(0, 0, 0, 255)))
            return w.snapshot()
        before, latin = shot("\\ue0b0"), shot("Hello")
        tre.register_font(open({str(font)!r}, "rb").read())
        assert shot("\\ue0b0") != before, "the glyph should now come from the registered font"
        assert shot("Hello") == latin, "Latin should be untouched"
        print("ok")
    """)
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip().endswith("ok")
