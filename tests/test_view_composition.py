"""M19 Phase 2 (§16.6): real, repeatable coverage that a `View` loaded
from a real, multi-file `view.yaml` genuinely splices `include:` into
an ordinary part of the tree, through the real Python FFI surface for
the first time. `engine-spec`'s own tests already proved `parse_view_
with_includes` in isolation (a real two-file splice, nested includes,
path confinement, cycle detection, the depth limit) -- this file
proves the same real capability reaches `View`, the actual entry point
a Python app uses.
"""

from tre import View


def write(tmp_path, name, content):
    path = tmp_path / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    return path


def test_a_view_with_a_real_include_loads_the_included_widget(tmp_path):
    write(
        tmp_path,
        "dialog.yaml",
        """
id: confirm
kind: Container
style: {width: 100, height: 40}
""",
    )
    main = write(
        tmp_path,
        "main.yaml",
        """
id: root
kind: Container
style: {width: 200, height: 200}
children:
  - include: dialog.yaml
""",
    )
    view = View(str(main))
    node = view.node("confirm")
    assert node is not None, "the included widget's own id must be reachable, same as an inline one"


def test_an_absolute_include_path_is_rejected_through_the_real_ffi_surface(tmp_path):
    main = write(
        tmp_path,
        "main.yaml",
        """
id: root
kind: Container
children:
  - include: /etc/passwd
""",
    )
    try:
        View(str(main))
        raised = False
    except ValueError:
        raised = True
    assert raised, "an absolute include path must fail loudly through the real FFI path too"
