# LOG — Branch `0.3.4`: Milestone 94

- User-directed: "Approved, start M94."

## Done

1. M93 closed: revision 2 of `docs/design/target-api.md` approved, with
   R1–R12 as written.
2. M94 scoped against the source. The engine already receives raw
   pointer, key, text, wheel, theme and resize input, but only five
   dispatch outcomes reach Python. `Key` has 12 keys, and Shift is the
   only modifier. Legacy handlers live in one map keyed
   `(NodeId, EventKind)`, touched in a handful of places. `Text` and
   `Icon` are never hit targets.

## Status

**M94 Phase 1 in progress.**
