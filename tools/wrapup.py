#!/usr/bin/env python3
"""What still has to be written down after a piece of work? A checklist, not an editor.

    python3 -I tools/wrapup.py            # run the checks
    python3 -I tools/wrapup.py --no-board # skip the GitHub board (offline)

AGENTS.md asks for documentation, LOG.md, the board and the memory entity to be current as soon as
a milestone, step or feature is finished. This reads git, LOG.md and the board and lists what looks
behind, so nothing is left to remembering:

  * commits since LOG.md last changed, and the issue numbers they mention that LOG.md does not;
  * code changes under engine-py or python/ with no docs/ change since the same point;
  * cards In progress / In review whose issue LOG.md never mentions;
  * uncommitted work, unpushed work, a stale version, a stale code graph.

It also prints a draft line for the one `tre-v2-gui-framework` memory entity. The memory server is
not reachable from a script, so that last step stays by hand: update the entity in place (decisions,
lessons, status), not one entry per commit. Exit status 1 if anything looks stale, so it can gate.
"""
import argparse
import json
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sh(*cmd):
    done = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    return done.stdout.rstrip("\n") if done.returncode == 0 else ""


def issue_numbers(text):
    return {int(n) for n in re.findall(r"#(\d{2,4})\b", text)}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--no-board", action="store_true", help="skip the GitHub board")
    args = ap.parse_args()

    stale = []
    ok = []

    def check(passed, good, bad):
        (ok if passed else stale).append(good if passed else bad)

    log = (ROOT / "LOG.md").read_text()
    log_numbers = issue_numbers(log)
    since = sh("git", "log", "-1", "--format=%H", "--", "LOG.md")
    rng = f"{since}..HEAD" if since else "HEAD"
    commits = sh("git", "log", "--format=%h %s", rng).splitlines()
    behind = (f"{len(commits)} commit(s) since LOG.md last changed, newest: {commits[0].split(' ', 1)[-1][:70]}"
              if commits else "")
    check(not commits, "LOG.md was edited in the newest commit", behind)

    # issues named in this branch's unmerged commits (or since LOG.md) that LOG.md does not mention
    trunk = "origin/main" if sh("git", "rev-parse", "--verify", "-q", "origin/main") else "main"
    mine = sh("git", "log", "--format=%s%n%b", f"{trunk}..HEAD")
    branch = sh("git", "branch", "--show-current")
    named = issue_numbers(mine) | issue_numbers(branch)
    missing = sorted(n for n in named if n not in log_numbers)
    check(not missing, "every issue in this branch's commits is in LOG.md",
          "issues in this branch's commits that LOG.md never names: " + ", ".join(f"#{n}" for n in missing))

    # code that users see changed, docs did not
    changed = set(sh("git", "diff", "--name-only", f"{trunk}...HEAD").splitlines())
    changed |= set(sh("git", "diff", "--name-only", "HEAD").splitlines())
    api = sorted(f for f in changed if f.startswith(("crates/engine-py/src/", "python/tre/")) and f.endswith((".rs", ".py", ".pyi")))
    docs = sorted(f for f in changed if f.startswith("docs/") or f in ("README.md", "CHANGELOG.md"))
    check(not api or docs, "no Python-facing code changed without docs",
          f"{len(api)} Python-facing file(s) changed and no docs/ file did (e.g. {api[0] if api else ''})")
    if "python/tre/_core.pyi" in changed:
        # the page is generated from the stub; regenerate (idempotent) and see whether it moved
        py = ROOT / ".venv/bin/python"
        subprocess.run([str(py) if py.exists() else sys.executable, "tools/gen_python_api.py"],
                       capture_output=True, cwd=ROOT)
        moved = bool(sh("git", "status", "--short", "docs/api/python.md"))
        check(not moved, "docs/api/python.md matches the stub",
              "docs/api/python.md was out of date with the stub and has been regenerated: commit it")

    dirty = sh("git", "status", "--short").splitlines()
    check(not dirty, "working tree clean", f"{len(dirty)} uncommitted path(s): " + ", ".join(l[3:] for l in dirty[:4]))
    up = sh("git", "rev-parse", "--abbrev-ref", "@{upstream}")
    unpushed = int(sh("git", "rev-list", "--count", f"{up}..HEAD") or 0) if up else int(sh("git", "rev-list", "--count", f"{trunk}..HEAD") or 0)
    check(unpushed == 0, "everything pushed", f"{unpushed} commit(s) not pushed (only the user says when to push)")

    version = subprocess.run([sys.executable, "-I", "tools/check_version.py"], capture_output=True, text=True, cwd=ROOT)
    check(version.returncode == 0, version.stdout.strip() or "version agrees", "version: " + (version.stdout + version.stderr).strip()[:150])
    graph = subprocess.run(["tools/graphify_refresh.sh", "--check"], capture_output=True, text=True, cwd=ROOT).stdout.strip()
    if graph:
        check(graph.startswith("graph is current"), graph, graph)

    cards = []
    if not args.no_board:
        done = subprocess.run([sys.executable, "-I", "tools/board.py", "status"], capture_output=True, text=True, cwd=ROOT)
        if done.returncode != 0:
            stale.append("board: could not read it (" + done.stderr.strip()[:80] + "); pass --no-board offline")
        else:
            column = None
            for line in done.stdout.splitlines():
                m = re.match(r"^(\w[\w ]*) \(\d+\)$", line)
                if m:
                    column = m.group(1)
                    continue
                m = re.match(r"^\s+(?:PR )?#(\d+)\s+(.*)", line)
                if m and column in ("In progress", "In review"):
                    cards.append((column, int(m.group(1)), m.group(2)))
            for column, n, title in cards:
                check(n in log_numbers, f"#{n} ({column}) is in LOG.md",
                      f"#{n} is {column} on the board but LOG.md never names it: {title[:60]}")

    print("OK")
    for line in ok:
        print(f"  + {line}")
    print("\nBEHIND" if stale else "\nnothing behind")
    for line in stale:
        print(f"  - {line}")

    print("\nBy hand (cannot be checked from here):")
    print("  * board: move the card and leave a comment with findings, errors and fixes")
    print("  * memory: update the one `tre-v2-gui-framework` entity in place; draft status line:")
    subjects = [c.split(" ", 1)[1] for c in commits[:3]] or [sh("git", "log", "-1", "--format=%s")]
    nums = ", ".join(f"#{n}" for n in sorted(named)) or "(no issue)"
    print(f"      {date.today()}: {nums} on {branch or 'detached HEAD'}: {subjects[0][:90]}"
          f"{' (+' + str(len(commits) - 1) + ' more)' if len(commits) > 1 else ''}; "
          f"{'pushed' if unpushed == 0 else str(unpushed) + ' unpushed'}.")
    print("      add decisions and lessons only if there were some")
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main())
