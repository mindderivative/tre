#!/usr/bin/env bash
# Where the project stands, in one screen. Run from anywhere:  tools/status.sh
#
# Read-only. Shows the branch and what is not pushed, the working tree, the version, the newest
# LOG.md section, and the open cards on the GitHub board (tools/board.py). The board part is
# skipped with a note if `gh` is not logged in.
set -uo pipefail
cd "$(dirname "$0")/.."

echo "== git"
branch=$(git branch --show-current)
echo "branch: ${branch:-"(detached)"}"
if git rev-parse --verify -q origin/main >/dev/null; then
    ahead=$(git rev-list --count origin/main..HEAD)
    behind=$(git rev-list --count HEAD..origin/main)
    echo "vs origin/main: $ahead ahead, $behind behind"
fi
upstream=$(git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || true)
if [ -n "$upstream" ]; then
    echo "not pushed to $upstream: $(git rev-list --count "$upstream"..HEAD) commit(s)"
else
    echo "no upstream branch: nothing of this branch is pushed"
fi
dirty=$(git status --short)
if [ -n "$dirty" ]; then echo "uncommitted:"; echo "$dirty" | sed 's/^/  /'; else echo "working tree clean"; fi
echo "recent:"; git log --oneline -5 | sed 's/^/  /'

echo
echo "== version"
python3 -I tools/check_version.py || true

echo
echo "== LOG.md"
grep -m1 -E '^\*\*.*(released|unreleased)' LOG.md || true
grep -m1 '^### ' LOG.md || true

echo
echo "== board"
if gh auth status >/dev/null 2>&1; then
    python3 -I tools/board.py status
else
    echo "gh is not logged in; skipped"
fi
