"""`tools/ci_plan.py`: which CI tier a change gets, and what a partial run covers.

The planner reads git and the crates' Cargo.toml files and runs nothing, so these tests feed it
file lists and a crate graph and check the plan. They need no GPU, display or built extension.
"""

import sys
from pathlib import Path

import pytest

TOOLS = Path(__file__).resolve().parent.parent / "tools"
sys.path.insert(0, str(TOOLS))
import ci_plan  # noqa: E402

GRAPH = {
    "engine-core": set(),
    "engine-render": {"engine-core", "engine-platform"},  # engine-platform is a dev-dependency
    "engine-platform": {"engine-core"},
    "engine-py": {"engine-core", "engine-render", "engine-platform"},
}


def plan(files, **kw):
    kw.setdefault("graph", GRAPH)
    kw.setdefault("select_tests", lambda files: ([], [], 100))
    return ci_plan.decide(files, **kw)


def test_the_real_workspace_graph_has_the_layering_the_rules_assume():
    real = ci_plan.crate_graph()
    assert set(real) == set(GRAPH)
    assert real["engine-core"] == set()
    assert "engine-core" in real["engine-platform"]
    assert {"engine-core", "engine-render", "engine-platform"} <= real["engine-py"]
    assert ci_plan.LOWER_CRATES | {ci_plan.PY_CRATE} == set(real)


@pytest.mark.parametrize(
    "changed, expected",
    [
        ({"engine-core"}, ["engine-core", "engine-platform", "engine-py", "engine-render"]),
        ({"engine-platform"}, ["engine-platform", "engine-py", "engine-render"]),
        ({"engine-render"}, ["engine-py", "engine-render"]),
        ({"engine-py"}, ["engine-py"]),
        (set(), []),
    ],
)
def test_a_change_reaches_the_crates_that_depend_on_it(changed, expected):
    assert ci_plan.affected_crates(changed, GRAPH) == expected


@pytest.mark.parametrize(
    "files",
    [
        ["docs/guide/text.md"],
        ["README.md", "LOG.md", "docs/index.md"],
        ["mkdocs.yml", "docs/api/python.md"],
        [".claude/skills/release-checklist/SKILL.md", "AGENTS.md"],
        [".github/workflows/docs.yml"],
        [],
    ],
)
def test_documentation_alone_gets_the_docs_tier(files):
    result = plan(files)
    assert result.tier == "docs"
    assert result.run_docs and not result.rust and result.pytest_scope == "none"


@pytest.mark.parametrize(
    "files",
    [
        ["crates/engine-core/src/node.rs"],
        ["docs/guide/text.md", "python/tre/__init__.py"],  # docs and code together
        ["tests/test_layers.py"],
        ["examples/popup_menu.py"],
        ["tools/gen_rust_api.py"],
        ["pyproject.toml"],
    ],
)
def test_any_code_in_the_change_gets_the_partial_tier(files):
    assert plan(files).tier == "partial"


def test_a_version_move_is_a_release_and_gets_the_full_tier():
    result = plan(["Cargo.toml", "pyproject.toml", "docs/migrating-0.5.md"], version_changed=True)
    assert result.tier == "full" and "release" in result.reason
    assert plan(["docs/guide/text.md"], version_changed=True).tier == "full"


@pytest.mark.parametrize(
    "path", ["Cargo.lock", "rust-toolchain.toml", "Cargo.toml", ".github/workflows/ci.yml"]
)
def test_build_configuration_that_can_break_another_platform_gets_the_full_tier(path):
    assert plan([path, "crates/engine-py/src/app.rs"]).tier == "full"


def test_the_label_and_a_manual_run_ask_for_the_full_tier():
    assert plan(["docs/guide/text.md"], labels=("full-ci",)).tier == "full"
    assert plan(["docs/guide/text.md"], labels=("bug",)).tier == "docs"
    assert plan([], forced="full").tier == "full"
    assert plan([], forced="docs").tier == "docs"


def test_not_knowing_what_changed_is_the_full_tier():
    result = plan(None)
    assert result.tier == "full" and "cannot tell" in result.reason


def test_a_lower_crate_runs_every_python_test_because_it_reaches_python_indirectly():
    for crate in ("engine-core", "engine-render", "engine-platform"):
        result = plan([f"crates/{crate}/src/lib.rs"])
        assert result.tier == "partial" and result.pytest_scope == "all", crate
        assert result.build_ext and result.rust


def test_a_change_to_the_python_surface_runs_the_tests_that_name_it_plus_the_smoke_set():
    picked = ["tests/test_layer_window.py", "tests/test_outputs.py"]
    result = plan(["crates/engine-py/src/layer_window.rs"], select_tests=lambda f: (picked, [], 100))
    assert result.pytest_scope == "selected"
    assert set(picked) <= set(result.pytest_files) and set(ci_plan.SMOKE_TESTS) <= set(result.pytest_files)
    assert result.affected == ["engine-py"] and result.typecheck


def test_a_source_file_no_test_names_runs_the_whole_suite():
    result = plan(["crates/engine-py/src/clock.rs"], select_tests=lambda f: ([], ["crates/engine-py/src/clock.rs"], 100))
    assert result.pytest_scope == "all" and "clock.rs" in result.why_pytest


def test_a_pick_that_is_most_of_the_suite_runs_the_whole_suite():
    many = [f"tests/test_{i}.py" for i in range(60)]
    result = plan(["crates/engine-py/src/app.rs"], select_tests=lambda f: (many, [], 100))
    assert result.pytest_scope == "all"


def test_a_selection_that_cannot_be_made_runs_the_whole_suite():
    result = plan(["python/tre/snapshot.py"], select_tests=lambda f: None)
    assert result.pytest_scope == "all"


def test_a_changed_example_alone_runs_that_example_and_no_tests():
    result = plan(["examples/popup_menu.py"])
    assert result.examples == ["examples/popup_menu.py"] and result.pytest_scope == "none"
    assert not result.rust and not result.typecheck


def test_a_docs_page_beside_code_also_builds_the_site():
    assert plan(["crates/engine-py/src/app.rs", "docs/reference/app.md"]).run_docs
    assert not plan(["crates/engine-py/src/app.rs"]).run_docs


def test_a_tool_with_its_own_test_runs_that_test():
    result = ci_plan.decide(
        ["tools/ci_plan.py"], graph=GRAPH, select_tests=lambda f: (["tests/test_ci_plan.py"], [], 100)
    )
    assert result.pytest_scope == "selected" and "tests/test_ci_plan.py" in result.pytest_files


def test_the_outputs_are_flat_strings_the_workflow_can_read():
    out = ci_plan.outputs(plan(["crates/engine-render/src/damage.rs"]))
    assert out["tier"] == "partial" and out["affected_args"] == "-p engine-py -p engine-render"
    assert out["pytest_scope"] == "all" and out["rust"] == "true"
    assert all(isinstance(v, str) and "\n" not in v for v in out.values())


def test_the_workspace_version_is_read_from_cargo_toml():
    assert ci_plan.workspace_version('[workspace.package]\nversion = "1.2.3"\n') == "1.2.3"
    assert ci_plan.workspace_version("[package]\nname = 'x'\n") is None
    assert ci_plan.workspace_version("not toml [") is None
