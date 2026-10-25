#!/usr/bin/env bash
# Rebuild the graphify code graph when a source file changed since it was last built.
#
# Registered as a Claude Code `Stop` hook in .claude/settings.json, so the graph is current at the end
# of every turn that touched code (AGENTS.md asks for `graphify update .` after code changes). It is
# a no-op, in milliseconds, when no .rs or .py file is newer than graphify-out/graph.json, and it
# never fails the hook: a missing graphify, a missing graph or an error all exit 0, quietly.
# `graphify update` is AST-only (no API calls) and takes about five seconds; graphify-out/ is ignored
# by git, so nothing is left to commit.
#
#     tools/graphify_refresh.sh            # refresh if stale
#     tools/graphify_refresh.sh --check    # print whether it is stale, change nothing
set -uo pipefail
cd "$(dirname "$0")/.." || exit 0

graph=graphify-out/graph.json
command -v graphify >/dev/null 2>&1 || exit 0
[ -f "$graph" ] || exit 0

newer=$(find crates python tests tools -type f \( -name '*.rs' -o -name '*.py' \) -newer "$graph" \
    -not -path '*/__pycache__/*' -print -quit 2>/dev/null)
if [ -z "$newer" ]; then
    [ "${1:-}" = "--check" ] && echo "graph is current"
    exit 0
fi
if [ "${1:-}" = "--check" ]; then
    echo "graph is stale: $newer is newer than $graph"
    exit 0
fi
graphify update . >/dev/null 2>&1 || true
exit 0
