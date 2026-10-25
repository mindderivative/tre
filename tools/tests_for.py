#!/usr/bin/env python3
"""Which tests cover the files I touched? Prints commands; runs nothing.

    python3 -I tools/tests_for.py                  # files changed since origin/main, plus uncommitted
    python3 -I tools/tests_for.py --base HEAD~3    # a different base
    python3 -I tools/tests_for.py crates/engine-render/src/damage.rs tests/test_layers.py

The standing rule is to run only the tests that cover a change, never the full suite (CI runs
that on the PR). This picks them:

  * a changed test file is itself the test;
  * a changed Rust source file gives `cargo test -p <crate>` filtered to its module when it has
    unit tests, plus the integration tests (Rust and Python) that mention its public names or
    share its file stem;
  * a changed Python module gives the pytest files that mention it.

It is a name match, not coverage data, so it can miss a test that reaches the code indirectly.
Names that appear in more than a quarter of the test files (`new`, `Node`...) are ignored as
noise. Read the "why" column before trusting a pick, and add a test you know is relevant.
"""
import argparse
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOISE_SHARE = 0.25
MAX_PER_FILE = 6

ITEM = re.compile(r"\bpub(?:\([a-z]+\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|const|static|type)\s+([A-Za-z_]\w*)")
PYCLASS = re.compile(r'#\[pyclass[^\]]*?name\s*=\s*"(\w+)"')
PYNAME = re.compile(r'#\[pyo3\(\s*name\s*=\s*"(\w+)"')
PYFN = re.compile(r"#\[(?:pyfunction|pymethods|getter|setter)\][^\n]*\n\s*(?:pub\s+)?(?:fn\s+)?([a-z_]\w*)")


def git(*args: str) -> list[str]:
    done = subprocess.run(["git", *args], capture_output=True, text=True, cwd=ROOT)
    return [l for l in done.stdout.splitlines() if l.strip()] if done.returncode == 0 else []


def changed_files(base: str) -> list[str]:
    if not git("rev-parse", "--verify", "-q", base):
        base = "main"
    files = set(git("diff", "--name-only", f"{base}...HEAD"))
    files |= set(git("diff", "--name-only", "HEAD"))
    files |= set(git("ls-files", "--others", "--exclude-standard"))
    return sorted(f for f in files if (ROOT / f).exists())


def read(path: Path) -> str:
    try:
        return path.read_text(errors="replace")
    except OSError:
        return ""


def names_in(path: Path) -> set[str]:
    """Public names a test could use to reach this file."""
    text = read(path)
    out = set(ITEM.findall(text)) | set(PYCLASS.findall(text)) | set(PYNAME.findall(text))
    if path.suffix == ".py":
        out |= set(re.findall(r"^(?:def|class)\s+([A-Za-z]\w*)", text, re.M))
    return {n for n in out if distinctive(n)}


def distinctive(name: str) -> bool:
    """A name a test only uses when it means this code: a type, a snake_case word, or a long one.
    Short plain words (`close`, `done`, `damage`) show up in prose and comments everywhere."""
    if name[0].isupper():
        return len(name) > 4
    return ("_" in name and len(name) > 6) or len(name) >= 10


def module_filter(src: Path) -> str:
    """`crates/x/src/a/b.rs` -> `a::b`; lib.rs and mod.rs give no filter."""
    rel = src.relative_to(ROOT / "crates" / src.parts[len(ROOT.parts) + 1] / "src").with_suffix("")
    parts = [p for p in rel.parts if p not in ("mod", "lib")]
    return "::".join(parts)


@dataclass
class Plan:
    files: list[str]
    pytest: dict[str, str] = field(default_factory=dict)  # test file -> why
    cargo: dict[str, str] = field(default_factory=dict)  # command -> why
    notes: list[str] = field(default_factory=list)
    crates: set[str] = field(default_factory=set)
    needs_build: bool = False
    unmatched: list[str] = field(default_factory=list)  # Rust sources no test file mentions by name


def plan(files: list[str]) -> Plan:
    """Which tests cover `files` (paths relative to the repo root). Nothing is run."""
    out = Plan(files=list(files))
    py_tests = sorted((ROOT / "tests").glob("test_*.py"))
    rs_tests = sorted(ROOT.glob("crates/*/tests/*.rs"))
    corpus = {t: read(t) for t in py_tests + rs_tests}
    noise_at = max(3, int(len(corpus) * NOISE_SHARE))

    # name -> how many test files mention it, to drop names that match everything
    def mentions(name: str) -> list[Path]:
        pat = re.compile(rf"\b{re.escape(name)}\b")
        return [t for t, text in corpus.items() if pat.search(text)]

    def rank_tests(p: Path) -> list[tuple[Path, set[str]]]:
        """Test files that mention this file's distinctive names or share its stem, best first."""
        hits: dict[Path, set[str]] = defaultdict(set)
        for n in names_in(p):
            found = mentions(n)
            if 0 < len(found) <= noise_at:
                for t in found:
                    hits[t].add(n)
        for t in corpus:  # same stem: layer_window.rs -> test_layer_window*.py
            if p.stem not in ("lib", "__init__") and (t.stem == p.stem or t.stem.startswith(f"test_{p.stem}")):
                hits[t].add(f"file name {p.stem}")
        return sorted(hits.items(), key=lambda kv: (-len(kv[1]), str(kv[0])))[:MAX_PER_FILE]

    def add_pytest(t: Path, why: str) -> None:
        out.pytest.setdefault(str(t.relative_to(ROOT)), why)

    def add_cargo(t: Path, why: str) -> None:
        crate = t.parts[len(ROOT.parts) + 1]
        out.cargo.setdefault(f"cargo test -p {crate} --release --test {t.stem}", why)

    for f in files:
        p = ROOT / f
        parts = Path(f).parts
        if f.startswith("tests/") and p.suffix == ".py":
            if p.name.startswith("test_"):
                add_pytest(p, "changed")
            else:
                for t in py_tests:
                    if re.search(rf"\b{p.stem}\b", corpus[t]):
                        add_pytest(t, f"imports changed helper {p.name}")
            continue
        if len(parts) > 3 and parts[0] == "crates" and parts[2] == "tests" and p.suffix == ".rs":
            add_cargo(p, "changed")
            out.crates.add(parts[1])
            continue
        if len(parts) > 3 and parts[0] == "crates" and parts[2] == "src" and p.suffix == ".rs":
            crate = parts[1]
            out.crates.add(crate)
            text = read(p)
            if "#[cfg(test)]" in text:
                flt = module_filter(p)
                cmd = f"cargo test -p {crate} --release --lib" + (f" {flt}" if flt else "")
                out.cargo.setdefault(cmd, f"unit tests in {f}")
            ranked = rank_tests(p)
            for t, why in ranked:
                reason = f"{f}: " + ", ".join(sorted(why)[:4])
                (add_pytest if t.suffix == ".py" else add_cargo)(t, reason)
            if not ranked:
                out.unmatched.append(f)
                if "#[cfg(test)]" not in text:
                    out.notes.append(f"{f}: no test mentions its names; pick one by hand")
            continue
        if f.startswith("python/tre/") and p.suffix == ".py":
            out.needs_build = True
            ranked = [(t, w) for t, w in rank_tests(p) if t.suffix == ".py"]
            for t, why in ranked:
                add_pytest(t, f"{f}: " + ", ".join(sorted(why)[:4]))
            if not ranked:
                out.unmatched.append(f)
            if p.stem == "__init__":
                out.notes.append(f"{f}: the package root; run the tests that exercise the changed names")
            continue
        if f == "python/tre/_core.pyi":
            out.notes.append(f"{f}: python -m mypy.stubtest tre._core --allowlist tools/stubtest_allowlist.txt && python -m mypy --strict python")
            continue
        if f.startswith("docs/") or f == "mkdocs.yml":
            out.notes.append(f"{f}: python -m mkdocs build --strict")
            continue
        if f.endswith("Cargo.toml") or f == "Cargo.lock" or f == "pyproject.toml":
            out.notes.append(f"{f}: cargo check --workspace --all-targets (and python tools/check_version.py if a version moved)")
            continue
        if f.startswith(("tools/", ".claude/")) or f.endswith((".md", ".sh", ".gitignore")):
            continue  # nothing here is covered by the test suites
        out.notes.append(f"{f}: not mapped to a test")
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("files", nargs="*", help="files to cover (default: changed files)")
    ap.add_argument("--base", default="origin/main", help="diff base (default origin/main)")
    args = ap.parse_args()

    files = [str(Path(f)) for f in args.files] or changed_files(args.base)
    if not files:
        print("no changed files")
        return 0
    found = plan(files)
    crates, cargo_cmds, pytest_files, notes = found.crates, found.cargo, found.pytest, found.notes
    needs_build = found.needs_build

    print(f"changed: {len(files)} file(s)\n")
    if crates:
        print("-- Rust (build + lint the touched crates)")
        for c in sorted(crates):
            print(f"cargo clippy -p {c} --all-targets -- -D warnings")
        print("cargo fmt --check\n")
    if cargo_cmds:
        print("-- Rust tests")
        for cmd, why in cargo_cmds.items():
            print(f"{cmd}    # {why}")
        print()
    if pytest_files:
        print("-- Python tests")
        if needs_build or crates:
            print("python -m maturin develop --release    # the tests below run against the built extension")
        print("python -m pytest " + " ".join(sorted(pytest_files)) + " -q")
        for t, why in sorted(pytest_files.items()):
            print(f"    {t}    # {why}")
        print()
    if notes:
        print("-- also")
        for n in notes:
            print(n)
    if not (cargo_cmds or pytest_files or notes or crates):
        print("none of the changed files is covered by a test suite (tools, scripts and prose only)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
