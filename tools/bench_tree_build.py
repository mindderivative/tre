#!/usr/bin/env python3
"""Times building a tree through the Python API: medians, release build, detached nodes.

    python tools/bench_tree_build.py            # append scaling, create/set, bar and tree builds
    tools/with_adapter.sh lvp -- python tools/bench_tree_build.py

Run by hand (no assertion: CI runners are too noisy for one). Numbers belong in the issue and LOG.
"""

import statistics as st
import time

from tre import Window

pc = time.perf_counter


def med(f, reps=7):
    xs = []
    for _ in range(reps):
        t = pc()
        f()
        xs.append(pc() - t)
    return st.median(xs)


w = Window(width=800, height=600)

for n in (1000, 5000, 20000):

    def run(n=n):
        p = w.create("box")
        kids = [w.create("box") for _ in range(n)]
        t = pc()
        for k in kids:
            p.add_child(k)
        return (pc() - t) / n * 1e6

    print(f"add_child, {n:5d} siblings: {st.median(run() for _ in range(5)):6.2f} us/call")

print(f"create box:  {med(lambda: [w.create('box') for _ in range(2000)]) / 2000 * 1e6:.2f} us")
node = w.create("box")
print(f"set 1 prop:  {med(lambda: [node.set(opacity=0.5) for _ in range(2000)]) / 2000 * 1e6:.2f} us")


def bar():
    b = w.create("box", display="flex", flex_direction="horizontal", gap=2)
    for i in range(30):
        b.add_child(w.create("text", text=f"item {i}", font_size=14, width=40, height=16))
    return b


def tree():
    r = w.create("box")
    for _ in range(100):
        r.add_child(bar())
    return r


print(f"31-node bar:     {med(lambda: [bar() for _ in range(50)]) / 50 * 1e6:.1f} us")
print(f"3100-node tree:  {med(tree, 9) * 1e3:.2f} ms")
