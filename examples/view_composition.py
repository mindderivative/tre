#!/usr/bin/env python3
"""M19 Phase 2's real view composition via `include:` (§16.6):
`view_composition.yaml` includes `confirm_dialog.yaml` as an ordinary
child, splicing it in during loading, before validation -- the
included widget's own `id` is reachable through `View.node()` exactly
like an inline one, no separate "included" concept persisting at
runtime.

`WidgetSpec`'s own real `deny_unknown_fields` validation runs on the
*fully expanded* tree (`engine_spec::include::parse_view_with_
includes`), not on `confirm_dialog.yaml` in isolation -- a typo inside
the included file is caught the same "load-time error with a line
number" way a typo in the main file already is.

What this script proves automatically (headless-CI-safe, no human
needed): the included widget (`confirm`, declared in a real second
file) is a genuine, reachable node in the loaded `View`'s own tree,
alongside `header`, which was declared inline in the main file.
"""

from pathlib import Path

from tre import View

view = View(str(Path(__file__).parent / "view_composition.yaml"))

header = view.node("header")
confirm = view.node("confirm")
print(f"header node reachable: {header is not None}")
print(f"included confirm node reachable: {confirm is not None}")
assert header is not None
assert confirm is not None

print("view_composition.py: exited cleanly, a real include spliced in correctly")
