#!/usr/bin/env python3
"""Re-stamp "0.5.7 (#N)" notes that were written for a release that was cut without them.

    python3 -I tools/restamp_version.py --released v0.5.7 --old 0.5.7 --new 0.5.8          # dry run
    python3 -I tools/restamp_version.py --released v0.5.7 --old 0.5.7 --new 0.5.8 --apply  # write

New code is stamped with the version it is expected to ship in ("0.5.8 (#198): ..."). When that
version has already been released and the work lands after the tag, every such stamp is one version
behind. Only lines added since the tag are looked at, and a line is changed only when it names an
issue (#N) that no stamp at the tag names: a stamp for an issue that really shipped in the release
(#168, #176...) is left alone. A line that carries the old version and no issue number is listed for
a person to decide, never changed, unless --also-unnumbered names its path (then it is changed when
the version appears once on the line). Generated pages (docs/api, site/), LOG.md and graphify-out/ are
skipped; LOG.md and PLAN.md keep their history, and docs/api is regenerated.
"""
import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKIP = ("docs/api/", "site/", "graphify-out/", "target/", "LOG.md", "PLAN.md", ".git/")


def git(*args):
    done = subprocess.run(["git", *args], capture_output=True, text=True, cwd=ROOT)
    if done.returncode != 0:
        sys.exit(f"git {' '.join(args[:2])} failed: {done.stderr.strip()}")
    return done.stdout


def issues(line):
    return {int(n) for n in re.findall(r"#(\d{2,4})\b", line)}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--released", required=True, help="tag of the release the stamps wrongly name")
    ap.add_argument("--old", required=True)
    ap.add_argument("--new", required=True)
    ap.add_argument("--also-unnumbered", nargs="*", default=[], metavar="PATH",
                    help="path prefixes where a line with no issue number is re-stamped too (docs, the stub)")
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()
    old = re.compile(re.escape(args.old) + r"(?![\d.]*\d)")

    # issues the released version really contains: every number stamped with the old version at the tag
    shipped = set()
    for line in git("grep", "-h", "-e", args.old, args.released, "--", ".", ":!docs/api", ":!site").splitlines():
        if old.search(line):
            shipped |= issues(line)

    # added lines since the tag: file -> [(new line number, text)]
    added = defaultdict(list)
    path, number = None, 0
    for row in git("diff", "-U0", "--no-color", args.released, "HEAD").splitlines():
        if row.startswith("+++ "):
            path = row[6:] if row.startswith("+++ b/") else None
        elif row.startswith("@@"):
            number = int(re.search(r"\+(\d+)", row).group(1)) - 1
        elif row.startswith("+") and path:
            number += 1
            if old.search(row[1:]) and not path.startswith(SKIP) and path not in SKIP:
                added[path].append((number, row[1:]))

    changes, manual = defaultdict(list), []
    for path, rows in sorted(added.items()):
        for number, text in rows:
            named = issues(text)
            if named and not (named & shipped):
                changes[path].append(number)
            elif named:
                manual.append((path, number, text, "names an issue that shipped in " + args.released))
            elif path.startswith(tuple(args.also_unnumbered)):
                if len(old.findall(text)) > 1:
                    manual.append((path, number, text, "carries the version twice: decide by hand"))
                else:
                    changes[path].append(number)
            else:
                manual.append((path, number, text, "names no issue"))

    total = sum(len(v) for v in changes.values())
    print(f"{total} line(s) in {len(changes)} file(s) would change {args.old} -> {args.new}")
    for path, numbers in changes.items():
        print(f"  {path}: {len(numbers)}")
    if manual:
        print(f"\n{len(manual)} line(s) carry {args.old} and were left alone:")
        for path, number, text, why in manual:
            print(f"  {path}:{number}: {text.strip()[:110]}   [{why}]")
    if not args.apply:
        print("\ndry run: pass --apply to write")
        return 0
    for path, numbers in changes.items():
        file = ROOT / path
        lines = file.read_text().split("\n")
        for n in numbers:
            lines[n - 1] = old.sub(args.new, lines[n - 1])
        file.write_text("\n".join(lines))
    print(f"\nwrote {total} line(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
