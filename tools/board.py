#!/usr/bin/env python3
"""Read and move cards on the GitHub project board "Tesserae Rendering Engine" (project 3).

    python3 tools/board.py status [--all]          # open cards by column (--all adds Done)
    python3 tools/board.py move N COLUMN [--comment TEXT] [--approved] [--dry-run]

COLUMN is Backlog, Ready, "In progress", "In review" or Done (case does not matter). Done is the
user's to approve, so moving to Done needs --approved. --comment posts TEXT on issue or PR N.
Needs `gh` logged in with the `project` scope.
"""
import argparse
import json
import subprocess
import sys

OWNER = "mindderivative"
PROJECT = "3"


def gh(*args):
    done = subprocess.run(["gh", *args], capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"gh {' '.join(args[:2])} failed: {done.stderr.strip()}")
    return done.stdout


def project_json(*args):
    return json.loads(gh("project", *args, PROJECT, "--owner", OWNER, "--format", "json"))


def columns():
    for field in project_json("field-list")["fields"]:
        if field["name"] == "Status":
            return field["id"], {o["name"].lower(): o for o in field["options"]}
    sys.exit("the project has no Status field")


def items():
    return project_json("item-list", "--limit", "500")["items"]


def status(show_done):
    _, options = columns()
    order = [o["name"] for o in options.values()]
    cards = items()
    for column in order:
        if column == "Done" and not show_done:
            continue
        rows = [c for c in cards if c.get("status") == column]
        print(f"\n{column} ({len(rows)})")
        for c in sorted(rows, key=lambda c: c["content"].get("number", 0)):
            kind = "PR " if c["content"].get("type") == "PullRequest" else ""
            print(f"  {kind}#{c['content'].get('number', '?')}  {c['title']}")
    return 0


def move(number, column, comment, approved, dry_run):
    field_id, options = columns()
    option = options.get(column.lower())
    if option is None:
        sys.exit(f"unknown column {column!r}; choose from: {', '.join(o['name'] for o in options.values())}")
    if option["name"] == "Done" and not approved:
        sys.exit("Done is the user's to approve: pass --approved once they have")
    card = next((c for c in items() if c["content"].get("number") == number), None)
    if card is None:
        sys.exit(f"#{number} is not on the board")
    print(f"#{number} {card['title']}: {card.get('status')} -> {option['name']}")
    if dry_run:
        print("dry run: nothing changed")
        return 0
    project_id = project_json("view")["id"]
    gh("project", "item-edit", "--id", card["id"], "--project-id", project_id,
       "--field-id", field_id, "--single-select-option-id", option["id"])
    if comment:
        gh("issue", "comment", str(number), "--body", comment)
        print("comment posted")
    print("moved")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("status")
    s.add_argument("--all", action="store_true")
    m = sub.add_parser("move")
    m.add_argument("number", type=int)
    m.add_argument("column")
    m.add_argument("--comment")
    m.add_argument("--approved", action="store_true")
    m.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    if args.cmd == "status":
        return status(args.all)
    return move(args.number, args.column, args.comment, args.approved, args.dry_run)


if __name__ == "__main__":
    sys.exit(main())
