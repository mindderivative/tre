# LOG — Branch `0.4.4`: Milestones 21–23

## Status

**Scaffolded (2026-09-30).** Branch `0.4.4` off `main` at `176a7f0`,
versions 0.4.4. User: scope scroll chaining (M21, Tesserae's 0.4.3
observation) and a Windows CI cache that saves (M22) as 0.4.4. Open
question: should keys chain too (M21 Step 2)? Next: M21.

**M21 complete (2026-09-30).** `Tree::can_scroll`; the wheel walk and
`scroll_view_for_key` chain past views that can't move their way. User:
keys chain too. 1 core test (+2 rewritten), 4 pytest cases; guide,
migration 0.4.4, ARCHITECTURE §11.7a. cargo 387, pytest 508 + 1 skipped.
Next: M22.
