#!/usr/bin/env python3
"""Decide which CI tier a change needs, and what the partial tier has to cover.

    python3 tools/ci_plan.py --base <sha> [--head HEAD] [--event pull_request] [--labels a,b]
    python3 tools/ci_plan.py --base origin/main              # try it on a branch; prints the plan
    python3 tools/ci_plan.py --event workflow_dispatch --tier full

Three tiers (`.github/workflows/ci.yml` runs the one this picks):

  docs     only documentation changed (docs/, *.md, mkdocs.yml, .claude/, ...): build the site with
           `mkdocs build --strict`, check the generated Python API page and the version and
           agent-doc consistency checks. No Rust, no Python tests.
  partial  code changed: the standard run. Builds and tests what the change reaches: the changed
           crates and every crate that depends on them (dev-dependencies included), clippy for the
           same, the Python tests that cover the change (see below), and the examples that use what
           changed (they name it, or share a word with the file's name) plus three broad ones.
           Linux only.
  full     a release (the workspace version in Cargo.toml moved), a build-config change that can
           break another platform (Cargo.lock, rust-toolchain.toml, the root Cargo.toml, the CI
           workflow itself), the `full-ci` label, or a manual run: everything, on Linux, Windows and
           macOS, plus the MSRV check, every example and the docs build. The docs tier runs no
           examples.

Which Python tests a partial run takes. The crates are layered (engine-core is under engine-render
and engine-platform, which are under engine-py, the Python surface). A change to a lower crate reaches
Python indirectly and its Rust names do not appear in the Python tests, so it runs the whole pytest
suite. A change to engine-py or python/ runs the tests that name what changed (tools/tests_for.py) plus
a small smoke set; if a changed file is named by no test, or the picks are more than 40% of the suite,
it runs the whole suite. Name matching is not coverage data: when in doubt the plan runs more, and a
PR can ask for everything with the `full-ci` label.

This script reads git and the crates' Cargo.toml files only; it builds and runs nothing. With
`--github-output FILE` it writes the plan as `key=value` lines, and with `--summary FILE` a markdown
summary, for the workflow.
"""
import argparse
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

# Files that are documentation, in the sense that no test suite covers them.
DOCS_PREFIXES = ("docs/", ".claude/", ".github/ISSUE_TEMPLATE/")
DOCS_FILES = {"mkdocs.yml", ".gitignore", ".github/workflows/docs.yml", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE"}
DOCS_SUFFIXES = (".md",)

# Changes that can break a platform or the toolchain without touching a line of Rust: the full tier.
FULL_FILES = {"Cargo.lock", "rust-toolchain.toml", "Cargo.toml", ".github/workflows/ci.yml"}

# Always taken with a partial run's Python tests: broad, quick, and not tied to one feature.
SMOKE_TESTS = [
    "tests/test_engine_py.py",
    "tests/test_kinds.py",
    "tests/test_node_props.py",
    "tests/test_structure.py",
    "tests/test_snapshot.py",
]
# Always taken, with the examples that relate to the change, when engine or Python code changed:
# broad scripts that exercise layout, animation and most node kinds.
SMOKE_EXAMPLES = ["examples/showcase.py", "examples/layout.py", "examples/animation.py"]
ALL_IF_OVER = 0.40  # a partial pick bigger than this share of the suite runs the whole suite

# The crates under engine-py: a change to one reaches Python only indirectly.
LOWER_CRATES = {"engine-core", "engine-render", "engine-platform"}
PY_CRATE = "engine-py"


@dataclass
class Selection:
    """What `select_tests` found for a set of changed files."""
    tests: list[str]
    unmatched: list[str]  # Rust or Python sources no test names
    suite: int  # how many test files there are
    examples: list[str] = field(default_factory=list)  # examples that use what changed


@dataclass
class Plan:
    tier: str
    reason: str
    files: list[str] = field(default_factory=list)
    crates: list[str] = field(default_factory=list)  # changed
    affected: list[str] = field(default_factory=list)  # changed + everything that depends on them
    run_docs: bool = False
    rust: bool = False
    build_ext: bool = False
    typecheck: bool = False
    pytest_scope: str = "none"  # none | selected | all
    pytest_files: list[str] = field(default_factory=list)
    examples: list[str] = field(default_factory=list)
    why_pytest: str = ""


def git(*args: str) -> str:
    done = subprocess.run(["git", *args], capture_output=True, text=True, cwd=ROOT)
    return done.stdout if done.returncode == 0 else ""


def changed_files(base: str, head: str) -> list[str] | None:
    """Paths that differ between base and head, or None when git cannot say."""
    if not base or set(base) == {"0"} or not git("rev-parse", "--verify", "-q", f"{base}^{{commit}}").strip():
        return None
    out = subprocess.run(["git", "diff", "--name-only", base, head], capture_output=True, text=True, cwd=ROOT)
    if out.returncode != 0:
        return None
    return sorted(line for line in out.stdout.splitlines() if line.strip())


def workspace_version(text: str) -> str | None:
    try:
        return tomllib.loads(text)["workspace"]["package"]["version"]
    except (KeyError, tomllib.TOMLDecodeError):
        return None


def version_moved(base: str, head: str) -> bool:
    before = workspace_version(git("show", f"{base}:Cargo.toml"))
    after = workspace_version(git("show", f"{head}:Cargo.toml") if head != "HEAD" else (ROOT / "Cargo.toml").read_text())
    return before is not None and after is not None and before != after


def crate_graph(root: Path = ROOT) -> dict[str, set[str]]:
    """`crate -> the workspace crates it depends on`, dev- and build-dependencies included."""
    manifests = {}
    for path in sorted(root.glob("crates/*/Cargo.toml")):
        data = tomllib.loads(path.read_text())
        manifests[data["package"]["name"]] = data
    graph = {}
    for name, data in manifests.items():
        deps: set[str] = set()
        for table in ("dependencies", "dev-dependencies", "build-dependencies"):
            deps |= {d for d in data.get(table, {}) if d in manifests}
        graph[name] = deps
    return graph


def affected_crates(changed: set[str], graph: dict[str, set[str]]) -> list[str]:
    """The changed crates and every crate that (transitively) depends on one of them."""
    out = set(changed)
    grew = True
    while grew:
        grew = False
        for name, deps in graph.items():
            if name not in out and deps & out:
                out.add(name)
                grew = True
    return sorted(out)


def is_docs(path: str) -> bool:
    return path.startswith(DOCS_PREFIXES) or path in DOCS_FILES or path.endswith(DOCS_SUFFIXES)


def crate_of(path: str) -> str | None:
    parts = path.split("/")
    return parts[1] if len(parts) > 2 and parts[0] == "crates" else None


def decide(
    files: list[str] | None,
    *,
    version_changed: bool = False,
    forced: str | None = None,
    labels: tuple[str, ...] = (),
    graph: dict[str, set[str]] | None = None,
    select_tests=None,
) -> Plan:
    """The pure part: from the changed paths (None = unknown) to a plan."""
    if forced:
        return Plan(forced, f"asked for by hand: {forced}", run_docs=True, rust=True, build_ext=True)
    if files is None:
        return Plan("full", "cannot tell what changed (no base to compare with)", run_docs=True, rust=True)
    if "full-ci" in labels:
        return Plan("full", "the full-ci label is on the PR", files, run_docs=True, rust=True)
    if version_changed:
        return Plan("full", "the workspace version moved: a release", files, run_docs=True, rust=True)
    shared = sorted(f for f in files if f in FULL_FILES)
    if shared:
        return Plan("full", "build configuration changed: " + ", ".join(shared), files, run_docs=True, rust=True)
    if all(is_docs(f) for f in files):
        why = "documentation only" if files else "no files changed"
        return Plan("docs", why, files, run_docs=True)

    graph = graph if graph is not None else crate_graph()
    crates = sorted({c for f in files if (c := crate_of(f)) and c in graph})
    affected = affected_crates(set(crates), graph)
    plan = Plan("partial", "code changed", files, crates, affected)
    plan.run_docs = any(f.startswith("docs/") or f == "mkdocs.yml" for f in files)
    plan.rust = bool(crates)
    py_changed = [f for f in files if f.startswith("python/") or f in ("pyproject.toml",)]
    test_changed = [f for f in files if f.startswith("tests/")]
    plan.examples = sorted(f for f in files if f.startswith("examples/") and f.endswith(".py"))
    code_changed = bool(crates) or bool(py_changed)
    picked = select_tests(files) if select_tests and (code_changed or test_changed or plan.examples) else None
    if picked is not None and code_changed:
        plan.examples = sorted(set(plan.examples) | set(picked.examples) | set(SMOKE_EXAMPLES))
    plan.examples = [e for e in plan.examples if (ROOT / e).exists()]  # not one the PR deleted
    plan.typecheck = bool(py_changed) or PY_CRATE in crates
    plan.build_ext = bool(PY_CRATE in affected or py_changed or test_changed)

    # A script in tools/ is covered by tests/test_<script>.py when there is one.
    tool_tests = [f for f in files if f.startswith("tools/") and f.endswith(".py") and (ROOT / "tests" / f"test_{Path(f).stem}.py").exists()]
    plan.build_ext = plan.build_ext or bool(tool_tests)

    lower = sorted(set(crates) & LOWER_CRATES)
    if lower:
        plan.pytest_scope = "all"
        plan.why_pytest = f"{', '.join(lower)} sits under engine-py, so its effects reach Python indirectly"
    elif PY_CRATE in crates or py_changed or test_changed or tool_tests:
        if picked is None and select_tests:
            picked = select_tests(files)
        if picked is None:
            plan.pytest_scope, plan.why_pytest = "all", "the tests could not be selected"
        else:
            if picked.unmatched:
                plan.pytest_scope = "all"
                plan.why_pytest = "no test names " + ", ".join(picked.unmatched[:3]) + (" ..." if len(picked.unmatched) > 3 else "")
            else:
                chosen = sorted(set(picked.tests) | set(SMOKE_TESTS))
                if len(chosen) > ALL_IF_OVER * picked.suite:
                    plan.pytest_scope = "all"
                    plan.why_pytest = f"{len(chosen)} of {picked.suite} test files picked"
                else:
                    plan.pytest_scope = "selected"
                    plan.pytest_files = chosen
                    plan.why_pytest = f"{len(chosen)} of {picked.suite} test files: those that name what changed, plus the smoke set"
    # Anything that runs Python needs the built extension.
    plan.build_ext = plan.build_ext or plan.pytest_scope != "none" or bool(plan.examples)
    return plan


def select_with_tests_for(files: list[str]):
    """A `Selection` from tools/tests_for.py: test files, sources no test names, related examples."""
    try:
        import tests_for
    except Exception:
        return None
    found = tests_for.plan([f for f in files if (ROOT / f).exists()])
    suite = len(list((ROOT / "tests").glob("test_*.py")))
    tests = set(found.pytest)
    for f in files:
        mine = ROOT / "tests" / f"test_{Path(f).stem}.py"
        if f.startswith("tools/") and f.endswith(".py") and mine.exists():
            tests.add(str(mine.relative_to(ROOT)))
    return Selection(sorted(tests), list(found.unmatched), suite, sorted(found.examples))


def render(plan: Plan) -> str:
    lines = [f"tier: {plan.tier}", f"why: {plan.reason}", f"changed files: {len(plan.files)}"]
    if plan.tier == "partial":
        lines += [
            f"changed crates: {' '.join(plan.crates) or '(none)'}",
            f"affected crates: {' '.join(plan.affected) or '(none)'}",
            f"python tests: {plan.pytest_scope}" + (f" ({plan.why_pytest})" if plan.why_pytest else ""),
        ]
        if plan.pytest_files:
            lines.append("  " + " ".join(plan.pytest_files))
        lines.append(f"mypy + stubtest: {plan.typecheck}; docs build: {plan.run_docs}")
        lines.append(f"examples ({len(plan.examples)}): {' '.join(plan.examples) or 'none'}")
    return "\n".join(lines)


def outputs(plan: Plan) -> dict[str, str]:
    """The plan as the flat key=value pairs the workflow reads."""
    flag = lambda b: "true" if b else "false"  # noqa: E731
    return {
        "tier": plan.tier,
        "reason": plan.reason,
        "run_docs": flag(plan.run_docs),
        "rust": flag(plan.rust),
        "affected_args": " ".join(f"-p {c}" for c in plan.affected),
        "build_ext": flag(plan.build_ext),
        "typecheck": flag(plan.typecheck),
        "pytest_scope": plan.pytest_scope,
        "pytest_files": " ".join(plan.pytest_files),
        "examples": " ".join(plan.examples),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="", help="the commit to compare with (PR base, or the push's before)")
    ap.add_argument("--head", default="HEAD")
    ap.add_argument("--event", default="pull_request")
    ap.add_argument("--tier", default="", help="with workflow_dispatch: docs or full (default full)")
    ap.add_argument("--labels", default="", help="comma-separated PR labels")
    ap.add_argument("--github-output", metavar="FILE")
    ap.add_argument("--summary", metavar="FILE")
    args = ap.parse_args()

    forced = None
    if args.event == "workflow_dispatch":
        forced = args.tier or "full"
        if forced not in ("docs", "full"):
            sys.exit(f"--tier must be docs or full, not {forced!r}")
    files = None if forced else changed_files(args.base, args.head)
    moved = bool(files is not None and not forced and version_moved(args.base, args.head))
    plan = decide(
        files,
        version_changed=moved,
        forced=forced,
        labels=tuple(x for x in args.labels.split(",") if x),
        select_tests=select_with_tests_for,
    )
    text = render(plan)
    print(text)
    if args.github_output:
        with open(args.github_output, "a") as handle:
            for key, value in outputs(plan).items():
                handle.write(f"{key}={value}\n")
    if args.summary:
        with open(args.summary, "a") as handle:
            handle.write("### CI plan\n\n```\n" + text + "\n```\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
