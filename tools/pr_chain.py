#!/usr/bin/env python3
"""The push / PR / CI / merge routine for a stack of branches. Changes nothing unless told to.

    python3 -I tools/pr_chain.py status                   # branches, their PRs, CI, and the next step
    python3 -I tools/pr_chain.py push BRANCH... --yes     # git push -u origin
    python3 -I tools/pr_chain.py open BRANCH --yes        # open a PR (base = the branch it sits on)
    python3 -I tools/pr_chain.py retarget N --yes         # base -> main, then close+reopen so CI runs
    python3 -I tools/pr_chain.py wait N [--timeout 1800]  # block until N's checks finish
    python3 -I tools/pr_chain.py merge N --yes            # merge commit, only if the checks are green
    python3 -I tools/pr_chain.py cleanup --yes            # delete merged branches, local and origin

Every command that writes (push, open, retarget, merge, cleanup) prints what it would do and stops
unless --yes is given. AGENTS.md: never push without an explicit instruction in chat, merge only
on an explicit "merge", so --yes is for after the user said so; it is not a convenience. Merging
also refuses when the checks are not all green, and never uses --admin or auto-merge (auto-merge is
off on the repo). Needs `gh` logged in.

Why the retarget step exists: ci.yml runs only on pull_request events targeting main. A PR based on
another branch gets no CI, and changing its base later does not start it, so a close and reopen
does. PRs in a stack are merged one at a time, the bottom one first.
"""
import argparse
import json
import subprocess
import sys
import time

REPO = "mindderivative/tre"
TRUNK = "main"


def run(*cmd, check=True):
    done = subprocess.run(cmd, capture_output=True, text=True)
    if check and done.returncode != 0:
        sys.exit(f"{' '.join(cmd[:3])} failed: {(done.stderr or done.stdout).strip()}")
    return done


def git(*args):
    return run("git", *args).stdout.strip()


def gh_json(*args):
    return json.loads(run("gh", *args).stdout or "null")


def local_branches():
    return git("for-each-ref", "--format=%(refname:short)", "refs/heads").split()


def is_ancestor(a, b):
    return subprocess.run(["git", "merge-base", "--is-ancestor", a, b]).returncode == 0


def count(rng):
    return int(git("rev-list", "--count", rng) or 0)


def parent_of(branch, names):
    """The nearest other local branch below `branch`, else the trunk."""
    best, dist = TRUNK, None
    for c in names:
        if c == branch or not is_ancestor(c, branch):
            continue
        if git("rev-parse", c) == git("rev-parse", branch):
            continue
        d = count(f"{c}..{branch}")
        if dist is None or d < dist:
            best, dist = c, d
    return best


def prs():
    return gh_json("pr", "list", "-R", REPO, "--state", "all", "--limit", "100", "--json",
                   "number,title,state,headRefName,baseRefName,url,mergeable,statusCheckRollup")


def checks_state(pr):
    """green / red / pending / none for a PR's check rollup."""
    rolled = pr.get("statusCheckRollup") or []
    if not rolled:
        return "none"
    states = []
    for c in rolled:
        if c.get("status") and c["status"] != "COMPLETED":
            states.append("pending")
        else:
            states.append({"SUCCESS": "green", "NEUTRAL": "green", "SKIPPED": "green"}.get(
                c.get("conclusion") or c.get("state") or "", "red"))
    return "red" if "red" in states else "pending" if "pending" in states else "green"


def pr_for(number):
    for p in prs():
        if p["number"] == number:
            return p
    sys.exit(f"no PR #{number} on {REPO}")


def confirm(what, yes):
    print(("DO   " if yes else "PLAN ") + what)
    if not yes:
        print("     (dry run: pass --yes once the user has said to)")
    return yes


def status():
    names = local_branches()
    by_head = {}
    for p in prs():
        by_head.setdefault(p["headRefName"], []).append(p)
    run("git", "fetch", "-q", "origin", check=False)
    trunk = f"origin/{TRUNK}"
    rows = 0
    for b in sorted(names):
        if b == TRUNK:
            continue
        merged = is_ancestor(b, trunk)
        up = run("git", "rev-parse", "--abbrev-ref", f"{b}@{{upstream}}", check=False).stdout.strip()
        unpushed = count(f"{up}..{b}") if up else count(f"{trunk}..{b}")
        parent = parent_of(b, names)
        mine = by_head.get(b, [])
        open_pr = next((p for p in mine if p["state"] == "OPEN"), None)
        merged_pr = next((p for p in mine if p["state"] == "MERGED"), None)
        line = f"{b}  on {parent}"
        if merged or merged_pr:
            nxt = "delete it (cleanup)"
            line += "  [merged]"
        elif open_pr:
            ci = checks_state(open_pr)
            base_ok = open_pr["baseRefName"] == TRUNK
            line += f"  [PR #{open_pr['number']} base {open_pr['baseRefName']}, CI {ci}]"
            if unpushed:
                nxt = f"push {unpushed} new commit(s)"
            elif not base_ok:
                nxt = (f"wait until {open_pr['baseRefName']} merges, then retarget #{open_pr['number']}"
                       if parent != TRUNK else f"retarget #{open_pr['number']} to {TRUNK}")
            elif ci == "green":
                nxt = f"ready to merge #{open_pr['number']} (needs the user's \"merge\")"
            elif ci == "pending":
                nxt = f"wait #{open_pr['number']}"
            elif ci == "none":
                nxt = f"no CI yet: close and reopen #{open_pr['number']}"
            else:
                nxt = f"CI red on #{open_pr['number']}: read the failure"
        elif not up:
            line += "  [not pushed]"
            nxt = "push, then open a PR (needs the user's instruction)"
        elif unpushed:
            line += f"  [{unpushed} unpushed, no PR]"
            nxt = "push, then open a PR"
        else:
            line += "  [pushed, no PR]"
            nxt = "open a PR"
        print(f"{line}\n    next: {nxt}")
        rows += 1
    if not rows:
        print("no branches besides main")
    return 0


def push(branches, yes):
    for b in branches:
        if b not in local_branches():
            sys.exit(f"no local branch {b}")
    for b in branches:
        if confirm(f"git push -u origin {b}   ({count(f'origin/{TRUNK}..{b}')} commit(s) over {TRUNK})", yes):
            run("git", "push", "-u", "origin", b)
    return 0


def open_pr(branch, base, title, body, yes):
    names = local_branches()
    if branch not in names:
        sys.exit(f"no local branch {branch}")
    if not run("git", "rev-parse", "--abbrev-ref", f"{branch}@{{upstream}}", check=False).stdout.strip():
        sys.exit(f"{branch} is not pushed; push it first")
    base = base or parent_of(branch, names)
    title = title or git("log", "-1", "--format=%s", branch)
    if body is None:
        body = "\n".join(f"- {l}" for l in git("log", "--format=%s", f"{base}..{branch}").splitlines())
    if base != TRUNK:
        print(f"note: base {base} is not {TRUNK}, so CI will not run until it is retargeted")
    if confirm(f"gh pr create -R {REPO} --head {branch} --base {base} --title {title!r}", yes):
        print(run("gh", "pr", "create", "-R", REPO, "--head", branch, "--base", base,
                  "--title", title, "--body", body).stdout.strip())
    return 0


def retarget(number, yes):
    p = pr_for(number)
    if p["state"] != "OPEN":
        sys.exit(f"#{number} is {p['state']}")
    if p["baseRefName"] == TRUNK and checks_state(p) != "none":
        print(f"#{number} already targets {TRUNK} and has CI ({checks_state(p)}); nothing to do")
        return 0
    if confirm(f"#{number}: base {p['baseRefName']} -> {TRUNK}, then close and reopen to start CI", yes):
        if p["baseRefName"] != TRUNK:
            run("gh", "pr", "edit", str(number), "-R", REPO, "--base", TRUNK)
        run("gh", "pr", "close", str(number), "-R", REPO)
        time.sleep(2)
        run("gh", "pr", "reopen", str(number), "-R", REPO)
        print(f"#{number} reopened; `wait {number}` follows the checks")
    return 0


def wait(number, timeout):
    end = time.time() + timeout
    last = None
    while True:
        state = checks_state(pr_for(number))
        if state != last:
            print(f"#{number} CI: {state}", flush=True)
            last = state
        if state in ("green", "red"):
            return 0 if state == "green" else 1
        if state == "none" and time.time() > end - timeout + 120:
            print("no checks appeared after 2 minutes: is the base main? (retarget)")
            return 2
        if time.time() > end:
            print("timed out")
            return 3
        time.sleep(20)


def merge(number, yes):
    p = pr_for(number)
    if p["state"] != "OPEN":
        sys.exit(f"#{number} is {p['state']}")
    if p["baseRefName"] != TRUNK:
        sys.exit(f"#{number} targets {p['baseRefName']}, not {TRUNK}: merge the one below it first, then retarget")
    ci = checks_state(p)
    if ci != "green":
        sys.exit(f"#{number} checks are {ci}, not green: not merging")
    if p.get("mergeable") == "CONFLICTING":
        sys.exit(f"#{number} has conflicts")
    if confirm(f"gh pr merge {number} --merge   ({p['title']})", yes):
        run("gh", "pr", "merge", str(number), "-R", REPO, "--merge")
        print(f"#{number} merged")
    return 0


def cleanup(yes):
    run("git", "fetch", "-q", "--prune", "origin", check=False)
    current = git("branch", "--show-current")
    todo = [b for b in local_branches()
            if b not in (TRUNK, current) and is_ancestor(b, f"origin/{TRUNK}")]
    if not todo:
        print("no merged branches to delete")
        return 0
    for b in todo:
        remote = run("git", "ls-remote", "--exit-code", "--heads", "origin", b, check=False).returncode == 0
        if confirm(f"delete {b} (merged into {TRUNK}) locally" + (" and on origin" if remote else ""), yes):
            run("git", "branch", "-d", b)
            if remote:
                run("git", "push", "origin", "--delete", b)
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status")
    for name in ("push", "open", "retarget", "merge", "cleanup"):
        s = sub.add_parser(name)
        s.add_argument("--yes", action="store_true", help="really do it (after the user said so)")
        if name == "push":
            s.add_argument("branches", nargs="+")
        elif name == "open":
            s.add_argument("branch")
            s.add_argument("--base")
            s.add_argument("--title")
            s.add_argument("--body")
        elif name in ("retarget", "merge"):
            s.add_argument("number", type=int)
    w = sub.add_parser("wait")
    w.add_argument("number", type=int)
    w.add_argument("--timeout", type=int, default=1800)
    a = ap.parse_args(argv)
    return {
        "status": lambda: status(),
        "push": lambda: push(a.branches, a.yes),
        "open": lambda: open_pr(a.branch, a.base, a.title, a.body, a.yes),
        "retarget": lambda: retarget(a.number, a.yes),
        "wait": lambda: wait(a.number, a.timeout),
        "merge": lambda: merge(a.number, a.yes),
        "cleanup": lambda: cleanup(a.yes),
    }[a.cmd]()


if __name__ == "__main__":
    sys.exit(main())
