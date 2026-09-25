#!/usr/bin/env python3
"""Migrate `tre` declarative YAML (views, stylesheets, themes) to 0.3.3's
property names (M90).

Rewrites files in place, preserving comments and formatting -- it edits
lines, it doesn't round-trip YAML through a parser. Renames:

- enum values to lowercase snake_case: `flex_direction`, `fit`,
  `align_items`, `justify_content` (`Horizontal` -> `horizontal`,
  `FlexStart` -> `flex_start`, `Cover` -> `cover`)
- `text: {role: ...}` -> `text: {typography_role: ...}`
- on `Text`/`Link`/`Icon`/`LoadingIndicator` widgets: `background` ->
  `foreground` (their paint color is their glyph/text; they have no fill)
- on `Switch`/`RadioButton` widgets: `checked:` -> `selected:`, including
  a `checked` binding key and `two_way: checked`
- on `Slider` widgets: a `thumb_position` binding key and
  `two_way: thumb_position` -> `value`

Widget-dependent renames work per widget block: a block starts at a line
beginning a widget mapping (`id:`, `- id:`, `- kind:`, `- include:`) and
runs to the next one. A widget whose `kind:` is written after a nested
`children:` list isn't recognized -- review the diff.

Usage:  python tools/migrate_views_0_3_3.py FILE_OR_DIR [...]
        (directories are searched for *.yaml / *.yml)
Prints each changed file; `--check` reports without writing.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ENUM_KEYS = ("flex_direction", "fit", "align_items", "justify_content")
GLYPH_KINDS = {"Text", "Link", "Icon", "LoadingIndicator"}
SELECTED_KINDS = {"Switch", "RadioButton"}

WIDGET_START = re.compile(r"^\s*(?:-\s+)?(?:id|kind|include)\s*:|^id\s*:")
KIND = re.compile(r"\bkind\s*:\s*[\"']?([A-Za-z]+)")


def snake(value: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", value).lower()


def migrate_enums(line: str) -> str:
    for key in ENUM_KEYS:
        line = re.sub(
            rf"(\b{key}\s*:\s*[\"']?)([A-Z][A-Za-z]*)",
            lambda m: m.group(1) + snake(m.group(2)),
            line,
        )
    return line


def migrate_role(line: str) -> str:
    # `role:` only ever appears inside a `text:` block in the view schema.
    return re.sub(r"(?<![\w.])role(\s*:)", r"typography_role\1", line)


def migrate_block(lines: list[str]) -> list[str]:
    kinds = {m.group(1) for line in lines for m in [KIND.search(line)] if m}
    out = []
    for line in lines:
        if kinds & GLYPH_KINDS:
            line = re.sub(r"(?<![\w.])background(\s*:)", r"foreground\1", line)
        if kinds & SELECTED_KINDS:
            line = re.sub(r"(?<![\w.])checked(\s*:)", r"selected\1", line)
            line = re.sub(r"(\btwo_way\s*:\s*[\"']?)checked\b", r"\1selected", line)
        if "Slider" in kinds:
            line = re.sub(r"(?<![\w.])thumb_position(\s*:)", r"value\1", line)
            line = re.sub(r"(\btwo_way\s*:\s*[\"']?)thumb_position\b", r"\1value", line)
        out.append(line)
    return out


def migrate_text(text: str) -> str:
    lines = [migrate_role(migrate_enums(line)) for line in text.splitlines(keepends=True)]
    blocks: list[list[str]] = [[]]
    for line in lines:
        if WIDGET_START.match(line) and blocks[-1]:
            blocks.append([])
        blocks[-1].append(line)
    return "".join(line for block in blocks for line in migrate_block(block))


def main(argv: list[str]) -> int:
    check = "--check" in argv
    targets = [a for a in argv if a != "--check"]
    if not targets:
        print(__doc__)
        return 2
    files: list[Path] = []
    for target in map(Path, targets):
        if target.is_dir():
            files += sorted(target.rglob("*.yaml")) + sorted(target.rglob("*.yml"))
        else:
            files.append(target)
    changed = 0
    for path in files:
        before = path.read_text()
        after = migrate_text(before)
        if after != before:
            changed += 1
            print(("would migrate " if check else "migrated ") + str(path))
            if not check:
                path.write_text(after)
    print(f"{changed} of {len(files)} file(s) {'need' if check else 'got'} changes")
    return 1 if check and changed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
