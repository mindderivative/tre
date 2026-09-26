"""M97 Phase 2 Step 4: the binding and cascade audit
(`docs/design/binding-cascade-audit.md`) -- its findings about how a
`View` applies bindings and handlers, pinned. The grammar and cascade
findings are pinned in `crates/engine-spec/tests/audit.rs`.
"""

from __future__ import annotations

from typing import Any

import pytest

from tre import Signal, View, ViewModel

STYLE = 'style: {background: "#112233", opacity: 1.0}'


def rect_view(binding: str) -> View:
    source = f'id: r\nkind: Rect\n{STYLE}\nbindings: {{opacity: "{binding}"}}\n'
    return View(source=source, path="audit.yaml")


def model(view: View, **signals: Any) -> Any:
    class VM(ViewModel):
        def __init__(self, view: View) -> None:
            for name, value in signals.items():
                setattr(self, name, Signal(value))
            super().__init__(view)

    return VM(view)


def test_p1_a_text_binding_must_resolve_to_a_string() -> None:
    view = View(
        source='id: t\nkind: Text\ntext: {content: "x", font_family: Roboto, font_size: 14}\n'
        'style: {foreground: black}\nbindings: {text: "{{ n.get() }}"}\n',
        path="audit.yaml",
    )
    with pytest.raises(ValueError, match="expects a string binding, got Int"):
        model(view, n=3)


def test_p2_a_binding_subscribes_only_to_what_its_first_evaluation_read() -> None:
    view = rect_view("{{ gate.get() and level.get() }}")
    vm = model(view, gate=0.0, level=0.5)
    node = view.node("r")
    vm.gate.set(1.0)  # read the first time: re-evaluates, now reading level
    assert node.get("opacity") == 0.5
    vm.level.set(0.2)  # not read the first time, so never subscribed
    assert node.get("opacity") == 0.5


def test_p3_a_failing_re_evaluation_raises_from_the_signal_write() -> None:
    view = rect_view("{{ a.get() / b.get() }}")
    vm = model(view, a=1, b=2)
    with pytest.raises(ValueError, match="unsupported operation Div"):
        vm.b.set(0)
    assert vm.b.get() == 0, "the Signal still took the value"


def test_p4_a_failing_binding_stops_the_signals_later_subscribers() -> None:
    source = f"""id: root
kind: Container
children:
  - id: bad
    kind: Rect
    {STYLE}
    bindings: {{opacity: "{{{{ 1 / b.get() }}}}"}}
  - id: good
    kind: Rect
    {STYLE}
    bindings: {{opacity: "{{{{ b.get() / 10 }}}}"}}
"""
    view = View(source=source, path="audit.yaml")
    vm = model(view, b=2)
    with pytest.raises(ValueError):
        vm.b.set(0)
    assert view.node("good").get("opacity") == pytest.approx(0.2), "never updated to 0"


def test_p5_an_unknown_event_name_is_accepted_and_never_fires() -> None:
    source = f"id: r\nkind: Rect\n{STYLE}\nhandlers: {{on_clik: go}}\n"
    view = View(source=source, path="audit.yaml")
    fired: list[str] = []

    class VM(ViewModel):
        def go(self) -> None:
            fired.append("go")

    VM(view)
    view.click(view.node("r"))
    assert fired == []


def test_p6_an_int_beyond_64_bits_arrives_as_a_float() -> None:
    view = rect_view("{{ big.get() - small.get() }}")
    model(view, big=2**70 + 1, small=2**70)
    assert view.node("r").get("opacity") == 0.0, "2**70 + 1 - 2**70, in floats"
