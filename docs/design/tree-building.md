# Building trees: `build`, `apply`, `set_children`

Work for issue #253 (0.5.10), from the Tesserae Shell's request TRE-REQ-009. Medians, release build, llvmpipe,
detached nodes, Python 3.14.

## Measured first

| Operation | Cost |
| --- | --- |
| `Window.create("box")` | 0.6 µs |
| `Node.set`, one property | 0.33 µs |
| `add_child`, per call, at 1,000 / 5,000 / 20,000 siblings | 0.63 / 2.76 / 10.93 µs before; **0.13 / 0.09 / 0.12 µs after** |
| a 31-node bar | 50 µs |
| a 3,100-node tree | 5-6 ms |
| 1,000 `set(text=)` then one frame | 0.42 ms |

**`add_child` was linear in the sibling count**, so N appends were quadratic: `content_len` walked every child, `try_add_child`
scanned `contains`, and `insert_child` walked again. It now counts the open layers (a handful, always last) instead,
and a child with another parent is the only one that needs a scan.

**Layout already runs once per frame, not once per `set`**: a thousand sets and one frame cost 0.42 ms. So there is no
`batch()` context manager and "one layout pass" comes free.

## What was built

`Window.create` is `prepare_node` (every check and parse, no side effect) then `make_node`; `Node.set` is `prepare_set` then
`run_set`. `build` prepares the whole nested description first and makes the nodes only when every one is valid
(terminals last), so a bad spec makes nothing, calls no canvas `draw` and starts no shell; a failure while making raises
what `create` would and frees the rest. `apply` prepares every pair before applying any, with at most one `shader`
entry (the cycle check reads the tree as it is). `set_children` moves the listed nodes first and then detaches the
dropped ones, so a dropped child holding a kept one is collected.

## Honest speed

`build` of 3,100 nodes took 5.3 ms against 4.9 ms of per-node calls, and `apply` of 1,000 entries 0.33 ms against 0.33 ms
of `set`. **They save Python-to-Rust calls and lines, not time.** The reason to use them is the all-or-nothing check.
`tools/bench_tree_build.py` reproduces the numbers by hand; CI asserts none of them.
