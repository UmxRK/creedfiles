#!/usr/bin/env python3
"""Geometry checks on `creed-shell dump-layout` output saved by tests/visual.sh.

For every surface:
  * every named box lies inside its parent box and inside the surface;
  * every clickable region lies inside the surface and has a real size;
  * the bar's centre section is centred on the bar (±1 px);
  * popup panels are fully on screen;
  * no two buttons overlap (except deliberate backdrop / group regions).
"""
import json, pathlib, sys

TOL = 0.75
errors = []


def inside(a, b):
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    return ax >= bx - TOL and ay >= by - TOL and ax + aw <= bx + bw + TOL and ay + ah <= by + bh + TOL


def check(path):
    try:
        surfaces = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError) as e:
        errors.append(f"{path}: unreadable ({e})")
        return
    for s in surfaces:
        kind, (w, h) = s["kind"], s["size"]
        screen = [0, 0, w, h]
        boxes = {b["name"]: b for b in s["boxes"]}
        for b in s["boxes"]:
            if not inside(b["rect"], screen):
                errors.append(f"{path.name}:{kind}: box {b['name']} {b['rect']} off-surface {w}x{h}")
            p = b.get("parent")
            if p and p in boxes and not inside(b["rect"], boxes[p]["rect"]):
                errors.append(f"{path.name}:{kind}: box {b['name']} {b['rect']} escapes parent {p} {boxes[p]['rect']}")
        if "bar" in boxes and "bar.center" in boxes:
            bx, by, bw, bh = boxes["bar"]["rect"]
            cx, cy, cw, ch = boxes["bar.center"]["rect"]
            if bw >= bh and abs((cx + cw / 2) - (bx + bw / 2)) > 1.0:
                errors.append(f"{path.name}: bar centre section off-centre by {(cx + cw / 2) - (bx + bw / 2):.1f}px")
        buttons = []
        for hit in s["hits"]:
            r = hit["rect"]
            if hit["role"] in ("backdrop", "group"):
                continue
            if r[2] < 1 or r[3] < 1:
                errors.append(f"{path.name}:{kind}: zero-size hit '{hit['label']}'")
            if not inside(r, screen):
                errors.append(f"{path.name}:{kind}: hit '{hit['label']}' {r} off-surface")
            if hit["role"] in ("button", "toggle"):
                buttons.append(hit)
        for i, a in enumerate(buttons):
            for b in buttons[i + 1:]:
                ax, ay, aw, ah = a["rect"]
                bx, by, bw, bh = b["rect"]
                ox = min(ax + aw, bx + bw) - max(ax, bx)
                oy = min(ay + ah, by + bh) - max(ay, by)
                if ox > 2 and oy > 2:
                    errors.append(f"{path.name}:{kind}: buttons overlap: '{a['label']}' and '{b['label']}'")


root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "tests/out")
files = sorted(root.glob("*/*.json"))
for f in files:
    check(f)
if errors:
    print("\n".join(sorted(set(errors))))
    print(f"✗ {len(set(errors))} layout problem(s) in {len(files)} dumps")
    sys.exit(1)
print(f"✓ layout geometry ok ({len(files)} dumps)")
