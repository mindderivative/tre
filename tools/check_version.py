#!/usr/bin/env python3
"""Check that every place the tre version lives agrees, before a release.

The version is in `Cargo.toml` ([workspace.package]), `pyproject.toml`, and the four `engine-*`
entries of `Cargo.lock`; each crate must inherit it (`version.workspace = true`).

    python3 tools/check_version.py                    # all places agree
    python3 tools/check_version.py --expect 0.5.8     # ... and equal the release you are cutting
    python3 tools/check_version.py --pypi             # ... and PyPI does not have it yet (network)

PyPI never accepts the same version twice, so --pypi is the pre-flight for a publish. With --pypi
and no --expect, a version PyPI already has is an error (the usual state right after a release).
Exit status 1 on any problem.
"""
import argparse
import json
import re
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

DIST = "tesserae-engine"
CRATES = ("engine-core", "engine-platform", "engine-py", "engine-render")


def found_versions(root):
    """Return ({label: version}, [problems]) for every place the version is kept."""
    found, problems = {}, []
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    found["Cargo.toml [workspace.package]"] = cargo["workspace"]["package"]["version"]
    found["pyproject.toml"] = tomllib.loads((root / "pyproject.toml").read_text())["project"]["version"]
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    locked = {p["name"]: p["version"] for p in lock["package"] if p["name"] in CRATES}
    for name in CRATES:
        if name in locked:
            found[f"Cargo.lock {name}"] = locked[name]
        else:
            problems.append(f"Cargo.lock has no {name} entry")
    for name in CRATES:
        manifest = tomllib.loads((root / "crates" / name / "Cargo.toml").read_text())
        if manifest["package"].get("version") != {"workspace": True}:
            problems.append(f"crates/{name}/Cargo.toml does not use `version.workspace = true`")
    return found, problems


def on_pypi(version):
    url = f"https://pypi.org/pypi/{DIST}/json"
    try:
        with urllib.request.urlopen(url, timeout=15) as reply:
            return version in json.load(reply)["releases"]
    except urllib.error.HTTPError as err:
        if err.code == 404:
            return False
        raise


def check(root, expect=None, pypi=False, published=on_pypi):
    found, problems = found_versions(root)
    distinct = set(found.values())
    if len(distinct) > 1:
        problems.append("versions disagree: " + ", ".join(f"{k}={v}" for k, v in found.items()))
    version = next(iter(distinct)) if len(distinct) == 1 else None
    if version and not re.fullmatch(r"\d+\.\d+\.\d+(\.\d+)?", version):
        problems.append(f"{version!r} is not a plain x.y.z[.w] version")
    if expect and version != expect:
        problems.append(f"expected {expect}, the files say {version}")
    if pypi and version and published(expect or version):
        problems.append(f"{DIST} {expect or version} is already on PyPI (it never accepts a version twice)")
    return version, problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--expect")
    parser.add_argument("--pypi", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    version, problems = check(args.root, args.expect, args.pypi)
    for problem in problems:
        print("problem:", problem)
    if not problems:
        print(f"ok: {version} everywhere" + (" and free on PyPI" if args.pypi else ""))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
