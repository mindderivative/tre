#!/usr/bin/env python3
"""M26 Phase 1's real stylesheet cascade & MD3 token resolution, now
reachable from Python for the first time (§16.3): `engine-spec`'s
`Reconciler::load`/`reconcile` already accepted `sheet`/`scheme` --
`engine-py::View` just always passed `None, None`. `View(path,
stylesheet=..., theme_seed=..., dark=...)` now threads both through.

What this script proves automatically (headless-CI-safe, no human
needed, no pixel readback needed):

1. The real cascade precedence (`stylesheet_tokens_sheet.yaml`'s
   baseline -> kind -> classes tiers) actually resolves through the
   real Python `View` API -- read back via `Node.get("corner_radius")`,
   already real and readable, so no new getter is needed to prove this.
2. MD3 token resolution (`background: primary`) is genuinely gated on
   a real scheme being supplied: the identical stylesheet, loaded with
   no `theme_seed`, still fails exactly the way it always has (`"primary"`
   isn't a literal CSS color, and `resolve_color`'s own fallback-to-
   literal-parsing path is unchanged) -- proving this phase didn't
   silently change the no-theme behavior. Loaded *with* a `theme_seed`,
   the same stylesheet now resolves `primary`/`secondary` against a
   real `ColorScheme` and succeeds.
"""

from pathlib import Path

from tre import View

HERE = Path(__file__).parent
VIEW_PATH = str(HERE / "stylesheet_tokens.yaml")
SHEET_PATH = str(HERE / "stylesheet_tokens_sheet.yaml")

# Without a theme, "background: primary" isn't a literal color -- must
# still fail exactly as it did before this phase (the real, unchanged
# fallback-to-literal-parsing path).
try:
    View(VIEW_PATH, stylesheet=SHEET_PATH)
    raise AssertionError(
        "must have raised -- 'primary' isn't a literal color and no theme_seed was given"
    )
except ValueError:
    pass
print("no theme_seed: 'background: primary' correctly fails to parse as a literal color")

# With a real theme, the same stylesheet now resolves for real.
view = View(VIEW_PATH, stylesheet=SHEET_PATH, theme_seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)

plain = view.node("plain")
accented = view.node("accented")

assert plain.get("corner_radius") == 8.0, "the kind: Rect rule must apply (baseline's 4 loses)"
assert accented.get("corner_radius") == 16.0, (
    "the classes: [accent] rule must override the kind: Rect rule (16, not 8)"
)
print(
    f"with theme_seed: plain.corner_radius={plain.get('corner_radius')}, "
    f"accented.corner_radius={accented.get('corner_radius')} -- cascade precedence confirmed"
)

print("stylesheet_tokens.py: exited cleanly, cascade and token resolution both proved")
