# LOG — Branch `0.3.5`: Milestone 100

- M99 complete and pushed (`1782532`). The user: "push and start M100".

## Done

1. Phase 1 Step 1: every surviving name in its target form except the
   synthetic-input surface; `read_clipboard`/`write_clipboard` and `Painter`
   added; an engine panic on an unresolved `size_hint` list fixed; tests
   migrated by codemod; cargo 326, pytest 443, 15 examples, docs clean.
2. Phase 1 Step 2: the 15 synthetic-input and system-clipboard methods
   removed; terminal keys, the terminal wheel, and Ctrl+C/X/V/A moved into
   `process_input`, so `simulate` covers them; a clipboard that lost what
   the app wrote fixed; cargo 326, pytest 434, 15 examples, docs clean.

## Status

**In progress.** Phase 1 Step 3 (migration page, Tesserae) next.
