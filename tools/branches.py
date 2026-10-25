#!/usr/bin/env python3
"""Show the local branches as a chain: which branch sits on which, and what is pushed.

    python3 -I tools/branches.py

Read-only, git only (no network, no `gh`). Each branch hangs under the nearest other branch
whose tip is one of its ancestors, so a stack of branches (each cut from the previous one) reads
top to bottom. For every branch it shows the commits it adds over its parent, and:

    merged    its tip is already in origin/main (safe to delete once the maintainer agrees)
    pushed    in step with its upstream;  "N unpushed" / "no upstream" otherwise
    *         the branch checked out now

`main` is the root. A branch that is only a rebase or squash-merge away from main shows up as
unmerged here, because the check is by commit, not by content.
"""
import subprocess
import sys


def git(*args: str) -> str:
    done = subprocess.run(["git", *args], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def count(rev_range: str) -> int:
    out = git("rev-list", "--count", rev_range)
    return int(out) if out else 0


def is_ancestor(a: str, b: str) -> bool:
    return subprocess.run(["git", "merge-base", "--is-ancestor", a, b]).returncode == 0


def main() -> int:
    if not git("rev-parse", "--git-dir"):
        print("not a git repository", file=sys.stderr)
        return 1
    current = git("branch", "--show-current")
    names = git("for-each-ref", "--format=%(refname:short)", "refs/heads").split()
    if not names:
        print("no local branches")
        return 0
    root = "main" if "main" in names else names[0]
    trunk = "origin/main" if git("rev-parse", "--verify", "-q", "origin/main") else root

    # Parent of a branch = the other branch with the shortest distance below it.
    parent: dict[str, str] = {}
    for b in names:
        if b == root:
            continue
        best, best_dist = root, None
        for c in names:
            if c == b or not is_ancestor(c, b):
                continue
            if is_ancestor(b, c) and git("rev-parse", b) == git("rev-parse", c) and c > b:
                continue  # two branches on one commit: break the tie by name, no cycles
            dist = count(f"{c}..{b}")
            if best_dist is None or dist < best_dist:
                best, best_dist = c, dist
        parent[b] = best

    children: dict[str, list[str]] = {}
    for b, p in parent.items():
        children.setdefault(p, []).append(b)
    for kids in children.values():
        kids.sort()

    def describe(b: str) -> str:
        bits = []
        if b != root:
            bits.append(f"{count(f'{parent[b]}..{b}')} commit(s) over {parent[b]}")
        if is_ancestor(b, trunk):
            bits.append("merged")
        up = git("rev-parse", "--abbrev-ref", f"{b}@{{upstream}}")
        if up:
            unpushed = count(f"{up}..{b}")
            behind = count(f"{b}..{up}")
            state = "pushed" if unpushed == 0 else f"{unpushed} unpushed"
            if behind:
                state += f", {behind} behind {up}"
            bits.append(state)
        else:
            bits.append("no upstream")
        return ", ".join(bits)

    def show(b: str, prefix: str, last: bool, top: bool) -> None:
        mark = "*" if b == current else " "
        branch = "" if top else ("└─ " if last else "├─ ")
        subject = git("log", "-1", "--format=%h %s", b)
        print(f"{prefix}{branch}{mark}{b}  [{describe(b)}]")
        print(f"{prefix}{'' if top else ('   ' if last else '│  ')}   {subject[:90]}")
        kids = children.get(b, [])
        for i, k in enumerate(kids):
            show(k, prefix + ("" if top else ("   " if last else "│  ")), i == len(kids) - 1, False)

    show(root, "", True, True)
    dirty = git("status", "--short")
    if dirty:
        print(f"\nuncommitted changes on {current or '(detached)'}: {len(dirty.splitlines())} path(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
