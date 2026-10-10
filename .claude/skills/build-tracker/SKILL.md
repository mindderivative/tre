---
name: build-tracker
description: Create, update, and publish a "Build Tracker" — a BUILD_TRACKER.md progress document (Milestones > Phases > Steps) paired with a generated, published interactive HTML Artifact. Use whenever a user asks to set up project/build/progress tracking, create or update a "build tracker", or sync an existing BUILD_TRACKER.md to its published artifact, in any repo.
---

# Build Tracker

A reusable, cross-project pattern: one markdown file (`BUILD_TRACKER.md`,
committed to the repo, hand-edited, the real source of truth) plus one
deterministic Python script that renders it into an interactive HTML
page, published as a Claude Artifact and kept at a stable URL across
every update. This skill exists so that pattern — and the real
formatting bugs it takes to get it wrong — never has to be rediscovered
per project.

This directory ships two assets, used as-is, never rewritten from
scratch:

- `generate_tracker_artifact.py` — the parser/renderer. Project-agnostic.
- `BUILD_TRACKER_TEMPLATE.md` — a minimal, validated-correct starting
  skeleton for a brand-new tracker.

## Architecture — why two files and one artifact

- **`BUILD_TRACKER.md`** (repo root): the actual source of truth, git-
  tracked, hand-edited prose with a small amount of required structure
  (below). This is what you read to understand a project's real state,
  and what future Claude sessions should check first.
- **`<repo>/tools/generate_tracker_artifact.py`**: a copy of this
  skill's own script, deterministically parsing `BUILD_TRACKER.md` into
  HTML. Never hand-edit a project's copy of this script — if the format
  needs to change, change it in this skill directory and re-copy it
  into each project that uses it, so every project stays on the same,
  bug-fixed version.
- **The published Artifact**: the human-facing dashboard — collapsible
  milestones, progress bars, an "expand all"/"collapse all" toggle. It
  is *regenerated and republished to the same URL* every time; it is
  never a fresh publish once one exists for a project (a fresh publish
  orphans the old link and defeats the entire point of a stable
  dashboard people can bookmark).

## First-time setup in a new project

1. Copy `generate_tracker_artifact.py` from this skill's directory to
   `<project-root>/tools/generate_tracker_artifact.py`.
2. Create `<project-root>/BUILD_TRACKER.md`. Either adapt
   `BUILD_TRACKER_TEMPLATE.md` (copy it in, then replace the placeholder
   milestones with the project's real ones), or hand-write it following
   the exact grammar below — but validate against the template's own
   structure either way, since every piece of that template is required
   by the parser, not decorative.
3. Run `python3 tools/generate_tracker_artifact.py` from the repo root.
   It must print `Parsed N milestones, M phases, K items.` and exit 0.
   If it raises instead, fix the reported line (see "Exact format"
   below) — do not work around it by editing the script.
4. Publish `tools/build-tracker-artifact.generated.html` via the
   Artifact tool. Suggested favicon: 🧭. Report the URL back to the
   user, and offer to pin it (`action: "pin"`) if they'll return to it
   regularly.
5. Record the artifact URL somewhere durable for this project (project
   memory, or just tell the user) — every future update needs to
   *republish to that same URL* (`url: "<the artifact URL>"` on the
   Artifact tool call), not create a new one.

## Ongoing maintenance workflow

Every time a milestone, phase, stage, or step completes (or starts, or
gets scoped but not yet started):

1. Edit `BUILD_TRACKER.md`:
   - Update the **Top Metrics** table row for that milestone (progress
     bar string + percent + status cell).
   - Update the milestone's own `### Phase N — ... <icon>` heading icon.
   - Update the specific `- Step N: ... — <icon> (...)` line for the
     item that changed.
   - Update the milestone's own `**Status: ...**` line if the
     milestone's overall state changed (see "the M27 bug" below — this
     line drifts easily and is easy to forget).
   - All three (or four) of the above, every time — updating only the
     Top Metrics table while leaving a stale per-milestone status line
     is a real bug this skill exists partly to prevent (confirmed to
     still happen even with this rule already written down — see the
     M52 recurrence in "Lessons learned" below — so treat step 4 in
     "Publishing checklist" as mandatory, not optional, every time).
2. **Every single time you touch `BUILD_TRACKER.md` for any reason at
   all** — not only when the thing that changed obviously affects one
   of these — review and, if stale, rewrite all three of the following
   together as one unit. They are three views onto the same current
   state, and letting any one lag the others is the exact class of bug
   this section exists to prevent:
   - **"Just closed"** — the most recently finished milestone/phase,
     summarized accurately as of *now*, not as of whenever it was last
     written.
   - **"Up next"** — what's actually scoped and not yet done right now.
     If nothing is currently scoped, say so explicitly rather than
     leaving a stale pointer to something already finished or already
     started.
   - **"Known gaps"** — re-read the *whole* list, not just the bullet
     related to what you just changed, and confirm every remaining
     bullet is still real and still open. If this update closed one:
     **move** it out of "Known gaps" and into "Fixed gaps" (creating
     that section if this is the first one), rather than leaving it
     struck-through in place — "Known gaps" is meant to answer "what's
     still open" at a glance, which a bullet that stays there forever,
     just struck through, defeats the moment the list gets long. See
     "Exact format" below for the grammar; the bullet's own text/
     strikethrough/`**Fixed**`-or-`**Resolved**` note usually carries
     over unchanged, just relocated.
   This pair-plus-list refresh (see "Exact format" below for the exact
   grammar) is what the generated artifact's own top-of-page highlight
   boxes are built from — a stale one there is exactly as visible and
   exactly as embarrassing as a stale per-milestone `**Status:**` line.
3. Run `python3 tools/generate_tracker_artifact.py`. It must exit 0.
4. **Audit every milestone's own `**Status:**` line against its Top
   Metrics row before publishing** — this is the concrete, mechanical
   check that catches the M27/M52-class bug (below) instead of relying
   on a human noticing it later. Fast enough to run every time, not
   just when something feels off:
   ```
   python3 -c "
   import re
   content = open('BUILD_TRACKER.md').read()
   tm = dict((int(m[0]), m[2].strip()) for m in re.findall(
       r'^\| M(\d+) — .*? \| .*?(\d+)% \| (.*?) \|\$', content, re.MULTILINE))
   ms = dict((int(m[0]), m[1].strip()) for m in re.findall(
       r'^## Milestone (\d+)[^\n]*\n\n\*\*Status: (.*?)\.\*\*', content, re.MULTILINE))
   for n in sorted(set(tm) | set(ms)):
       t, s = tm.get(n), ms.get(n)
       if t is None or s is None:
           print(f'M{n}: missing from one side -- top_metrics={t!r} status={s!r}'); continue
       done = lambda x: '✅' in x or 'Complete' in x or 'Archived' in x
       prog = lambda x: '🚧' in x
       if done(t) != done(s) or prog(t) != prog(s):
           print(f'M{n}: MISMATCH -- Top Metrics={t!r}  Status line={s!r}')
   "
   ```
   A clean run prints nothing. Any printed line is a real bug to fix
   before publishing, not after.
5. Publish the regenerated HTML via the Artifact tool, passing the
   *existing* artifact `url` so it updates in place.
6. If this project keeps other planning docs in sync with
   `BUILD_TRACKER.md` (a `PLAN.md`/`LOG.md` pair, say), update those
   too, per that project's own conventions — this skill only owns the
   tracker itself.

## Archiving a large tracker

A tracker that has grown to many dozens of milestones gets unwieldy in
two independent ways: the file itself is long to scroll, and its own
front matter (everything before the first `## Milestone` heading) tends
to accumulate stacked, mostly-redundant "Just closed"/"Up next" pairs
over time if a project has been writing a fresh one at the top on every
close rather than overwriting. When a user asks to split/archive a
tracker (first done for a real project 2026-09-23, at 56 milestones):

1. **Where the archive lives:** a plain markdown file at the repo root,
   named `<TRACKER_BASENAME>_ARCHIVE_M<A>-M<B>.md` (e.g.
   `BUILD_TRACKER_ARCHIVE_M1-M50.md`) — **not** a second published
   Artifact. The whole point of this skill's own "one tracker, one
   stable URL" architecture is a single dashboard people bookmark;
   introducing a second published artifact for old history undermines
   that. The archive is meant to be read on GitHub/in an editor, not
   as an interactive page.
2. **What goes in the archive:** a short dated intro (date archived,
   the milestone range, a link back to the live tracker), the Top
   Metrics rows for that range only, the "Fixed gaps" list as it then
   stood (kept, since it's genuine historical context for that range),
   and the full `## Milestone <N>` sections for that range, copied
   verbatim — relocated, not rewritten. **No "Known gaps" section** —
   that's inherently a live-state concept, and duplicating it invites
   the two copies drifting apart; the archive should point at the live
   file for current gaps instead.
3. **What the live tracker becomes:** Top Metrics trimmed to start at
   the last archived milestone number (**one milestone of intentional
   overlap** — that milestone's own full `## Milestone <N>` section
   also stays in the live file, not just its Top Metrics row, so a
   reader starting from either file has real context for where the
   other one picks up); a short note near the top linking to the new
   archive file; front matter trimmed to exactly the *current*
   "Just closed"/"Up next" pair (the older stacked ones' substance
   already lives in each Milestone's own write-up, and now in the
   archive too); "Known gaps" freshly re-audited (see below); "Fixed
   gaps" kept as the *full*, un-trimmed history (low-cost, collapsed by
   default, worth keeping complete in the actively-maintained file).
4. **Known gaps refresh, every time this is done:** don't just carry
   the existing "Known gaps" bullets forward unexamined. Grep/read
   every recent milestone's own "Explicitly out of scope, named not
   silent" notes and confirm each candidate is *still* true against
   live source, not assumed from the note alone — this is exactly the
   same real-verification discipline "Fixed gaps" bullets already
   require, applied in the other direction (finding gaps that were
   *found and named* but never promoted to the visible "Known gaps"
   list at all). Distinguish a real missing capability from a
   deliberate, already-accepted tradeoff named elsewhere (e.g. "handler
   storage is never pruned on node removal") — the latter doesn't
   belong in "Known gaps," which exists to answer "what's actually
   still open," not "every design decision that could theoretically be
   revisited."

## Exact `BUILD_TRACKER.md` format (required grammar)

The parser is regex-based and deliberately strict — strict enough to
`raise` instead of silently dropping content when a line doesn't match
(a real bug this format used to have, fixed in this script; see
"Lessons learned" below). Every rule here is load-bearing.

**Top Metrics table** — one row per milestone, matched only to extract
the milestone number and percent:
```
| M<N> — <title...> | <anything> <NN>% | <anything> |
```
The percent must be a literal integer followed by `%` somewhere in the
second cell (the `██████████`/`⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜` bar string itself isn't
parsed, it's just there for humans reading the raw markdown).

**Milestone heading**, exactly:
```
## Milestone <N> — <Title>
```
immediately followed (blank lines allowed in between) by a status line,
exactly:
```
**Status: <✅|⬜|🚧> <short label, no periods or asterisks>.** <rest of the paragraph...>
```
The short label (e.g. `Complete — all 3 phases done (2026-09-18)`) is
what the published artifact shows as the milestone's own badge — it
must contain **no periods** (the regex stops at the first `.`) and no
literal `*` characters. Keep it short; put detail in the rest of the
paragraph after `.**`.

**Phase heading**, exactly, ending in one of the three icons with
nothing after it on the line:
```
### Phase <N> — <Title> <✅|⬜|🚧>
```

**Step/Stage line** — this is the one every real formatting bug this
session hit came from:
```
- Step <N>: <text...> — <✅|⬜|🚧>
- Step <N>: <text...> — <✅|⬜|🚧> (<note text...>)
- Stage <N>: <same two shapes>
- Step: <unnumbered form also works>
```
**The line must end with `— <icon>`, optionally followed by one
balanced `(...)` group, and nothing else.** A period, a stray closing
thought, or the icon appearing mid-sentence instead of at the true end
all fail silently-turned-loud (the script now raises with the exact
line number). If a step's real content is one long paragraph with no
natural short/long split, it is completely fine to put the *entire*
paragraph before the em dash and end with a bare `— ✅` — the parser
doesn't require a `(note)` at all, only the trailing marker.

**Known gaps** (optional, appears once, anywhere before the first
milestone section is fine, or right after Top Metrics):
```
**Known gaps:**
- A real, open gap.
```
Only the *first* `**Known gaps:**` in the file is read — unlike "Just
closed"/"Up next" below, this section is not meant to repeat per
milestone. Keep this list to what's *actually still open* — when a
bullet closes, don't strike it through in place; **move** it to "Fixed
gaps" instead (below). A "Known gaps" list that only ever grows with
struck-through history stops answering "what's still open" at a glance,
which is the whole reason this section exists.

**Fixed gaps** (optional, appears once, same placement rule as "Known
gaps" — conventionally written directly after it):
```
**Fixed gaps:**
- ~~A gap that used to be open.~~ **Fixed (M2 Phase 1).** One sentence.
- ~~Investigated, decided not to build.~~ **Resolved (M3 Phase 2): no change needed.**
```
Same bullet grammar "Known gaps" uses (so a bullet's own struck-through
text carries over unchanged when moved) — the published artifact
renders this as its own collapsed-by-default `<details>` section (an
archive, not a status view), so a long, ever-growing history of closed
gaps never crowds out the "what's still open" list above it. Use
`**Fixed (M<N> Phase <N>).**` for something that was actually built,
`**Resolved (M<N> Phase <N>): <why>.**` for a gap that was investigated
and found not to need any change — both are real, distinct outcomes,
worth keeping distinguishable in the historical record.

**"Just closed" / "Up next"** — a pair of bolded labels, written once
at the true tail of the file (after the last milestone section),
summarizing current state:
```
**Just closed:** <what just shipped>. **Up next:** <what's next, or "nothing currently scoped">.
```
The parser takes the **last** occurrence of each label in the whole
file — so if you write a fresh pair at the tail every time a milestone
closes, older pairs earlier in the file (if any exist from an older
convention) are harmless history, not read. **Never let this pair go
stale** — a status/label that's true when written and silently wrong
two milestones later is worse than not writing one, since a reader (or
the published artifact) presents it as current. If genuinely nothing
is scoped next, write that explicitly rather than leaving an outdated
pointer.

## Lessons learned (read before debugging a parse failure)

These are real bugs found in production use of this exact tool, not
hypothetical:

- **A `- Step`/`- Stage` line silently vanished from the rendered
  artifact with zero error.** Root cause: the line was missing its
  trailing `— <icon>` marker (either no marker at all, or the icon
  appeared mid-paragraph instead of at the end, or a `(note)` had an
  unbalanced/missing closing paren). The script used to just skip such
  lines with no signal. **Fixed**: it now raises immediately, naming
  the exact line number, so this can't recur silently. If you ever see
  this script fail with `looks like a Step/Stage item but doesn't
  match ITEM_RE`, the fix is almost always: move the icon to the true
  end of the line, or add the missing closing `)`.
- **A milestone's own `**Status:**` line said "Phase 1/5 done" after
  all 5 phases had actually completed.** The Top Metrics table row had
  been correctly updated every time; the milestone's *own* status line,
  a few lines below its heading, had not. These are two separate places
  that both need updating — see step 1 of "Ongoing maintenance" above.
  **This recurred later on a different real project** — a milestone's
  own `**Status:**` line still read "🚧 In progress — Phase 1 of 6"
  after all 6 phases had shipped and the Top Metrics table already
  showed 100%/Complete, caught only when the user happened to read the
  raw file and asked "why does this say in progress" — proving the
  written-down rule alone, with no mechanical check behind it, was not
  enough to actually prevent the recurrence it was written to prevent.
  Fixed properly this time: step 4 of "Ongoing maintenance workflow"
  above is now a real, runnable audit script (also step 2 of
  "Publishing checklist") that catches this class of drift before
  publishing, not a "remember to reread it" instruction alone.
- **The "Just closed"/"Up next" highlight boxes stayed frozen on old
  text for 20+ milestones -- twice, from two different real causes, not
  the same bug recurring.** First: `re.search` finds the *first* match
  in a file where every closed milestone historically wrote its own
  such pair — meaning it kept finding the oldest one forever. Fixed at
  the time by taking the *last* match instead, on the assumption that
  fresh pairs are appended, so the newest sits closest to the bottom.
  Second, found later (2026-09-21) on a much larger real tracker: that
  assumption was wrong for how this convention is *actually* used — a
  fresh "Just closed"/"Up next" (and "Known gaps"/"Fixed gaps") pair is
  written at the very *top* of the file, right after Top Metrics, every
  time a milestone closes, newest first. A large tracker also
  accumulates many legitimate, *older* "Just closed" mentions deep
  inside individual milestone sections' own historical narrative (a
  step's own transient status note, written while that milestone was
  still being built) — so "last match in the whole file" was silently
  finding one of *those* instead, and had been wrong for over a dozen
  milestones before anyone noticed (caught by diffing the published
  artifact's own highlight box against the real current top-of-file
  text). Fixed properly this time: `parse_narrative` now bounds its
  search to the real *front matter* (everything before the first `##
  Milestone <N>` heading, which is exactly where the newest-first stack
  lives and excludes every embedded historical mention by construction)
  and takes the *first* match within that bound, falling back to the
  original "search the whole file, take the last match" behavior only
  when the front matter has no match at all -- the real, other
  documented convention (a *single* pair at the true tail, after every
  milestone section, `BUILD_TRACKER_TEMPLATE.md`'s own example) has
  nothing in the front matter to find, so the fallback's "last match in
  the whole file" correctly lands on that one pair instead of coming
  back empty. If you're maintaining a fork of the script and see this
  regress again, check that the front-matter bound, the "first match"
  preference within it, and the whole-file fallback are all still
  present together -- removing any one of the three breaks one of the
  two real conventions this now supports.

## Publishing checklist (every update)

1. Confirm "Just closed"/"Up next"/"Known gaps" were all reviewed as
   one unit this update (step 2 of "Ongoing maintenance workflow"
   above) — not just whichever one obviously relates to what changed.
2. Run the Status-line-vs-Top-Metrics audit script (step 4 of "Ongoing
   maintenance workflow" above). It must print nothing.
3. `python3 tools/generate_tracker_artifact.py` — must exit 0 and print
   a milestone/phase/item count that looks right for what you just
   changed (a count that didn't move when you know you added a step is
   itself a signal something didn't parse).
4. Spot-check the specific thing you changed actually rendered: grep
   the generated HTML for the milestone/phase title you touched and
   confirm real text follows it, not an empty `<ul class="steps">`.
5. Publish via the Artifact tool with `url` set to the existing
   artifact's URL.
6. Tell the user what changed and confirm the link still resolves to
   the same place (Claude Code: `/artifacts`; web: the artifacts
   gallery).
