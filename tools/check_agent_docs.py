#!/usr/bin/env python3
"""Keep the agent instructions in one place: AGENTS.md holds them, CLAUDE.md only imports it.

Tools such as `graphify claude install` append their own section to CLAUDE.md. This check
fails when CLAUDE.md is anything but the import line, and `--fix` moves such a section into
AGENTS.md (unless AGENTS.md already has a section with the same heading) and restores CLAUDE.md.

    python3 tools/check_agent_docs.py          # report, exit 1 on a problem
    python3 tools/check_agent_docs.py --fix    # repair
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMPORT = "@AGENTS.md\n"


def sections(text):
    """Split into (heading, body) pairs at `## ` headings; the preamble has heading None."""
    parts = re.split(r"(?m)^(?=## )", text)
    return [(p.splitlines()[0] if p.startswith("## ") else None, p) for p in parts if p]


def main():
    fix = "--fix" in sys.argv[1:]
    agents = ROOT / "AGENTS.md"
    claude = ROOT / "CLAUDE.md"
    if not agents.is_file():
        print("AGENTS.md is missing")
        return 1
    current = claude.read_text() if claude.is_file() else ""
    if current == IMPORT:
        print("ok: CLAUDE.md only imports AGENTS.md")
        return 0
    print("CLAUDE.md is not just `@AGENTS.md`")
    if not fix:
        print("run with --fix to move its extra sections into AGENTS.md")
        return 1
    have = {h for h, _ in sections(agents.read_text()) if h}
    moved = [
        body for h, body in sections(current) if h and h not in have
    ]
    if moved:
        text = agents.read_text().rstrip("\n") + "\n\n" + "\n".join(b.strip("\n") + "\n" for b in moved)
        agents.write_text(text)
        print(f"moved {len(moved)} section(s) into AGENTS.md")
    claude.write_text(IMPORT)
    print("restored CLAUDE.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
