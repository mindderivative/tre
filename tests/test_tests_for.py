"""`tools/tests_for.py`: which tests cover the files you touched.

CI's partial tier uses its Python-test selection, so a silent loss of matches (one happened: a
refactor dropped the loop that adds the tests a changed Rust source file names) would run too few
tests. These pin the matches for a few real files; they read the repo's own tests and run nothing.
"""

import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent.parent / "tools"
sys.path.insert(0, str(TOOLS))
import tests_for  # noqa: E402


def test_a_changed_test_file_is_itself_the_test():
    assert "tests/test_layers.py" in tests_for.plan(["tests/test_layers.py"]).pytest


def test_a_changed_python_surface_file_gets_the_tests_that_name_it():
    found = tests_for.plan(["crates/engine-py/src/layer_window.rs"])
    assert {"tests/test_layer_window.py", "tests/test_layer_window_live.py"} <= set(found.pytest)
    assert "engine-py" in found.crates and not found.unmatched


def test_a_changed_rust_source_gets_the_rust_tests_that_name_it():
    found = tests_for.plan(["crates/engine-render/src/damage.rs"])
    assert any(cmd.endswith("--test damage") for cmd in found.cargo)


def test_a_changed_python_module_gets_its_tests():
    assert "tests/test_snapshot.py" in tests_for.plan(["python/tre/snapshot.py"]).pytest


def test_a_source_file_no_test_names_is_reported_as_unmatched():
    found = tests_for.plan(["crates/engine-core/src/lib.rs"])
    assert found.unmatched == ["crates/engine-core/src/lib.rs"]


def test_prose_and_scripts_map_to_no_tests():
    found = tests_for.plan(["docs/guide/text.md", "tools/status.sh"])
    assert not found.pytest and not found.cargo and not found.unmatched


def test_a_changed_source_gets_the_examples_that_use_it():
    found = tests_for.plan(["crates/engine-core/src/gradient.rs"])
    assert "examples/gradients.py" in found.examples
    found = tests_for.plan(["crates/engine-py/src/layer_window.rs"])
    assert {"examples/layer_bar.py", "examples/popup_menu.py"} <= set(found.examples)


def test_a_changed_example_is_itself_an_example_to_run():
    assert tests_for.plan(["examples/slider.py"]).examples == {"examples/slider.py": "changed"}
