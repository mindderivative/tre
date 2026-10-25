#!/usr/bin/env python3
"""Do what `graphify claude install` does, but put the instructions in AGENTS.md.

The installer writes a `## graphify` section to CLAUDE.md and registers two PreToolUse hooks in
.claude/settings.json. Here CLAUDE.md only imports AGENTS.md, so this runs the real installer in a
scratch directory (the repo is not touched by it) and then:

  * replaces the `## graphify` section of AGENTS.md with the installer's (or appends it),
  * adds any hook the installer registers that .claude/settings.json does not have yet.

Run it after upgrading graphify. It is safe to repeat; nothing changes when the files are current.

    python3 tools/graphify_agents.py           # apply
    python3 tools/graphify_agents.py --check   # report only, exit 1 if anything would change
"""
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADING = "## graphify"


def run_installer():
    """Return (section text, settings dict) as `graphify claude install` writes them."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["git", "init", "-q", str(tmp)], check=True)
        subprocess.run(
            ["graphify", "claude", "install"],
            cwd=tmp, check=True, capture_output=True, text=True,
        )
        return (
            (tmp / "CLAUDE.md").read_text().strip("\n") + "\n",
            json.loads((tmp / ".claude" / "settings.json").read_text()),
        )


def with_section(text, section):
    """Replace the graphify section (up to the next `## ` heading) or append it."""
    pattern = re.compile(rf"(?ms)^{re.escape(HEADING)}\b.*?(?=^## |\Z)")
    if pattern.search(text):
        return pattern.sub(lambda _: section + "\n", text, count=1).rstrip("\n") + "\n"
    return text.rstrip("\n") + "\n\n" + section


def with_hooks(settings, wanted):
    """Add wanted PreToolUse entries whose (matcher, command) pairs are not in settings."""
    entries = settings.setdefault("hooks", {}).setdefault("PreToolUse", [])
    have = {
        (e.get("matcher"), h.get("command"))
        for e in entries for h in e.get("hooks", [])
    }
    for entry in wanted.get("hooks", {}).get("PreToolUse", []):
        key = {(entry.get("matcher"), h.get("command")) for h in entry.get("hooks", [])}
        if not key <= have:
            entries.append(entry)
    return settings


def main():
    check = "--check" in sys.argv[1:]
    agents = ROOT / "AGENTS.md"
    settings_path = ROOT / ".claude" / "settings.json"
    section, wanted = run_installer()

    old_agents = agents.read_text()
    new_agents = with_section(old_agents, section)
    old_settings = json.loads(settings_path.read_text()) if settings_path.is_file() else {}
    new_settings = with_hooks(json.loads(json.dumps(old_settings)), wanted)

    changes = []
    if new_agents != old_agents:
        changes.append("AGENTS.md (graphify section)")
    if new_settings != old_settings:
        changes.append(".claude/settings.json (hooks)")
    if not changes:
        print("ok: AGENTS.md and .claude/settings.json already match graphify's install")
        return 0
    print("out of date: " + ", ".join(changes))
    if check:
        return 1
    if new_agents != old_agents:
        agents.write_text(new_agents)
    if new_settings != old_settings:
        settings_path.parent.mkdir(exist_ok=True)
        settings_path.write_text(json.dumps(new_settings, indent=2) + "\n")
    print("updated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
