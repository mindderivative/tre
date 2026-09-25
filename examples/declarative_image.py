#!/usr/bin/env python3
"""M22 Phase 2's real declarative image support (§16.1): `kind: Image`
is now a real, loadable `view.yaml` widget kind -- `image.src:` (a path
relative to the owning `view.yaml` file, confined the same
canonicalization-based way `include:` already confines its own paths)
plus an optional `image.fit:` (`Cover`/`Contain`/`Fill`, default
`Fill`) map onto the identical real `NodeKind::Image`/`ContentFit`
`Window.add_image` already builds imperatively (Phase 1) -- both
authoring paths construct the exact same real engine state.

What this script proves automatically (headless-CI-safe, no human
needed): a real `view.yaml` with a `kind: Image` widget parses,
resolves its own `src:` against the view file's real directory, reads
and decodes a real PNG, and the resulting node is genuinely reachable
through `View.node()` -- the same "FFI wiring only" proof `view_
composition.py` already establishes for `include:`; the definitive
pixel-level proof that `fit:` genuinely changes the real painted
geometry is `crates/engine-render/tests/image_paint.rs`'s own content-
fit tests, not this script.

Generates its own tiny real PNG plus a temporary `view.yaml` next to
it at runtime, the same dependency-free, no-checked-in-binary-fixture
approach `examples/image.py`/`tests/test_image.py` already use --
`view.yaml`/`.png` are normally checked-in siblings (`view_
composition.yaml`/`confirm_dialog.yaml`), but a real image binary
asset in this project's own `examples/` directory would be the first
one, so this generates both at runtime instead.
"""

import base64
import tempfile
from pathlib import Path

from tre import View

# The identical real, tiny 4x4 PNG `examples/image.py` already embeds.
_TINY_PNG_B64 = (
    "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAT0lEQVR4nAFEALv/"
    "APRDNv//mAD//+s7/0yvUP8BALzU/yHaHwAeu8IAXdb7AADpHmP/eVVI/2B9i/8A"
    "AACAAf////8AwggAjAJDANx3bQC9UiChZNHhkwAAAABJRU5ErkJggg=="
)

tmp_dir = Path(tempfile.mkdtemp(prefix="tre_declarative_image_example_"))
(tmp_dir / "logo.png").write_bytes(base64.b64decode(_TINY_PNG_B64))
(tmp_dir / "view.yaml").write_text(
    """
id: root
kind: Container
style: {width: 220, height: 220, padding: 20}
children:
  - id: logo
    kind: Image
    image: {src: logo.png, fit: cover}
    style: {width: 160, height: 160}
"""
)

view = View(str(tmp_dir / "view.yaml"))
logo = view.node("logo")
print(f"declarative Image node reachable: {logo is not None}")
assert logo is not None

print("declarative_image.py: exited cleanly, kind: Image loaded and built correctly")
