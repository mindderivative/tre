"""Finish the engine-md3 handover and write its page.

`crates/engine-md3/tests/handover.rs` writes the engine's half of
`docs/design/md3-handover.json` (colour science, scales, icons, curves,
loading shapes). This adds the MD3 values that live in `engine-py` --
the no-theme baseline colours, the shipped default theme, and the
container transform's defaults -- read from their source files, then
writes `docs/design/md3-handover.md` from the whole file.

    UPDATE_HANDOVER=1 cargo test -p engine-md3 --test handover
    python tools/md3_handover.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
HANDOVER_JSON = ROOT / "docs" / "design" / "md3-handover.json"
HANDOVER_MD = ROOT / "docs" / "design" / "md3-handover.md"
FACTORY_RS = ROOT / "crates" / "engine-py" / "src" / "window_factory.rs"
DEFAULT_THEME = ROOT / "crates" / "engine-py" / "assets" / "default_theme.yaml"
WINDOW_INPUT_RS = ROOT / "crates" / "engine-py" / "src" / "window_input.rs"
PY_KEYS = ("baseline", "default_theme", "container_transform")


def baseline() -> dict[str, str]:
    """`Md3Baseline`'s colours -- what a factory used with no theme set."""
    source = FACTORY_RS.read_text()
    block = source[source.index("impl Md3Baseline {") :]
    block = block[: block.index("\n}\n")]
    pattern = (
        r"const (\w+): Color = "
        r"Color::from_rgba8\((0x\w+), (0x\w+), (0x\w+), 0x\w+\)"
    )
    return {
        name.lower(): "#{:02X}{:02X}{:02X}".format(*(int(v, 16) for v in rgb))
        for name, *rgb in re.findall(pattern, block)
    }


def default_theme() -> dict[str, Any]:
    """The shipped `default_theme.yaml`'s component overrides and per-kind
    corner radii. Its flow-mapping lines are simple enough to read without
    a YAML parser, which `tre`'s test dependencies don't include."""
    styles, components = DEFAULT_THEME.read_text().split("\ncomponents:\n")
    kind_pattern = r"- kind: (\w+)\n\s+style: \{corner_radius: ([\d.]+)\}"
    entries: dict[str, dict[str, float]] = {}
    for key, body in re.findall(r"^  ([\w.]+): \{(.*)\}$", components, re.M):
        pairs = (pair.split(":") for pair in body.split(","))
        entries[key] = {k.strip(): float(v) for k, v in pairs}
    return {
        "components": entries,
        "kind_corner_radius": {k: float(v) for k, v in re.findall(kind_pattern, styles)},
    }


def container_transform() -> dict[str, Any]:
    match = re.search(r"duration_ms=(\d+), content_stagger_ms=(\d+)", WINDOW_INPUT_RS.read_text())
    assert match, "begin_container_transform's signature moved"
    return {
        "duration_ms": int(match[1]),
        "content_stagger_ms": int(match[2]),
        "easing": "emphasized",
    }


def handover() -> dict[str, Any]:
    data: dict[str, Any] = json.loads(HANDOVER_JSON.read_text())
    data["baseline"] = baseline()
    data["default_theme"] = default_theme()
    data["container_transform"] = container_transform()
    return data


# --- The page --------------------------------------------------------------


def _table(header: list[str], rows: list[list[str]]) -> list[str]:
    lines = ["| " + " | ".join(header) + " |", "|" + " --- |" * len(header)]
    lines += ["| " + " | ".join(row) + " |" for row in rows]
    return lines + [""]


def _num(value: float | None) -> str:
    return "" if value is None else f"{value:g}"


def page(data: dict[str, Any]) -> str:
    color = data["color"]
    base_seed = "#6750A4"
    light = color["seeds"][base_seed]["light"]
    # The JSON's keys are sorted; the page lists each set in its own order.
    baseline_rows = [
        [f"`{role}`", f"`{data['baseline'][role]}`", f"`{light[role]}`"]
        + ["yes" if light[role] == data["baseline"][role] else ""]
        for role in color["roles"]
        if role in data["baseline"]
    ]
    type_roles = [
        f"{group}_{size}"
        for group in ("display", "headline", "title", "body", "label")
        for size in ("large", "medium", "small")
    ]
    type_rows = [
        [f"`{role}`", data["typography"][role]["font_family"]]
        + [_num(data["typography"][role][k]) for k in ("font_weight", "font_size", "line_height")]
        for role in type_roles
    ]
    component_rows = [
        [f"`{key}`", _num(v.get("corner_radius")), _num(v.get("elevation"))]
        for key, v in data["default_theme"]["components"].items()
    ]
    curve_rows = [
        [f"`{name}`", f"`{tuple(c['cubic_bezier'])}`"]
        for name, c in data["motion"]["curves"].items()
        if "cubic_bezier" in c
    ]
    segments = data["motion"]["curves"]["emphasized"]["segments"]
    loading = data["loading_indicator"]
    transform = data["container_transform"]
    kinds = data["default_theme"]["kind_corner_radius"]
    icons = data["icons"]

    def listing(scale: dict[str, float]) -> str:
        ordered = sorted(scale.items(), key=lambda item: (item[1], item[0]))
        return ", ".join(f"`{k}` {v:g}" for k, v in ordered)

    lines = [
        "# MD3 handover",
        "",
        "<!-- Generated by tools/md3_handover.py; don't edit. -->",
        "",
        "Everything Material Design 3 that `tre` holds and 0.3.5 removes, as data a",
        "framework can rebuild it from and check the rebuild against. The values are",
        "in [`md3-handover.json`](md3-handover.json), generated from the code itself:",
        "`crates/engine-md3/tests/handover.rs` writes the engine's half and fails when",
        "it's stale, and `tools/md3_handover.py` adds the rest and writes this page.",
        "",
        "## Colour",
        "",
        f"`tre` builds a theme with {color['library']}: the",
        f"`{color['variant']}` variant at contrast `{color['contrast']:g}`, one light and one",
        f"dark scheme per seed, each answering {len(color['roles'])} roles. A theme's `dark`",
        "flag picks the scheme; its `colors:` overrides replace roles in both.",
        "",
        "Rebuilding it in Python means a port of material-color-utilities run with the",
        "same variant and contrast. The JSON's `color.seeds` holds the full light and",
        "dark schemes, and the six tonal palettes at the tones in `color.tones`, for",
        "four seeds — " + ", ".join(f"`{s}`" for s in color["seeds"]) + " — so a port",
        "is checked across hues, not one colour. Match those, and it matches `tre`.",
        "",
        "### Baseline colours",
        "",
        "Until a theme was set, a factory used fixed baseline colours. They are MD3's",
        "published baseline, not the scheme `tre` builds from the baseline seed",
        f"`{base_seed}`, and most roles differ:",
        "",
    ]
    header = ["Role", "No theme", f"Built from `{base_seed}`, light", "Same"]
    lines += _table(header, baseline_rows)
    lines += [
        "## Typography",
        "",
        "The type scale. `line_height` is a multiple of the font size, the way `text`",
        "takes it. There's no letter spacing: `tre` never applied MD3's tracking.",
        "",
    ]
    lines += _table(["Role", "Family", "Weight", "Size", "Line height"], type_rows)
    lines += [
        "## Shape and elevation",
        "",
        f"Corner radii: {listing(data['shape'])}.",
        f"Elevation levels: {listing(data['elevation'])} — each level's shadows are on",
        "[Legacy widget behavior](legacy-behavior.md#elevation).",
        "",
        "### The shipped default theme",
        "",
        "Corner radius and elevation per component, keyed `component` or",
        "`component.variant`; a variant's value wins over its component's.",
        "",
    ]
    lines += _table(["Key", "Corner radius", "Elevation"], component_rows)
    lines += [
        f"And corner radius per node kind: {listing(kinds)}.",
        "",
        "## Icons",
        "",
        f"{len(icons['paths'])} icons, each an SVG path string in Material Symbols' view",
        f"box `{tuple(icons['view_box'])}`, in the JSON's `icons.paths`:",
        ", ".join(f"`{name}`" for name in icons["paths"]) + ". As a node:",
        "",
        "```python",
        'window.create("path", data=paths["home"], view_box=(0, -960, 960, 960),',
        "              width=24, height=24, fill=on_surface)",
        "```",
        "",
        "## Motion",
        "",
        "The named easing curves, as the control points `animate`'s `easing` takes:",
        "",
    ]
    lines += _table(["Curve", "`easing`"], curve_rows)
    emphasized = "; ".join(f"`{[tuple(p) for p in segment]}`" for segment in segments)
    lines += [
        f"`emphasized` is two segments joined at `(1/6, 0.4)` — {emphasized} —",
        "so no single tuple is it. The JSON's `motion.curves` also samples every",
        "curve at `motion.sample_times`. `tre` has no duration scale; the durations",
        "its widgets used are on [Legacy widget behavior](legacy-behavior.md).",
        "",
        "### Container transform",
        "",
        "`begin_container_transform(trigger, destination)` morphs one box into another",
        f"over `{transform['duration_ms']}` ms, `{transform['easing']}`. The destination",
        "starts scaled and moved onto the trigger's bounds, with the trigger's corner",
        "radius and fill and no elevation, and animates to its own; the trigger's",
        "children fade out over the same time, and the destination's fade in from `0`,",
        f"starting `{transform['content_stagger_ms']}` ms later. Rebuilt with `animate` on",
        "`translate_x`/`translate_y`/`scale`, `corner_radius`, `fill`, `shadows`, and",
        "each child's `opacity`.",
        "",
        "## Loading indicator",
        "",
        f"Four shapes in the view box `{tuple(loading['view_box'])}` — they scale",
        f"linearly — morphed in order, `{loading['step_ms']}` ms each, `{loading['easing']}`,",
        "forever. A `path` morphs its `data` from one outline to the next.",
        "",
        "**What `tre` draws isn't quite what it defines.** A morph shape keeps only",
        "each path element's end point, so the pill and the oval — defined with",
        "curves — are drawn as the polygons through their curves' ends: diamonds.",
        "`shapes` is what's drawn; `intended` is the pill and the oval as defined.",
        "",
    ]
    loading_rows = [
        [f"`{n}`", f"`{loading['shapes'][n]}`", f"`{loading['intended'][n]}`" if n in loading["intended"] else "the same"]
        for n in loading["order"]
    ]
    lines += _table(["Shape", "Drawn (`shapes`)", "Defined (`intended`)"], loading_rows)
    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    data = handover()
    HANDOVER_JSON.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    HANDOVER_MD.write_text(page(data))
    print(f"wrote {HANDOVER_JSON.relative_to(ROOT)} and {HANDOVER_MD.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
