#!/usr/bin/env python3
"""Generates docs/api/rust.md, the Rust quick reference, from rustdoc's JSON.

    for c in engine-core engine-render engine-platform; do
      RUSTC_BOOTSTRAP=1 cargo rustdoc -p $c --lib -- -Z unstable-options --output-format json
    done
    python tools/gen_rust_api.py

It lists the public types, functions and constants each crate re-exports at its
root: for a type, the fields or variants, the public methods, what it
implements (linked), and the events it takes or produces. A description is the
first sentence of the item's doc comment with milestone prefixes removed;
`tools/rust_api_descriptions*.json` (keys like `Tree`, `Tree::insert`,
`Node.visible`, `Key::Enter`) replaces any that read badly. Regenerate when the
public API changes.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = ROOT / "target" / "doc"
OUT = ROOT / "docs" / "api" / "rust.md"
OVERRIDES_GLOB = "rust_api_descriptions*.json"

CRATES = [
    (
        "engine-core",
        "engine_core",
        "The node tree, animation, layout, input dispatch, focus, layers and "
        "docking. Pure Rust: no window, GPU or Python.",
    ),
    (
        "engine-render",
        "engine_render",
        "Scene building and GPU rendering with `vello_gpu` and `wgpu`: the paint "
        "walk, damage tracking, text, images, shaders and GPU health.",
    ),
    (
        "engine-platform",
        "engine_platform",
        "The `winit` event loop, window creation, platform input translation "
        "and the accessibility adapter.",
    ),
]

STD = {
    "Clone": "std/clone/trait.Clone.html",
    "Copy": "std/marker/trait.Copy.html",
    "Debug": "std/fmt/trait.Debug.html",
    "Display": "std/fmt/trait.Display.html",
    "Default": "std/default/trait.Default.html",
    "PartialEq": "std/cmp/trait.PartialEq.html",
    "Eq": "std/cmp/trait.Eq.html",
    "PartialOrd": "std/cmp/trait.PartialOrd.html",
    "Ord": "std/cmp/trait.Ord.html",
    "Hash": "std/hash/trait.Hash.html",
    "From": "std/convert/trait.From.html",
    "Into": "std/convert/trait.Into.html",
    "Deref": "std/ops/trait.Deref.html",
    "DerefMut": "std/ops/trait.DerefMut.html",
    "Error": "std/error/trait.Error.html",
    "Drop": "std/ops/trait.Drop.html",
    "Add": "std/ops/trait.Add.html",
    "Mul": "std/ops/trait.Mul.html",
    "Iterator": "std/iter/trait.Iterator.html",
}
SKIP_TRAITS = {
    "Send", "Sync", "Freeze", "Unpin", "UnwindSafe", "RefUnwindSafe",
    "StructuralPartialEq", "Any", "Borrow", "BorrowMut", "TryFrom", "TryInto", "ToOwned",
}

# Types that are events, and the events other types take or produce.
EVENT_TYPES = {
    "InputEvent": "An input the platform delivers to the tree.",
    "DispatchOutcome": "What the tree did with an input event.",
    "ChangedValue": "A value an input changed.",
    "GpuReport": "What the GPU reported about its own health.",
    "WindowLifecycle": "A window lifecycle change the platform reports.",
}
EVENTS = {
    "Tree": "takes `InputEvent` (`dispatch`) and returns `DispatchOutcome`; scroll, value and focus changes are read back after a dispatch",
    "GpuWatch": "produces `GpuReport` (`poll`)",
    "WindowRenderer": "takes the tracker's `Damage` (`prepare`, then `draw`)",
    "DamageTracker": "produces `Damage` (`damage`)",
    "EventLoopWaker": "wakes an idle loop with a platform event from any thread (`wake`)",
    "WindowOpener": "asks the loop to open a window (`open_window`)",
}

OVERRIDES: dict[str, str] = {}
for _file in sorted((ROOT / "tools").glob(OVERRIDES_GLOB)):
    OVERRIDES.update(json.loads(_file.read_text()))


# --- descriptions ---------------------------------------------------------------

PREFIX = re.compile(
    r"^\s*(?:(?:0\.\d+\.\d+(?:\.\d+)?|M\d+|Milestone \d+|Phase \d+|Step \d+|issue #\d+|#\d+|"
    r"§[\d.]+|review|D\d+)(?:\s+(?:Phase|Step)\s+\d+)*"
    r"(?:\s*\([^)]*\))*\s*[:,;.\-—]*\s*)+",
    re.IGNORECASE,
)


def summary(doc: str | None, limit: int = 150) -> str:
    """The first sentence of a doc comment, milestone prefixes removed."""
    if not doc:
        return ""
    text = re.sub(r"\s+", " ", doc.strip().split("\n\n")[0])
    for _ in range(4):
        stripped = PREFIX.sub("", text, count=1)
        if stripped == text:
            break
        text = stripped
    text = re.sub(r"^\(\d\.\d+\.\d+(?:\.\d+)?\)\s*", "", text)  # a leading "(0.5.0)"
    text = re.sub(r"\[(`[^`]+`)\]\([^)]*\)", r"\1", text)  # [`x`](link) -> `x`
    text = re.sub(r"\[([^\]`]+)\]\([^)]*\)", r"\1", text)  # [x](link) -> x
    text = re.sub(r"\[(`[^`]+`)\]", r"\1", text)  # [`x`] -> `x`
    m = re.search(r"(?<=[a-z0-9`)\]])[.!?](\s|$)", text)
    if m:
        text = text[: m.start() + 1]
    text = text.replace(" -- ", " — ").replace("--", "—")
    head, sep, _ = text.partition(" — ")
    if sep and len(head) >= 25:  # keep a quick reference short: the first clause
        text = head.rstrip(",;:") + "."
    text = text.replace("|", "\\|").strip()
    if len(text) > limit:
        text = text[: limit - 1].rsplit(" ", 1)[0].rstrip(",;:") + "…"
    return text[:1].upper() + text[1:]


# --- type rendering -------------------------------------------------------------


def short(path: str) -> str:
    return path.split("::")[-1] if path else path


def type_str(t) -> str:
    if t is None:
        return "()"
    ((kind, v),) = t.items()
    if kind in ("primitive", "generic"):
        return v
    if kind == "resolved_path":
        base = short(v["path"])
        args = v.get("args")
        if args and "angle_bracketed" in args:
            parts = []
            for a in args["angle_bracketed"]["args"]:
                if "type" in a:
                    parts.append(type_str(a["type"]))
                elif "lifetime" in a:
                    parts.append(a["lifetime"])
                elif "const" in a:
                    parts.append(str(a["const"].get("expr", "_")))
            for c in args["angle_bracketed"].get("constraints", []):
                eq = c.get("binding", {}).get("equality")
                if eq and "type" in eq:
                    parts.append(f'{c["name"]} = {type_str(eq["type"])}')
            if parts:
                base += "<" + ", ".join(parts) + ">"
        elif args and "parenthesized" in args:
            p = args["parenthesized"]
            base += "(" + ", ".join(type_str(i) for i in p["inputs"]) + ")"
            if p.get("output"):
                base += " -> " + type_str(p["output"])
        return base
    if kind == "borrowed_ref":
        lt = f"{v['lifetime']} " if v.get("lifetime") else ""
        return "&" + lt + ("mut " if v["is_mutable"] else "") + type_str(v["type"])
    if kind == "tuple":
        return "(" + ", ".join(type_str(i) for i in v) + ")"
    if kind == "slice":
        return "[" + type_str(v) + "]"
    if kind == "array":
        return "[" + type_str(v["type"]) + "; " + str(v["len"]) + "]"
    if kind == "raw_pointer":
        return ("*mut " if v["is_mutable"] else "*const ") + type_str(v["type"])
    if kind == "impl_trait":
        return "impl " + " + ".join(bound_str(b) for b in v)
    if kind == "dyn_trait":
        return "dyn " + " + ".join(bound_str({"trait_bound": b}) for b in v["traits"])
    if kind == "function_pointer":
        sig = v["sig"]
        ins = ", ".join(type_str(i[1]) for i in sig["inputs"])
        out = f" -> {type_str(sig['output'])}" if sig.get("output") else ""
        return f"fn({ins}){out}"
    if kind == "qualified_path":
        return f"{type_str(v['self_type'])}::{v['name']}"
    return "_"


def bound_str(b) -> str:
    if "trait_bound" in b:
        return type_str({"resolved_path": b["trait_bound"]["trait"]})
    return b.get("outlives", "_")


def generics_str(g) -> str:
    params = []
    for p in g.get("params", []):
        kind = p["kind"]
        if "lifetime" in kind:
            params.append(p["name"])
        elif "type" in kind and not kind["type"].get("is_synthetic"):
            bounds = [bound_str(b) for b in kind["type"].get("bounds", [])]
            params.append(p["name"] + (": " + " + ".join(bounds) if bounds else ""))
    return "<" + ", ".join(params) + ">" if params else ""


def fn_sig(name: str, f) -> str:
    sig = f["sig"]
    args = []
    for pname, ptype in sig["inputs"]:
        ts = type_str(ptype)
        if pname == "self":
            args.append({"&Self": "&self", "&mut Self": "&mut self"}.get(ts, "self"))
        else:
            args.append(f"{pname}: {ts}")
    out = ""
    if sig.get("output") is not None and type_str(sig["output"]) != "()":
        out = " -> " + type_str(sig["output"])
    quals = ("const " if f["header"].get("is_const") else "") + (
        "unsafe " if f["header"].get("is_unsafe") else ""
    )
    return f"{quals}fn {name}{generics_str(f['generics'])}({', '.join(args)}){out}"


# --- the crate model --------------------------------------------------------------


class Crate:
    def __init__(self, label: str, mod: str, blurb: str):
        self.label, self.blurb = label, blurb
        data = json.loads((DOC / f"{mod}.json").read_text())
        self.index = data["index"]
        root = self.index[str(data["root"])]
        self.items: list[tuple[str, dict]] = []
        self.externals: list[tuple[str, str]] = []
        for i in root["inner"]["module"]["items"]:
            it = self.index[str(i)]
            kind = next(iter(it["inner"]))
            if kind == "use":
                u = it["inner"]["use"]
                target = self.index.get(str(u["id"]))
                if target is None:
                    self.externals.append((u["name"], u["source"]))
                else:
                    self.items.append((u["name"], target))
            elif it.get("visibility") == "public" and kind in (
                "struct", "enum", "trait", "function", "constant", "type_alias",
            ):
                self.items.append((it["name"], it))

    def kind(self, it) -> str:
        return next(iter(it["inner"]))

    def impls(self, it):
        inner = it["inner"][self.kind(it)]
        return [self.index[str(i)] for i in inner.get("impls", [])]


def anchor(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def base_name(name: str) -> str:
    return re.split(r"[<(\[&\s]", name.lstrip("&").replace("mut ", ""))[0]


def link_trait(name: str, local: set[str]) -> str:
    base = name.split("<")[0]
    if base in local:
        return f"[`{name}`](#{anchor(base)})"
    if base in STD:
        return f"[`{name}`](https://doc.rust-lang.org/{STD[base]})"
    return f"`{name}`"


def link_type(name: str, local: set[str]) -> str:
    base = base_name(name)
    return f"[`{name}`](#{anchor(base)})" if base in local else f"`{name}`"


def implemented(crate: Crate, it, local: set[str]) -> list[str]:
    out: list[str] = []
    for im in crate.impls(it):
        inner = im["inner"]["impl"]
        tr = inner.get("trait")
        if not tr or inner.get("is_synthetic") or inner.get("blanket_impl"):
            continue
        name = type_str({"resolved_path": tr})
        base = name.split("<")[0]
        if base in SKIP_TRAITS:
            continue
        if base == "Deref":
            target = ""
            for mid in inner["items"]:
                m = crate.index[str(mid)]
                if m["name"] == "Target":
                    target = type_str(m["inner"]["assoc_type"]["type"])
            out.append(
                f"[`Deref`](https://doc.rust-lang.org/{STD['Deref']}) to {link_type(target, local)}"
            )
            continue
        out.append(link_trait(name, local))
    return sorted(set(out), key=out.index)


def methods(crate: Crate, it):
    rows = []
    for im in crate.impls(it):
        inner = im["inner"]["impl"]
        if inner.get("trait"):
            continue
        for mid in inner["items"]:
            m = crate.index[str(mid)]
            if m.get("visibility") == "public" and "function" in m["inner"]:
                rows.append((m["name"], m))
    return sorted(rows, key=lambda r: r[0])


def describe(key: str, item) -> str:
    return OVERRIDES.get(key) or summary(item.get("docs"))


def render_type(crate: Crate, name: str, it, local: set[str], lines: list[str]):
    kind = crate.kind(it)
    lines += [f"### {name} {{ #{anchor(name)} }}", ""]
    lines += [f"*{kind}.* {describe(name, it)}".rstrip(), ""]
    inner = it["inner"][kind]
    if kind == "trait":
        implementors = sorted(
            n
            for n, other in crate.items
            if crate.kind(other) in ("struct", "enum")
            and any(
                (i["inner"]["impl"].get("trait") or {}).get("path", "").endswith(name)
                for i in crate.impls(other)
            )
        )
        if implementors:
            lines += ["**Implemented by:** " + ", ".join(link_type(n, local) for n in implementors), ""]
    else:
        imps = implemented(crate, it, local)
        if imps:
            lines += ["**Implements:** " + ", ".join(imps), ""]
    if name in EVENT_TYPES:
        lines += [f"**Event type.** {EVENT_TYPES[name]}", ""]
    elif name in EVENTS:
        lines += [f"**Events:** {EVENTS[name]}", ""]

    if kind == "struct":
        sk = inner["kind"]
        fields = []
        if "plain" in sk:
            ids = sk["plain"]["fields"]
        elif "tuple" in sk:
            ids = [i for i in sk["tuple"] if i is not None]
        else:
            ids = []
        for n, fid in enumerate(ids):
            f = crate.index[str(fid)]
            if f.get("visibility") == "public":
                fields.append((f.get("name") or str(n), type_str(f["inner"]["struct_field"]), f))
        if fields:
            lines += ["| Field | Type | Description |", "| --- | --- | --- |"]
            for fname, ftype, f in fields:
                lines.append(f"| `{fname}` | `{ftype}` | {describe(f'{name}.{fname}', f)} |")
            lines.append("")
    elif kind == "enum":
        rows = []
        for vid in inner["variants"]:
            v = crate.index[str(vid)]
            vk = v["inner"]["variant"]["kind"]
            payload = ""
            if isinstance(vk, dict) and "tuple" in vk:
                parts = [
                    type_str(crate.index[str(x)]["inner"]["struct_field"])
                    for x in vk["tuple"]
                    if x is not None
                ]
                payload = "(" + ", ".join(parts) + ")"
            elif isinstance(vk, dict) and "struct" in vk:
                parts = []
                for x in vk["struct"]["fields"]:
                    fx = crate.index[str(x)]
                    parts.append(f"{fx['name']}: {type_str(fx['inner']['struct_field'])}")
                payload = "{ " + ", ".join(parts) + " }"
            rows.append((v["name"], payload, describe(f"{name}::{v['name']}", v)))
        if rows:
            head = "Event" if name in EVENT_TYPES else "Variant"
            lines += [f"| {head} | Payload | Description |", "| --- | --- | --- |"]
            for vname, payload, d in rows:
                cell = f"`{payload}`" if payload else ""
                lines.append(f"| `{vname}` | {cell} | {d} |")
            lines.append("")

    ms = methods(crate, it) if kind != "trait" else []
    if kind == "trait":
        for mid in inner["items"]:
            m = crate.index[str(mid)]
            if "function" in m["inner"]:
                ms.append((m["name"], m))
    if ms:
        lines += ["| Method | Description |", "| --- | --- |"]
        for mname, m in ms:
            sig = fn_sig(mname, m["inner"]["function"]).replace("|", "\\|")
            lines.append(f"| `{sig}` | {describe(f'{name}::{mname}', m)} |")
        lines.append("")


def main() -> int:
    crates = []
    for label, mod, blurb in CRATES:
        if not (DOC / f"{mod}.json").exists():
            print(f"missing {DOC / (mod + '.json')}: run rustdoc JSON first", file=sys.stderr)
            return 1
        crates.append(Crate(label, mod, blurb))
    local = {n for c in crates for n, it in c.items if c.kind(it) in ("struct", "enum", "trait")}

    lines = [
        "# Rust API",
        "",
        "<!-- Generated by tools/gen_rust_api.py from rustdoc's JSON; do not edit by hand. -->",
        "",
        "A quick reference to the public Rust API of the engine's crates: every type with its "
        "fields or variants, methods, what it implements, and the events it takes or "
        "produces. The [Python API](python.md) is a thin layer over these. For the design see "
        "[Architecture](../architecture.md); for full signatures and examples build rustdoc "
        "with `cargo doc --workspace --no-deps --open`.",
        "",
        "Rust has no class inheritance. Where a class would list its parents, each type "
        "here lists the **traits it implements** (linked), and a `Deref` to another type "
        "where one exists. Items are the ones each crate re-exports at its root.",
        "",
        "| Crate | Role |",
        "| --- | --- |",
    ]
    for c in crates:
        lines.append(f"| [`{c.label}`](#{anchor(c.label)}) | {c.blurb} |")
    lines += ["| `engine-py` | The PyO3 bindings. Its classes are the [Python API](python.md). |", ""]

    for c in crates:
        lines += [f"## {c.label} {{ #{anchor(c.label)} }}", "", c.blurb, ""]
        order = {"struct": 0, "enum": 1, "trait": 2}
        types = sorted(
            ((n, it) for n, it in c.items if c.kind(it) in order),
            key=lambda x: (order[c.kind(x[1])], x[0]),
        )
        for n, it in types:
            render_type(c, n, it, local, lines)
        funcs = sorted((n, it) for n, it in c.items if c.kind(it) == "function")
        if funcs:
            lines += [f"### Functions ({c.label}) {{ #functions-{anchor(c.label)} }}", "",
                      "| Function | Description |", "| --- | --- |"]
            for n, it in funcs:
                sig = fn_sig(n, it["inner"]["function"]).replace("|", "\\|")
                lines.append(f"| `{sig}` | {describe(n, it)} |")
            lines.append("")
        consts = sorted((n, it) for n, it in c.items if c.kind(it) == "constant")
        if consts:
            lines += [f"### Constants ({c.label}) {{ #constants-{anchor(c.label)} }}", "",
                      "| Constant | Type | Description |", "| --- | --- | --- |"]
            for n, it in consts:
                ty = type_str(it["inner"]["constant"]["type"])
                lines.append(f"| `{n}` | `{ty}` | {describe(n, it)} |")
            lines.append("")
        if c.externals:
            lines += [f"### Re-exports ({c.label}) {{ #reexports-{anchor(c.label)} }}", "",
                      "Types re-exported from other crates:", ""]
            lines += [f"- `{n}` from `{src.split('::')[0]}`" for n, src in c.externals]
            lines.append("")

    OUT.write_text("\n".join(lines).rstrip("\n") + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(lines)} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
