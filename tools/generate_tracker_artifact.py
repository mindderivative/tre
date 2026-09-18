#!/usr/bin/env python3
"""Generate a Build Tracker artifact's HTML from BUILD_TRACKER.md.

Regenerates the *source file* for the interactive Build Tracker artifact
by parsing BUILD_TRACKER.md deterministically -- no hand-transcribing
markdown edits into matching HTML by eye (that mismatch is exactly how
this script's own first version came to exist).

This script cannot publish anything itself: there is no API a script can
call to push a page to claude.ai, only Claude's own Artifact tool can do
that, inside a conversation. The intended workflow (see the
`build-tracker` skill this file ships with for the full process):

    1. Edit BUILD_TRACKER.md as usual.
    2. Run this script -> writes the regenerated HTML to --out.
    3. Publish that file to the existing artifact URL (same URL every
       time -- never a fresh publish once one exists for this project).

Usage:
    python3 tools/generate_tracker_artifact.py
    python3 tools/generate_tracker_artifact.py --md BUILD_TRACKER.md --out /tmp/tracker.html
    python3 tools/generate_tracker_artifact.py --project "My Project"

This file is project-agnostic. Copy it verbatim into any repo at
`<repo>/tools/generate_tracker_artifact.py` (sibling to a
`BUILD_TRACKER.md` at the repo root) -- do not hand-edit a per-project
copy; if the format needs to change, change it here (the skill's own
copy) and re-copy it into each project that uses it.
"""

from __future__ import annotations

import argparse
import html
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MD = REPO_ROOT / "BUILD_TRACKER.md"
DEFAULT_OUT = REPO_ROOT / "tools" / "build-tracker-artifact.generated.html"

ICON_CLASS = {"✅": "done", "🚧": "progress", "⬜": "idle"}
ICON_LABEL_FALLBACK = {"✅": "Done", "🚧": "In progress", "⬜": "Not started"}


# --------------------------------------------------------------------------
# Data model
# --------------------------------------------------------------------------

@dataclass
class Item:
    label: str          # "Stage" or "Step"
    number: str | None  # e.g. "1", or None for unnumbered stages/steps
    text: str
    note: str | None
    icon: str


@dataclass
class Phase:
    number: str
    title: str
    icon: str
    items: list[Item] = field(default_factory=list)


@dataclass
class Milestone:
    number: str
    title: str
    status_icon: str
    status_label: str
    note: str
    percent: int
    phases: list[Phase] = field(default_factory=list)


@dataclass
class Tracker:
    just_closed: str
    up_next: str
    known_gaps: list[str]
    milestones: list[Milestone]
    commit: str


# --------------------------------------------------------------------------
# Inline markdown -> HTML
# --------------------------------------------------------------------------

_CODE_RE = re.compile(r"`([^`]+)`")
_BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")
_STRIKE_RE = re.compile(r"~~([^~]+)~~")


def inline_md(text: str) -> str:
    """Convert a small, known subset of inline markdown to HTML.

    Order matters: code spans are pulled out behind placeholders first so
    `**`/`~~` characters that might appear inside a code span are never
    misread as bold/strikethrough markers.
    """
    escaped = html.escape(text, quote=False)

    code_spans: list[str] = []

    def _stash_code(m: re.Match[str]) -> str:
        code_spans.append(m.group(1))
        return f"\x00CODE{len(code_spans) - 1}\x00"

    escaped = _CODE_RE.sub(_stash_code, escaped)
    escaped = _BOLD_RE.sub(r"<strong>\1</strong>", escaped)
    escaped = _STRIKE_RE.sub(
        r'<span style="text-decoration:line-through;opacity:.55">\1</span>', escaped
    )

    def _restore_code(m: re.Match[str]) -> str:
        idx = int(m.group(1))
        return f'<span class="mono">{code_spans[idx]}</span>'

    escaped = re.sub(r"\x00CODE(\d+)\x00", _restore_code, escaped)
    return escaped


# --------------------------------------------------------------------------
# Parsing
# --------------------------------------------------------------------------

TOP_ROW_RE = re.compile(r"^\|\s*(M\d+)\s*—[^|]*\|\s*[^|]*?(\d+)%\s*\|")
MILESTONE_HEADING_RE = re.compile(r"^## Milestone (\d+)\s*—\s*(.+)$")
STATUS_LINE_RE = re.compile(r"^\*\*Status:\s*(✅|⬜|🚧)\s*([^.*]+)\.\*\*\s*(.*)$")
PHASE_HEADING_RE = re.compile(r"^### Phase (\d+)\s*—\s*(.+?)\s*(✅|⬜|🚧)\s*$")
ITEM_RE = re.compile(
    r"^- (Stage|Step)(?:\s+(\d+))?:\s*(.+?)\s*—\s*(✅|⬜|🚧)\s*(\(.*\))?\s*$"
)


def _strip_note(raw: str | None) -> str | None:
    if not raw:
        return None
    inner = raw.strip()
    if inner.startswith("(") and inner.endswith(")"):
        inner = inner[1:-1]
    return inner


def parse_percentages(lines: list[str]) -> dict[str, int]:
    out: dict[str, int] = {}
    for line in lines:
        m = TOP_ROW_RE.match(line)
        if m:
            out[m.group(1)] = int(m.group(2))
    return out


def parse_narrative(text: str) -> tuple[str, str, list[str]]:
    def _grab(label: str) -> str:
        # Every closed milestone conventionally writes its own
        # "**Just closed:**"/"**Up next:**" pair at its own point in the
        # file, so a plain `re.search` would always find the *oldest*
        # one, frozen there forever regardless of how far the project
        # moves. The most recent pair (closest to the bottom of the
        # file) is the one that actually answers "what's the current
        # status" -- take the last match, not the first.
        matches = list(re.finditer(rf"\*\*{label}:\*\*\s*(.+?)(?=\n\n|\Z)", text, re.S))
        return matches[-1].group(1).strip() if matches else ""

    just_closed = _grab("Just closed")
    up_next = _grab("Up next")

    gaps: list[str] = []
    gaps_match = re.search(r"\*\*Known gaps:\*\*\s*\n((?:- .+\n?)+)", text)
    if gaps_match:
        for line in gaps_match.group(1).splitlines():
            line = line.strip()
            if line.startswith("- "):
                gaps.append(line[2:].strip())
    return just_closed, up_next, gaps


def parse_milestones(lines: list[str], percentages: dict[str, int]) -> list[Milestone]:
    milestones: list[Milestone] = []
    current_milestone: Milestone | None = None
    current_phase: Phase | None = None

    i = 0
    while i < len(lines):
        line = lines[i]

        m = MILESTONE_HEADING_RE.match(line)
        if m:
            number, title = m.group(1), m.group(2).strip()
            # next non-blank line should be the **Status: ...** line
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            status_icon, status_label, note = "⬜", "Not started", ""
            if j < len(lines):
                sm = STATUS_LINE_RE.match(lines[j].strip())
                if sm:
                    status_icon, status_label, note = sm.group(1), sm.group(2).strip(), sm.group(3).strip()
            key = f"M{number}"
            current_milestone = Milestone(
                number=number,
                title=title,
                status_icon=status_icon,
                status_label=status_label,
                note=note,
                percent=percentages.get(key, 0),
            )
            milestones.append(current_milestone)
            current_phase = None
            i += 1
            continue

        pm = PHASE_HEADING_RE.match(line)
        if pm and current_milestone is not None:
            current_phase = Phase(number=pm.group(1), title=pm.group(2).strip(), icon=pm.group(3))
            current_milestone.phases.append(current_phase)
            i += 1
            continue

        im = ITEM_RE.match(line)
        if im and current_phase is not None:
            label, number, text, icon, note_raw = im.groups()
            current_phase.items.append(
                Item(label=label, number=number, text=text.strip(), note=_strip_note(note_raw), icon=icon)
            )
            i += 1
            continue

        # A `- Step N:`/`- Stage N:` line that doesn't match ITEM_RE
        # (missing "— <icon>" marker, or an unbalanced trailing note
        # paren) would otherwise fall straight through with no signal
        # at all -- silently dropping that step's entire real content
        # from the rendered artifact. Fail loudly instead: a markdown-
        # formatting slip here is exactly the "hand-transcribing" class
        # of bug this script exists to prevent. See the `build-tracker`
        # skill's own "Exact format" section for the required grammar.
        if current_phase is not None and re.match(r"^- (Stage|Step)\b", line.strip()):
            raise SystemExit(
                f"generate_tracker_artifact.py: line {i + 1} looks like a Step/Stage "
                f"item but doesn't match ITEM_RE (needs '— ✅/⬜/🚧' at the end, "
                f"optionally followed by a balanced '(...)' note):\n  {line.strip()!r}"
            )

        i += 1

    return milestones


def parse_tracker(md_text: str, commit: str) -> Tracker:
    lines = md_text.splitlines()
    percentages = parse_percentages(lines)
    just_closed, up_next, gaps = parse_narrative(md_text)
    milestones = parse_milestones(lines, percentages)
    return Tracker(
        just_closed=just_closed,
        up_next=up_next,
        known_gaps=gaps,
        milestones=milestones,
        commit=commit,
    )


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------

ICON_SVG = {
    "done": '<svg class="icon done" viewBox="0 0 24 24" fill="none"><path d="M5 13l4 4L19 7" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    "progress": '<svg class="icon progress" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="8" stroke="currentColor" stroke-width="2.2"/><path d="M8 12h8" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/></svg>',
    "idle": '<svg class="icon idle" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="8" stroke="currentColor" stroke-width="2.2"/></svg>',
}

CHEV = '<svg class="chev" viewBox="0 0 24 24" fill="none"><path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg>'
CHEV_SMALL = '<svg class="chev small" viewBox="0 0 24 24" fill="none"><path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'


def render_item(item: Item) -> str:
    prefix = f"{item.label} {item.number}: " if item.number else f"{item.label}: "
    text_html = inline_md(prefix + item.text)
    note_html = f' <span class="step-note">— {inline_md(item.note)}</span>' if item.note else ""
    icon_html = ICON_SVG[ICON_CLASS.get(item.icon, "idle")]
    return f'<li>{icon_html}<span class="step-text">{text_html}{note_html}</span></li>'


def render_phase(phase: Phase, open_first: bool) -> str:
    css_class = ICON_CLASS.get(phase.icon, "idle")
    badge_label = ICON_LABEL_FALLBACK.get(phase.icon, "Not started")
    items_html = "\n              ".join(render_item(it) for it in phase.items)
    return f"""          <details class="phase">
            <summary>
              {CHEV_SMALL}
              <span class="p-title">Phase {phase.number} — {inline_md(phase.title)}</span>
              <span class="badge {css_class}">{badge_label}</span>
            </summary>
            <ul class="steps">
              {items_html}
            </ul>
          </details>"""


def render_milestone(m: Milestone, open_by_default: bool) -> str:
    css_class = ICON_CLASS.get(m.status_icon, "idle")
    bar_class = "" if m.percent > 0 else " zero"
    bar_width = f"{m.percent}%" if m.percent > 0 else "0"
    open_attr = " open" if open_by_default else ""

    phases_html = ""
    if m.phases:
        phase_blocks = "\n\n".join(render_phase(p, False) for p in m.phases)
        phases_html = f'\n        <div class="phases">\n{phase_blocks}\n        </div>'

    note_html = f'<p class="milestone-note">{inline_md(m.note)}</p>' if m.note else ""

    return f"""    <details class="milestone"{open_attr}>
      <summary>
        {CHEV}
        <div class="m-head">
          <div class="m-title-row">
            <span class="display">M{m.number} — {inline_md(m.title)}</span>
            <span class="badge {css_class}">{m.status_label}</span>
          </div>
          <div class="m-progress">
            <span class="bar-track"><span class="bar-fill{bar_class}" style="width:{bar_width}"></span></span>
            <span class="pct">{m.percent}%</span>
          </div>
        </div>
      </summary>
      <div class="milestone-body">
        {note_html}{phases_html}
      </div>
    </details>"""


def render_gap(gap: str) -> str:
    # inline_md() already converts ~~struck~~ markdown to a strikethrough
    # span -- no separate wrapping needed here.
    return f"<li>{inline_md(gap)}</li>"


PAGE_TEMPLATE = """<title>{project} Build Tracker</title>
<style>
  @import url('https://fonts.googleapis.com/css2?family=Manrope:wght@500;700;800&display=swap');

  :root {{
    --bg: #faf9fc; --surface: #ffffff; --surface-alt: #f3effa; --border: #e3dfea;
    --text: #1c1b1f; --text-muted: #635e6d; --accent: #6750a4; --accent-soft: #efe7fb;
    --success: #1e6b30; --success-bg: #e2f4e6; --warn: #8a5a00; --warn-bg: #fbedd3;
    --idle: #635e6d; --idle-bg: #edeaf2;
    --shadow: 0 1px 2px rgba(28,27,31,0.04), 0 4px 16px rgba(28,27,31,0.06);
    color-scheme: light dark;
  }}
  @media (prefers-color-scheme: dark) {{
    :root:not([data-theme="light"]) {{
      --bg: #131218; --surface: #1c1a22; --surface-alt: #242030; --border: #34303e;
      --text: #eae6f0; --text-muted: #a79fb3; --accent: #cfbcff; --accent-soft: #2c2540;
      --success: #8bdc9f; --success-bg: #16281c; --warn: #f0c46b; --warn-bg: #302509;
      --idle: #a79fb3; --idle-bg: #262231;
      --shadow: 0 1px 2px rgba(0,0,0,0.3), 0 4px 20px rgba(0,0,0,0.35);
    }}
  }}
  :root[data-theme="dark"] {{
    --bg: #131218; --surface: #1c1a22; --surface-alt: #242030; --border: #34303e;
    --text: #eae6f0; --text-muted: #a79fb3; --accent: #cfbcff; --accent-soft: #2c2540;
    --success: #8bdc9f; --success-bg: #16281c; --warn: #f0c46b; --warn-bg: #302509;
    --idle: #a79fb3; --idle-bg: #262231;
    --shadow: 0 1px 2px rgba(0,0,0,0.3), 0 4px 20px rgba(0,0,0,0.35);
  }}

  * {{ box-sizing: border-box; }}
  body {{ margin: 0; background: var(--bg); color: var(--text);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    line-height: 1.5; }}
  .mono {{ font-family: ui-monospace, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace; font-size: 0.86em; }}
  h1, .display {{ font-family: "Manrope", -apple-system, sans-serif; text-wrap: balance; }}
  .page {{ max-width: 880px; margin: 0 auto; padding: 28px 20px 64px; }}

  header.top {{ display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 22px; }}
  header.top h1 {{ font-size: 1.5rem; font-weight: 800; margin: 0 0 4px; letter-spacing: -0.01em; }}
  header.top p {{ margin: 0; color: var(--text-muted); font-size: 0.88rem; }}

  .controls {{ display: flex; gap: 8px; flex-shrink: 0; }}
  .btn {{ font: inherit; font-size: 0.78rem; font-weight: 700; color: var(--accent);
    background: var(--accent-soft); border: 1px solid transparent; border-radius: 8px;
    padding: 7px 12px; cursor: pointer; white-space: nowrap; }}
  .btn:hover {{ filter: brightness(1.05); }}
  .btn:active {{ transform: translateY(1px); }}

  .metrics {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin-bottom: 24px; }}
  @media (max-width: 640px) {{ .metrics {{ grid-template-columns: 1fr; }} }}
  .metric {{ background: var(--surface); border: 1px solid var(--border); border-radius: 14px;
    padding: 14px 16px; box-shadow: var(--shadow); }}
  .metric .eyebrow {{ font-size: 0.68rem; font-weight: 800; text-transform: uppercase; letter-spacing: 0.06em;
    color: var(--text-muted); display: flex; align-items: center; gap: 6px; margin-bottom: 6px; }}
  .metric .eyebrow .dot {{ width: 7px; height: 7px; border-radius: 50%; display: inline-block; flex-shrink: 0; }}
  .metric p {{ margin: 0; font-size: 0.83rem; color: var(--text); }}
  .metric.gaps ul {{ margin: 0; padding-left: 16px; font-size: 0.82rem; }}
  .metric.gaps li {{ margin-bottom: 5px; }}
  .metric.gaps li:last-child {{ margin-bottom: 0; }}

  .overview {{ background: var(--surface); border: 1px solid var(--border); border-radius: 14px;
    box-shadow: var(--shadow); padding: 14px 16px; margin-bottom: 24px; display: grid; gap: 12px; }}
  .overview-row {{ display: grid; grid-template-columns: 130px 1fr 40px; align-items: center; gap: 12px; }}
  .overview-row .label {{ font-size: 0.82rem; font-weight: 700; }}
  .overview-row .pct {{ font-size: 0.8rem; font-weight: 700; text-align: right; color: var(--text-muted);
    font-variant-numeric: tabular-nums; }}
  .bar-track {{ display: block; height: 8px; border-radius: 5px; background: var(--idle-bg); overflow: hidden; }}
  .bar-fill {{ display: block; height: 100%; border-radius: 5px; background: var(--accent); }}
  .bar-fill.zero {{ width: 0; }}

  .badge {{ display: inline-flex; align-items: center; gap: 5px; font-size: 0.68rem; font-weight: 800;
    text-transform: uppercase; letter-spacing: 0.04em; padding: 3px 8px; border-radius: 999px;
    white-space: nowrap; flex-shrink: 0; }}
  .badge.done {{ color: var(--success); background: var(--success-bg); }}
  .badge.progress {{ color: var(--warn); background: var(--warn-bg); }}
  .badge.idle {{ color: var(--idle); background: var(--idle-bg); }}

  .milestones {{ display: grid; gap: 12px; }}
  details.milestone {{ background: var(--surface); border: 1px solid var(--border); border-radius: 14px;
    box-shadow: var(--shadow); overflow: hidden; }}
  details.milestone > summary {{ list-style: none; cursor: pointer; padding: 14px 16px;
    display: flex; align-items: center; gap: 12px; }}
  details.milestone > summary::-webkit-details-marker {{ display: none; }}
  .chev {{ width: 18px; height: 18px; flex-shrink: 0; color: var(--text-muted); transition: transform 0.18s ease; }}
  details[open] > summary .chev {{ transform: rotate(90deg); }}
  .m-head {{ flex: 1; min-width: 0; }}
  .m-title-row {{ display: flex; align-items: center; gap: 8px; margin-bottom: 6px; flex-wrap: wrap; }}
  .m-title-row .display {{ font-size: 0.98rem; font-weight: 800; }}
  .m-progress {{ display: flex; align-items: center; gap: 8px; }}
  .m-progress .bar-track {{ flex: 1; min-width: 80px; }}
  .m-progress .pct {{ font-size: 0.74rem; font-weight: 700; color: var(--text-muted);
    font-variant-numeric: tabular-nums; width: 34px; text-align: right; flex-shrink: 0; }}
  .milestone-body {{ border-top: 1px solid var(--border); padding: 4px 16px 14px; }}
  .milestone-note {{ font-size: 0.82rem; color: var(--text-muted); padding: 10px 0 4px; }}

  .phases {{ display: grid; gap: 8px; margin-top: 8px; }}
  details.phase {{ background: var(--surface-alt); border: 1px solid var(--border); border-radius: 10px; overflow: hidden; }}
  details.phase > summary {{ list-style: none; cursor: pointer; padding: 10px 12px; display: flex; align-items: center; gap: 10px; }}
  details.phase > summary::-webkit-details-marker {{ display: none; }}
  .chev.small {{ width: 15px; height: 15px; }}
  .p-title {{ flex: 1; font-size: 0.86rem; font-weight: 700; }}

  .steps {{ list-style: none; margin: 0; padding: 2px 12px 12px 37px; display: grid; gap: 7px; }}
  .steps li {{ display: flex; gap: 8px; align-items: flex-start; font-size: 0.83rem; }}
  .steps li .icon {{ flex-shrink: 0; width: 16px; height: 16px; margin-top: 1px; }}
  .steps li .icon.done {{ color: var(--success); }}
  .steps li .icon.progress {{ color: var(--warn); }}
  .steps li .icon.idle {{ color: var(--idle); }}
  .steps li .step-note {{ color: var(--text-muted); }}

  footer.sync {{ margin-top: 26px; text-align: center; font-size: 0.76rem; color: var(--text-muted); }}
</style>

<div class="page">
  <header class="top">
    <div>
      <h1>{project} Build Tracker</h1>
      <p>synced with <span class="mono">BUILD_TRACKER.md</span> @ <span class="mono">{commit}</span> — generated, not hand-edited</p>
    </div>
    <div class="controls">
      <button class="btn" id="expandAll">Expand all</button>
      <button class="btn" id="collapseAll">Collapse all</button>
    </div>
  </header>

  <section class="metrics">
    <div class="metric">
      <div class="eyebrow"><span class="dot" style="background:var(--success)"></span>Just closed</div>
      <p>{just_closed}</p>
    </div>
    <div class="metric">
      <div class="eyebrow"><span class="dot" style="background:var(--accent)"></span>Up next</div>
      <p>{up_next}</p>
    </div>
    <div class="metric gaps">
      <div class="eyebrow"><span class="dot" style="background:var(--warn)"></span>Known gaps</div>
      <ul>
{gaps_html}
      </ul>
    </div>
  </section>

  <section class="overview">
{overview_rows}
  </section>

  <section class="milestones" id="milestones">
{milestones_html}
  </section>

  <footer class="sync">Generated from <span class="mono">BUILD_TRACKER.md</span> by <span class="mono">tools/generate_tracker_artifact.py</span> — do not hand-edit this file.</footer>
</div>

<script>
  document.getElementById('expandAll').addEventListener('click', () => {{
    document.querySelectorAll('details').forEach(d => d.open = true);
  }});
  document.getElementById('collapseAll').addEventListener('click', () => {{
    document.querySelectorAll('details').forEach(d => d.open = false);
  }});
</script>
"""


def render_html(tracker: Tracker, project: str) -> str:
    gaps_html = "\n".join(f"        {render_gap(g)}" for g in tracker.known_gaps)

    overview_rows = "\n".join(
        f"""    <div class="overview-row">
      <span class="label">M{m.number} — {inline_md(m.title)}</span>
      <span class="bar-track"><span class="bar-fill{'' if m.percent > 0 else ' zero'}" style="width:{m.percent if m.percent > 0 else 0}%"></span></span>
      <span class="pct">{m.percent}%</span>
    </div>"""
        for m in tracker.milestones
    )

    # Open every milestone except one with zero phases and 100% (pure
    # archive, nothing to drill into) -- matches the hand-authored
    # version's original judgment call, kept deterministic.
    milestones_html = "\n\n".join(
        render_milestone(m, open_by_default=not (len(m.phases) == 0 and m.percent == 100))
        for m in tracker.milestones
    )

    return PAGE_TEMPLATE.format(
        project=html.escape(project, quote=False),
        commit=tracker.commit,
        just_closed=inline_md(tracker.just_closed),
        up_next=inline_md(tracker.up_next),
        gaps_html=gaps_html,
        overview_rows=overview_rows,
        milestones_html=milestones_html,
    )


# --------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------

def git_short_hash(md_path: Path) -> str:
    try:
        out = subprocess.run(
            ["git", "log", "-1", "--format=%h", "--", str(md_path.name)],
            cwd=md_path.parent,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        h = out.stdout.strip()
        if h:
            return h
    except Exception:
        pass
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=md_path.parent,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        return out.stdout.strip() or "unknown"
    except Exception:
        return "unknown"


def default_project_name(md_path: Path) -> str:
    """Best-effort project name when --project isn't given: the repo
    root's own directory name, title-cased word-by-word only if it
    looks like a plain slug (avoids mangling something like "tre-v2"
    into "Tre V2" -- kept as-is if it already has any uppercase)."""
    root = md_path.resolve().parent
    name = root.name
    if name and name == name.lower():
        name = name.replace("-", " ").replace("_", " ").title()
    return name or "Project"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--md", type=Path, default=DEFAULT_MD, help="Path to BUILD_TRACKER.md")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT, help="Output HTML path")
    parser.add_argument(
        "--project",
        type=str,
        default=None,
        help="Project name shown in the artifact header (default: derived from the repo directory name)",
    )
    args = parser.parse_args()

    if not args.md.exists():
        print(f"error: {args.md} not found", file=sys.stderr)
        return 1

    project = args.project or default_project_name(args.md)
    md_text = args.md.read_text(encoding="utf-8")
    commit = git_short_hash(args.md)
    tracker = parse_tracker(md_text, commit)

    if not tracker.milestones:
        print("error: parsed zero milestones -- BUILD_TRACKER.md format may have changed; "
              "check the heading/status-line regexes in this script", file=sys.stderr)
        return 1

    html_out = render_html(tracker, project)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(html_out, encoding="utf-8")

    total_phases = sum(len(m.phases) for m in tracker.milestones)
    total_items = sum(len(p.items) for m in tracker.milestones for p in m.phases)
    print(f"Parsed {len(tracker.milestones)} milestones, {total_phases} phases, {total_items} items.")
    print(f"Wrote {args.out}")
    print("Next: publish this file to the existing artifact URL (same file path each time keeps the link).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
