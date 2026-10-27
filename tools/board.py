#!/usr/bin/env python3
"""Read and move cards on the GitHub project board "Tesserae Rendering Engine" (project 3).

    python3 tools/board.py status [--all]          # open cards by column (--all adds Done)
    python3 tools/board.py move N COLUMN [--comment TEXT] [--approved] [--dry-run]
    python3 tools/board.py create TITLE (--body TEXT | --body-file FILE) [--label L]... [--column COLUMN] [--dry-run]

COLUMN is Backlog, Ready, "In progress", "In review" or Done (case does not matter). Done is the
user's to approve, so moving to Done needs --approved. --comment posts TEXT on issue or PR N.
`create` opens an issue on the repo and puts it on the board (default column Backlog). Anything but
Backlog or Ready is refused: a card only reaches In progress, In review or Done by `move`. Issues are
public, so use --dry-run first to see what would be filed. Needs `gh` logged in with the `project`
scope.
"""
import argparse
import json
import subprocess
import sys

OWNER = "mindderivative"
PROJECT = "3"
REPO = f"{OWNER}/tre"


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


def set_status(item_id, field_id, option):
    project_id = project_json("view")["id"]
    gh("project", "item-edit", "--id", item_id, "--project-id", project_id,
       "--field-id", field_id, "--single-select-option-id", option["id"])


def create(title, body, labels, column, dry_run):
    field_id, options = columns()
    option = options.get(column.lower())
    if option is None:
        sys.exit(f"unknown column {column!r}; choose from: {', '.join(o['name'] for o in options.values())}")
    if option["name"] not in ("Backlog", "Ready"):
        sys.exit("create files cards in Backlog or Ready only; use `move` for the other columns")
    if not title.strip() or not body.strip():
        sys.exit("an issue needs a title and a body")
    print(f"issue on {REPO}: {title}")
    print(f"  labels: {', '.join(labels) or '(none)'}   column: {option['name']}   body: {len(body)} chars")
    if dry_run:
        print("dry run: nothing filed")
        return 0
    cmd = ["issue", "create", "-R", REPO, "--title", title, "--body", body]
    for label in labels:
        cmd += ["--label", label]
    url = gh(*cmd).strip().splitlines()[-1]
    print(f"filed {url}")
    # The project may add new issues by itself (an auto-add workflow); then item-add says the
    # content already exists, and the card is found by its issue number instead.
    added = subprocess.run(["gh", "project", "item-add", PROJECT, "--owner", OWNER, "--url", url, "--format", "json"],
                           capture_output=True, text=True)
    if added.returncode == 0:
        item_id = json.loads(added.stdout)["id"]
    elif "already exists" in added.stderr:
        number = int(url.rstrip("/").rsplit("/", 1)[-1])
        card = next((c for c in items() if c["content"].get("number") == number), None)
        if card is None:
            sys.exit(f"{url} is filed and on the board already, but its card was not found; check the board")
        item_id = card["id"]
    else:
        sys.exit(f"gh project item-add failed: {added.stderr.strip()}")
    set_status(item_id, field_id, option)
    print(f"on the board in {option['name']}")
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
    set_status(card["id"], field_id, option)
    if comment:
        gh("issue", "comment", str(number), "-R", REPO, "--body", comment)
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
    c = sub.add_parser("create")
    c.add_argument("title")
    body = c.add_mutually_exclusive_group(required=True)
    body.add_argument("--body")
    body.add_argument("--body-file")
    c.add_argument("--label", action="append", default=[])
    c.add_argument("--column", default="Backlog")
    c.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    if args.cmd == "status":
        return status(args.all)
    if args.cmd == "create":
        text = args.body if args.body is not None else sys.stdin.read() if args.body_file == "-" else open(args.body_file).read()
        return create(args.title, text, args.label, args.column, args.dry_run)
    return move(args.number, args.column, args.comment, args.approved, args.dry_run)


if __name__ == "__main__":
    sys.exit(main())
