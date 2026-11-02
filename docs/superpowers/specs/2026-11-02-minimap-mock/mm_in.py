"""Minimap mockups, round 2: close up (A, chosen) indoors. Drawn over the game's own
frames: your flat on 18F of the Velform Group Plaza megablock, and 2F of the Hextek
Holdings Plaza office, Miracorp Tattoo tracked from both (the walk goes by the lift).

    python3 -I mm_in.py <shots dir> <out.html>

The shots dir holds indoor_flat.json and indoor_office.json from the throwaway
tmp_minimap_indoor_dump shot test, and minimap_dump.json (the street) for mm.py.

1 · the floor plan: the floor in close up, 2 tiles a pixel (a floor is drawn three
    times its street footprint), rooms as blocks, corridors as the streets between.
2 · the street outside: the street close up stays, centred on the building, the
    building lit and you placed in it; the walk from its door.
3 · the floor plan and the shaft: 1, with the building's floors in a strip at the
    right; yours, the one the walk wants, the lift between them in cyan."""

import html
import json
import os
import sys
from collections import deque

HERE = os.path.dirname(os.path.abspath(__file__))
S = sys.argv[1]
OUT = sys.argv[2]
sys.argv = [sys.argv[0], S]
sys.path.insert(0, HERE)
import mm  # noqa: E402
import term  # noqa: E402
from mm import CYAN, DIM, FAINT, GREEN, PANEL, PH, PW, PX, PY, CROW, mix, scale  # noqa: E402
from cells import Grid  # noqa: E402

WHITE = "#e6e6f0"
VOID = "#05070b"
SEL = "#14202c"


def level_name(lv):
    return "L" if lv == 0 else ("B%d" % -lv if lv < 0 else "%dF" % lv)


class Place:
    def __init__(self, name):
        self.J = json.load(open(os.path.join(S, name + ".json")))
        J = self.J
        self.W, self.H = J["w"], J["h"]
        self.T = J["terrain"]
        self.fp = J["fp"]
        self.player = tuple(J["player"])
        self.path = [tuple(p) for p in J["path"]]
        cx, cy = self.fp[0] + self.fp[2] // 2, self.fp[1] + self.fp[3] // 2
        self.district = mm.district(cx, cy)
        self.room_at = {}
        for i, r in enumerate(J["rooms"]):
            x, y, w, h = r["rect"]
            for yy in range(y, y + h):
                for xx in range(x, x + w):
                    self.room_at[(xx, yy)] = r

    def frame(self):
        g = Grid(120, 40)
        for y, row in enumerate(self.J["cells"]):
            for x, (ch, fg, bg, bold) in enumerate(row):
                g.c[y][x] = [ch or " ", fg or "#c8d0dc", bg or mm.BG, bold]
        return g

    def ter(self, x, y):
        return self.T[y][x] if 0 <= x < self.W and 0 <= y < self.H else None

    def colours(self):
        d = self.district
        roof, road = d["roof"], d["road"]
        return {
            "wall": mix(PANEL, roof, 0.3),
            "corridor": scale(road, 2.1),
            "room": scale(roof, 1.1),
            "open plan": scale(roof, 1.25),
            "home": mix(scale(roof, 1.1), GREEN, 0.22),
            "rail": mix(roof, WHITE, 0.25),
        }

    def shade(self, x0, y0, s):
        c = self.colours()
        n = wall = atrium = rail = 0
        kinds = {}
        for y in range(y0, y0 + s):
            for x in range(x0, x0 + s):
                t = self.ter(x, y)
                if t is None or t == " ":
                    continue
                n += 1
                if t in "WN#":
                    wall += 1
                elif t == "A":
                    atrium += 1
                elif t == "R":
                    rail += 1
                else:
                    r = self.room_at.get((x, y))
                    k = r["kind"] if r else "corridor"
                    kinds[k] = kinds.get(k, 0) + 1
        if n == 0:
            return PANEL
        if rail:
            return c["rail"]
        if atrium * 2 >= n:
            return VOID
        if wall * s >= n:
            return c["wall"]
        k = max(kinds, key=kinds.get) if kinds else "corridor"
        return c.get(k, c["room"])

    def room_name(self):
        r = self.room_at.get(self.player)
        if not r:
            return "corridor"
        return r["kind"] + (" %d" % r["number"] if r["number"] else "")


def floor_view(p, x, w):
    """A View over the floor: the whole floor centred, at 2 tiles a pixel or 3 when 2
    is too big; a floor too big for 3 follows you."""
    h = PH
    s = 2 if p.W <= 2 * w and p.H <= 4 * h else 3
    v = mm.View(x, PY, w, h, s, 0, 0)

    def axis(size, cells, at):
        span = cells * s
        if size <= span:
            return -(span - size) // 2
        return max(0, min(size - span, at - span // 2))
    v.ox = axis(p.W, w, p.player[0])
    v.oy = axis(p.H, 2 * h, p.player[1])
    return v


def draw_floor(g, p, v):
    px = [[p.shade(v.ox + i * v.s, v.oy + j * v.s, v.s) for i in range(v.w)]
          for j in range(2 * v.h)]
    lit = {v.px(t) for t in p.path}
    for (i, j) in lit:
        if 0 <= i < v.w and 0 <= j < 2 * v.h:
            px[j][i] = mix(px[j][i], CYAN, 0.75)
    mm.paint(g, v, px)
    J = p.J
    mm.mark(g, v, tuple(J["stair"]), "≡", DIM)
    if J["lift"]:
        mm.mark(g, v, tuple(J["lift"]), "⇅", WHITE)
    for r in J["rooms"]:
        if r["kind"] == "home" and p.room_at.get(p.player) is not r and r["door"]:
            mm.mark(g, v, tuple(r["door"]), "⌂", GREEN)
    mm.mark(g, v, p.player, "@", "#ffffff")


def floor_row(g, p, right_w=None):
    lv = level_name(p.J["level"])
    mm.row(g, CROW, [(" " + lv, WHITE, True), (" · " + p.room_name(), DIM, False)],
           [("⌖ ", CYAN, False), ("lift to L", DIM, False)])


def floor_plan(p):
    g = p.frame()
    g.fill(PX, PY, PW, PH, PANEL)
    v = floor_view(p, PX, PW)
    draw_floor(g, p, v)
    floor_row(g, p)
    return g


def walk_street(a, b):
    ok = set("rsapfk^g")
    prev = {a: None}
    q = deque([a])
    while q:
        c = q.popleft()
        if c == b:
            break
        for n in ((c[0] + 1, c[1]), (c[0] - 1, c[1]), (c[0], c[1] + 1), (c[0], c[1] - 1)):
            if n not in prev and (mm.ter(*n) in ok or n == b):
                prev[n] = c
                q.append(n)
    out, c = [], b
    while c is not None and c in prev:
        out.append(c)
        c = prev[c]
    return out[::-1]


MIRACORP = next(tuple(l["tile"]) for l in mm.J["listings"] if l["name"] == "Miracorp Tattoo")


def street_outside(p):
    g = p.frame()
    fx, fy, fw, fh = p.fp
    centre = (fx + fw // 2, fy + fh // 2)
    v = mm.View.centred(PX, PY, PW, PH, 3, centre)
    px = v.pixels()
    for j in range(2 * v.h):
        for i in range(v.w):
            x0, y0 = v.ox + i * v.s, v.oy + j * v.s
            if fx <= x0 + 1 < fx + fw and fy <= y0 + 1 < fy + fh:
                px[j][i] = mix(px[j][i], WHITE, 0.32)
    door = tuple(p.J["standing"])
    out = walk_street(door, MIRACORP)
    for t in out:
        q = v.px(t)
        if 0 <= q[0] < v.w and 0 <= q[1] < 2 * v.h:
            px[q[1]][q[0]] = mix(px[q[1]][q[0]], CYAN, 0.75)
    mm.paint(g, v, px)
    mm.mark(g, v, MIRACORP, "+", "#ff4f5e") or mm.edge_arrow(g, v, MIRACORP, "+", "#ff4f5e")
    you = (fx + p.player[0] // 3, fy + p.player[1] // 3)
    mm.mark(g, v, you, "@", "#ffffff")
    lv = level_name(p.J["level"])
    mm.row(g, CROW, [(" " + lv, WHITE, True), (" · " + p.J["name"], scale(p.district["accent"], 0.85), False)],
           [("+ ", "#ff4f5e", False), ("61 m ↗", DIM, False)])
    return g


def stack_rows(p, n):
    top, low, you = p.J["top"], p.J["lowest"], p.J["level"]
    want = 0
    for r in range(2, -1, -1):
        keep = {top, low} | set(range(you - r, you + r + 1)) | {want - 1, want, want + 1}
        keep = sorted({k for k in keep if low <= k <= top}, reverse=True)
        rows = []
        for a, b in zip(keep, keep[1:] + [None]):
            rows.append(a)
            if b is not None and a - b > 1:
                rows.append(None)
        if len(rows) <= n:
            return rows
    return rows[:n]


def floor_and_shaft(p):
    g = p.frame()
    g.fill(PX, PY, PW, PH, PANEL)
    sw = 8
    v = floor_view(p, PX, PW - sw)
    draw_floor(g, p, v)
    x0 = PX + PW - sw
    rows = stack_rows(p, PH)
    you, want = p.J["level"], 0
    top_y = PY + (PH - len(rows)) // 2
    iy = {lv: top_y + i for i, lv in enumerate(rows)}
    lo, hi = sorted((iy[you], iy[want]))
    for i, lv in enumerate(rows):
        y = top_y + i
        g.put(x0, y, "│", FAINT, PANEL)
        lit = lo < y < hi
        if lv is None:
            g.put(x0 + 2, y, "  ┊ ", FAINT, PANEL)
            if lit:
                g.put(x0 + 6, y, "┃", CYAN, PANEL, True)
            continue
        name = level_name(lv)
        if lv == you:
            g.fill(x0 + 1, y, sw - 1, 1, SEL)
            g.put(x0 + 1, y, name.rjust(4), WHITE, SEL, True)
            g.put(x0 + 6, y, "@", "#ffffff", SEL, True)
        elif lv == want:
            g.put(x0 + 1, y, name.rjust(4), CYAN, PANEL, True)
            g.put(x0 + 6, y, "⌖", CYAN, PANEL, True)
        else:
            g.put(x0 + 1, y, name.rjust(4), DIM, PANEL)
            g.put(x0 + 6, y, "┃" if lit else "│", CYAN if lit else FAINT, PANEL, lit)
    floor_row(g, p)
    return g


# ---------------------------------------------------------------- the page

PLACES = [("indoor_flat", "Your flat · 18F of B1–27F · Velform Group Plaza (megablock)"),
          ("indoor_office", "An office · 2F of B1–14F · Hextek Holdings Plaza")]

CONCEPTS = [
    ("now", "Now", None),
    ("1", "1 · The floor plan", floor_plan),
    ("2", "2 · The street outside", street_outside),
    ("3", "3 · Floor plan + the shaft", floor_and_shaft),
]

TEXT = {
    "1": "The floor you are on, whole: 2 tiles a pixel, or 3 for a floor too big for 2 (the "
         "megablock's 60×60; a floor is drawn three times its street footprint). Rooms "
         "are blocks, corridors the streets between them, the atrium's drop a dark pit inside "
         "its railing. ⇅ the lift, ≡ the stairs, ⌂ your flat from outside it, the walk to the "
         "lift in cyan.",
    "2": "Indoors changes nothing: the street close up stays, centred on your building, which "
         "is lit; you are placed inside its footprint. The walk shows from its door on. You "
         "keep your bearings in the city, but the floor itself isn't drawn.",
    "3": "1 with the building's floors in a strip at the right, the way a lift panel shows "
         "them: the top and bottom, the floors around yours, the one the walk wants (⌖), the "
         "shaft between lit cyan. It earns its place in tall buildings.",
}


def page():
    parts = ["<style>%s .grid{display:grid;grid-template-columns:repeat(4,max-content);gap:14px 18px}"
             " .grid h3{margin:0 0 6px} .note{max-width:1620px;font-size:13px;line-height:1.5;"
             "color:#b8c0cc;margin:4px 0 22px} .note b{color:#e6e6f0}</style>" % term.CSS,
             "<h2>Close up, indoors</h2>",
             "<p class='subtitle'>Three ways the chosen close up could carry indoors, over the "
             "game's own frames (Miracorp Tattoo tracked, so the walk goes by the lift). "
             "Same 40×13 pane.</p>"]
    for name, title in PLACES:
        p = Place(name)
        parts.append("<h3 style='margin-top:18px'>%s</h3><div class='grid'>" % html.escape(title))
        for key, label, draw in CONCEPTS:
            g = p.frame() if draw is None else draw(p)
            parts.append("<div class='option' data-choice='%s-%s' onclick='toggleSelect(this)'>"
                         "<h3>%s</h3>%s</div>" % (key, name, html.escape(label),
                                                  term.render(g.crop(78, 0, 42, 28), 14)))
        parts.append("</div>")
    parts.append("<div class='note'>%s</div>" % "<br>".join(
        "<b>%s</b> · %s" % (html.escape(l), html.escape(TEXT[k])) for k, l, _ in CONCEPTS[1:]))
    parts.append("<h3>1 in the whole frame</h3>")
    parts.append(term.render(floor_plan(Place("indoor_flat")), 11.5))
    return "\n".join(parts)


open(OUT, "w").write(page())
print("wrote", OUT)
