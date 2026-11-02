"""Minimap mockups, round 1, drawn over the game's own street frame (seed 8, the walk
to Marlo's body tracked, 120x40).

    python3 -I mm.py <shots dir> <out.html>

The shots dir holds minimap_dump.json from the throwaway tmp_minimap_dump shot test.

Now · the minimap as it is: the whole city, 10x10 tiles a cell, accent x walkability.
A · close up: the deck MAP's look (no seen/unseen), 3x3 tiles a half-block pixel,
    centred on you; your walk, the target, home.
B · street lines: the same ground as box-drawing streets, avenues bright, blocks
    tinted by district, district names.
C · the whole city: every district at once in the deck MAP's look, the view framed,
    the district you stand in named."""

import html
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, HERE)
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else None
J = json.load(open(os.path.join(S, "minimap_dump.json")))
CW, CH = J["w"], J["h"]
TER, DIS = J["terrain"], J["district"]
DISTRICTS = {d["id"]: d for d in J["districts"]}
PLAYER = tuple(J["player"])
BODY = tuple(J["body"])
PATH = [tuple(p) for p in J["path"]]

BG = "#0a0b10"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
GREEN = "#7cff9a"
DIM = "#8a93a3"
FAINT = "#4a525e"
TEXT = "#cdd4de"
WATER = "#0e2848"
NEUTRAL = "#282c34"

# The pane: x 80..119, the map rows 13..25, the coordinate row 26.
PX, PY, PW, PH = 80, 13, 40, 13
CROW = 26
SOLID = set("WMNRD")
WALK = set("sapfk^g")


def rgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def hexc(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in t)


def scale(c, f):
    return hexc(tuple(v * f for v in rgb(c)))


def mix(a, b, f):
    a, b = rgb(a), rgb(b)
    return hexc(tuple(a[i] + (b[i] - a[i]) * f for i in range(3)))


def frame():
    f = J["frames"][0]
    g = Grid(f["w"], f["h"])
    for y, row in enumerate(f["cells"]):
        for x, (ch, fg, bg, bold) in enumerate(row):
            g.c[y][x] = [ch or " ", fg or "#c8d0dc", bg or BG, bold]
    return g


BASE = frame()
PANEL = BG

# The main view, from where the @ stands on it.
_at = next((x, y) for y in range(5, 35) for x in range(0, 79) if BASE.c[y][x][0] == "@"
           and BASE.c[y][x][1] == "#ffffff")
VIEW = (PLAYER[0] - _at[0], PLAYER[1] - (_at[1] - 5), 79, 30)


def ter(x, y):
    return TER[y][x] if 0 <= x < CW and 0 <= y < CH else " "


def district(x, y):
    if not (0 <= x < CW and 0 <= y < CH):
        return None
    c = DIS[y][x]
    return None if c == "." else DISTRICTS[ord(c) - ord("a")]


def shade(x0, y0, s):
    """The deck MAP's pixel, with no seen/unseen: its class in its district's colours."""
    n = solid = water = 0
    road = walk = False
    dc = {}
    for y in range(y0, y0 + s):
        for x in range(x0, x0 + s):
            t = ter(x, y)
            if t == " " and not (0 <= x < CW and 0 <= y < CH):
                continue
            n += 1
            if t in SOLID:
                solid += 1
            elif t == "w":
                water += 1
            elif t == "r":
                road = True
            if t in WALK:
                walk = True
            d = district(x, y)
            if d:
                dc[d["id"]] = dc.get(d["id"], 0) + 1
    if n == 0:
        return PANEL
    if solid * 2 > n:
        cls = "B"
    elif road:
        cls = "r"
    elif walk:
        cls = "s"
    elif water * 2 >= n:
        return WATER
    else:
        return PANEL
    d = DISTRICTS[max(dc, key=dc.get)] if dc else None
    if d is None:
        return NEUTRAL
    return {"B": d["roof"], "r": scale(d["road"], 1.9), "s": scale(d["sidewalk"], 1.15)}[cls]


class View:
    """A window of half-block pixels: `s` tiles a pixel from tile (ox, oy)."""

    def __init__(self, x, y, w, h, s, ox, oy):
        self.x, self.y, self.w, self.h, self.s, self.ox, self.oy = x, y, w, h, s, ox, oy

    def centred(x, y, w, h, s, at):
        ox = at[0] - w * s // 2
        oy = at[1] - h * s
        return View(x, y, w, h, s, ox, oy)

    def px(self, t):
        return ((t[0] - self.ox) // self.s, (t[1] - self.oy) // self.s)

    def cell(self, t):
        px, py = self.px(t)
        if 0 <= px < self.w and 0 <= py < 2 * self.h:
            return (self.x + px, self.y + py // 2)
        return None

    def pixels(self):
        return [[shade(self.ox + i * self.s, self.oy + j * self.s, self.s) for i in range(self.w)]
                for j in range(2 * self.h)]


def paint(g, v, p):
    for j in range(v.h):
        for i in range(v.w):
            t, b = p[2 * j][i], p[2 * j + 1][i]
            if t == b:
                g.c[v.y + j][v.x + i] = [" ", t, t, False]
            else:
                g.c[v.y + j][v.x + i] = ["▀", t, b, False]


def walk_px(v, p, f=0.75):
    lit = set()
    for t in PATH:
        q = v.px(t)
        if 0 <= q[0] < v.w and 0 <= q[1] < 2 * v.h:
            lit.add(q)
    for (i, j) in lit:
        p[j][i] = mix(p[j][i], CYAN, f)


def mark(g, v, t, ch, fg, keep_bg=True):
    c = v.cell(t)
    if not c:
        return None
    x, y = c
    cell = g.c[y][x]
    # A glyph over a half-block cell: keep the pixel under the glyph's half as its ground.
    px, py = v.px(t)
    if cell[0] == "▀":
        bg = cell[1] if py % 2 == 0 else cell[2]
    else:
        bg = cell[2]
    g.c[y][x] = [ch, fg, bg, True]
    return c


def edge_arrow(g, v, t, ch, fg):
    """A target off the view: its glyph on the border, pointing the way."""
    cx = v.x + v.w // 2
    cy = v.y + v.h // 2
    tx = (t[0] - v.ox) / v.s
    ty = (t[1] - v.oy) / v.s / 2
    dx, dy = tx - v.w / 2, ty - v.h / 2
    k = min((v.w / 2 - 1) / abs(dx) if dx else 1e9, (v.h / 2 - 1) / abs(dy) if dy else 1e9)
    x, y = int(cx + dx * k), int(cy + dy * k)
    arrows = {(1, 0): "▸", (-1, 0): "◂", (0, 1): "▾", (0, -1): "▴"}
    a = arrows[(1 if abs(dx) > abs(dy) * 2 and dx > 0 else -1 if abs(dx) > abs(dy) * 2 else 0,
                0 if abs(dx) > abs(dy) * 2 else (1 if dy > 0 else -1))]
    g.c[y][x] = [ch, fg, g.c[y][x][2], True]


def view_corners(g, v, fg="#e6e6f0"):
    x0, y0, w, h = VIEW
    for (t, ch) in (((x0, y0), "┌"), ((x0 + w - 1, y0), "┐"), ((x0, y0 + h - 1), "└"),
                    ((x0 + w - 1, y0 + h - 1), "┘")):
        c = v.cell(t)
        if c:
            cell = g.c[c[1]][c[0]]
            bg = cell[2] if cell[0] != "▀" else (cell[1] if ((t[1] - v.oy) // v.s) % 2 == 0 else cell[2])
            g.c[c[1]][c[0]] = [ch, fg, bg, False]


def view_box(g, v, fg="#e6e6f0"):
    x0, y0, w, h = VIEW
    a, b = v.cell((x0, y0)), v.cell((x0 + w - 1, y0 + h - 1))
    if not a or not b:
        return
    for x in range(a[0], b[0] + 1):
        for y in (a[1], b[1]):
            cell = g.c[y][x]
            g.c[y][x] = ["─", fg, cell[2] if cell[0] != "▀" else cell[2], False]
    for y in range(a[1], b[1] + 1):
        for x in (a[0], b[0]):
            cell = g.c[y][x]
            g.c[y][x] = ["│", fg, cell[2], False]
    for (x, y, ch) in ((a[0], a[1], "┌"), (b[0], a[1], "┐"), (a[0], b[1], "└"), (b[0], b[1], "┘")):
        g.c[y][x][0] = ch


def row(g, y, left, right=None, lfg=DIM, rfg=DIM):
    g.fill(PX, y, PW, 1, PANEL)
    x = PX
    for (s, fg, bold) in left:
        g.put(x, y, s, fg, PANEL, bold)
        x += len(s)
    if right:
        n = sum(len(s) for s, _, _ in right)
        x = PX + PW - n - 1
        for (s, fg, bold) in right:
            g.put(x, y, s, fg, PANEL, bold)
            x += len(s)


def label(g, v, d, fg=None, dark=False, taken=None):
    """A district's name across its centre, in its accent, when it falls in view; moved
    a row up or down when it would cross another, dropped when it cannot fit."""
    c = v.cell(tuple(d["center"]))
    if not c:
        return
    name = d["name"].upper()
    x = max(v.x, min(v.x + v.w - len(name), c[0] - len(name) // 2))
    taken = set() if taken is None else taken
    for y in (c[1], c[1] - 1, c[1] + 1, c[1] - 2, c[1] + 2):
        span = {(x + i, y) for i in range(-1, len(name) + 1)}
        if v.y <= y < v.y + v.h and not span & taken:
            c = (c[0], y)
            taken |= span
            break
    else:
        return
    for i, ch in enumerate(name):
        cell = g.c[c[1]][x + i]
        bg = cell[2] if cell[0] != "▀" else mix(cell[1], cell[2], 0.5)
        g.c[c[1]][x + i] = [ch, fg or scale(d["accent"], 0.8), scale(bg, 0.55) if dark else bg, True]


# ---------------------------------------------------------------- the concepts

def now():
    return BASE


def close_up():
    g = frame()
    v = View.centred(PX, PY, PW, PH, 3, PLAYER)
    p = v.pixels()
    walk_px(v, p)
    paint(g, v, p)
    view_corners(g, v)
    mark(g, v, BODY, "◆", MAGENTA) or edge_arrow(g, v, BODY, "◆", MAGENTA)
    mark(g, v, PLAYER, "@", "#ffffff")
    here = district(*PLAYER)
    row(g, CROW, [(" " + here["name"], scale(here["accent"], 0.85), False)],
        [("◆ ", MAGENTA, False), ("73 m ↗", DIM, False)])
    return g


BOX = {
    (0, 0, 1, 1): "─", (1, 1, 0, 0): "│", (1, 1, 1, 1): "┼",
    (0, 1, 1, 1): "┬", (1, 0, 1, 1): "┴", (1, 1, 0, 1): "├", (1, 1, 1, 0): "┤",
    (0, 1, 0, 1): "┌", (0, 1, 1, 0): "┐", (1, 0, 0, 1): "└", (1, 0, 1, 0): "┘",
    (0, 0, 0, 1): "╶", (0, 0, 1, 0): "╴", (1, 0, 0, 0): "╵", (0, 1, 0, 0): "╷",
    (0, 0, 0, 0): "·",
}
HEAVY = {
    "─": "━", "│": "┃", "┼": "╋", "┬": "┳", "┴": "┻", "├": "┣", "┤": "┫", "┌": "┏", "┐": "┓",
    "└": "┗", "┘": "┛", "╶": "╺", "╴": "╸", "╵": "╹", "╷": "╻",
}


def runs():
    """For each road tile, the length of its horizontal and vertical road runs."""
    hr = [[0] * CW for _ in range(CH)]
    vr = [[0] * CW for _ in range(CH)]
    for y in range(CH):
        x = 0
        while x < CW:
            if TER[y][x] == "r":
                e = x
                while e < CW and TER[y][e] == "r":
                    e += 1
                for i in range(x, e):
                    hr[y][i] = e - x
                x = e
            else:
                x += 1
    for x in range(CW):
        y = 0
        while y < CH:
            if TER[y][x] == "r":
                e = y
                while e < CH and TER[e][x] == "r":
                    e += 1
                for j in range(y, e):
                    vr[j][x] = e - y
                y = e
            else:
                y += 1
    return hr, vr


HR, VR = runs()


def centres():
    """The middle tile across each road: of a vertical road in each row, of a horizontal
    one in each column, with the road's width there."""
    vc = {}
    hc = {}
    for y in range(CH):
        x = 0
        while x < CW:
            if VR[y][x] >= 10:
                e = x
                while e < CW and VR[y][e] >= 10:
                    e += 1
                vc[((x + e - 1) // 2, y)] = e - x
                x = e
            else:
                x += 1
    for x in range(CW):
        y = 0
        while y < CH:
            if HR[y][x] >= 7:
                e = y
                while e < CH and HR[e][x] >= 7:
                    e += 1
                hc[(x, (y + e - 1) // 2)] = e - y
                y = e
            else:
                y += 1
    return vc, hc


VC, HC = centres()


def street_lines():
    g = frame()
    s = 3
    w, h = PW, PH
    ox = PLAYER[0] - w * s // 2
    oy = PLAYER[1] - h * s
    cw, chh = s, 2 * s

    def tiles(cx, cy):
        return [(ox + cx * cw + i, oy + cy * chh + j) for j in range(chh) for i in range(cw)]

    def H(cx, cy):
        return max([HC.get(t, 0) for t in tiles(cx, cy)])

    def V(cx, cy):
        return max([VC.get(t, 0) for t in tiles(cx, cy)])

    hv = {(cx, cy): (H(cx, cy), V(cx, cy)) for cy in range(-1, h + 1) for cx in range(-1, w + 1)}
    for cy in range(h):
        for cx in range(w):
            hh, vv = hv[(cx, cy)]
            ts = tiles(cx, cy)
            dc = {}
            for (x, y) in ts:
                d = district(x, y)
                if d:
                    dc[d["id"]] = dc.get(d["id"], 0) + 1
            d = DISTRICTS[max(dc, key=dc.get)] if dc else None
            water = sum(1 for (x, y) in ts if ter(x, y) == "w") * 2 > len(ts)
            gx, gy = PX + cx, PY + cy
            if water:
                g.c[gy][gx] = [" ", WATER, mix(PANEL, WATER, 0.7), False]
                continue
            ground = mix(PANEL, d["roof"], 0.22) if d else PANEL
            if not hh and not vv:
                g.c[gy][gx] = [" ", TEXT, ground, False]
                continue
            up = vv and hv[(cx, cy - 1)][1]
            dn = vv and hv[(cx, cy + 1)][1]
            lf = hh and hv[(cx - 1, cy)][0]
            rt = hh and hv[(cx + 1, cy)][0]
            key = tuple(1 if a else 0 for a in (up, dn, lf, rt))
            ch = BOX.get(key, "·")
            wide = hh >= 3 or vv >= 3
            if wide:
                ch = HEAVY.get(ch, ch)
            acc = d["accent"] if d else "#8090a0"
            # Toned toward grey so the walk keeps the only true cyan.
            fg = mix(PANEL, mix(acc, "#9aa3b3", 0.45), 0.8 if wide else 0.5)
            g.c[gy][gx] = [ch, fg, ground, wide]
    # The walk as its own line, cell to cell.
    seq = []
    for t in [PLAYER] + PATH:
        c = ((t[0] - ox) // cw, (t[1] - oy) // chh)
        if not seq or seq[-1] != c:
            seq.append(c)
    arms = {}
    for a, b in zip(seq, seq[1:]):
        dx, dy = b[0] - a[0], b[1] - a[1]
        arms.setdefault(a, set()).add((dx, dy))
        arms.setdefault(b, set()).add((-dx, -dy))
    for (cx, cy), ds in arms.items():
        if 0 <= cx < w and 0 <= cy < h:
            key = tuple(1 if d in ds else 0 for d in ((0, -1), (0, 1), (-1, 0), (1, 0)))
            ch = HEAVY.get(BOX.get(key, "·"), BOX.get(key, "·"))
            cell = g.c[PY + cy][PX + cx]
            g.c[PY + cy][PX + cx] = [ch, CYAN, cell[2], True]
    v = View(PX, PY, w, h, s, ox, oy)
    for d in DISTRICTS.values():
        c = v.cell(tuple(d["center"]))
        if c and DISTRICTS.get(d["id"]):
            name = d["name"].upper()
            x = max(PX, min(PX + PW - len(name), c[0] - len(name) // 2))
            for i, ch in enumerate(name):
                g.c[c[1]][x + i] = [ch, scale(d["accent"], 0.9), g.c[c[1]][x + i][2], False]
    for (t, ch, fg) in ((BODY, "◆", MAGENTA), (PLAYER, "@", "#ffffff")):
        c = v.cell(t)
        if c:
            g.c[c[1]][c[0]] = [ch, fg, g.c[c[1]][c[0]][2], True]
    here = district(*PLAYER)
    row(g, CROW, [(" N ", TEXT, True), ("▲", DIM, False)],
        [("◆ ", MAGENTA, False), ("The drop · 73 m", DIM, False)])
    return g


def whole_city():
    g = frame()
    s = max(-(-CW // PW), -(-CH // (2 * 10)))
    rows = -(-CH // (2 * s))
    v = View(PX, PY, PW, rows, s, 0, 0)
    p = v.pixels()
    walk_px(v, p, 0.9)
    paint(g, v, p)
    view_box(g, v)
    x0, y0, w, h = VIEW
    a, b = v.cell((x0, y0)), v.cell((x0 + w - 1, y0 + h - 1))
    taken = {(x, y) for x in range(a[0], b[0] + 1) for y in range(a[1], b[1] + 1)}
    for d in DISTRICTS.values():
        label(g, v, d, dark=True, taken=taken)
    mark(g, v, BODY, "◆", MAGENTA)
    mark(g, v, PLAYER, "@", "#ffffff")
    for y in range(PY + rows, CROW):
        g.fill(PX, y, PW, 1, PANEL)
    here = district(*PLAYER)
    row(g, PY + rows + 1, [(" you  ", DIM, False), (here["name"], here["accent"], True)],
        [("Street", DIM, False)])
    row(g, PY + rows + 2, [(" ◆    ", MAGENTA, False), ("Kowloon Row", DISTRICTS[5]["accent"], False)],
        [("73 m ↗", DIM, False)])
    row(g, CROW, [], [("172,56 · Street", FAINT, False)])
    return g


# ---------------------------------------------------------------- the page

def crop(g):
    return g.crop(78, 0, 42, 36)


CONCEPTS = [
    ("now", "Now", "The whole city squeezed into 24 columns: one cell per 10×10 tiles, coloured "
     "by the district's accent and how walkable it is. No streets, no places, no aspect."),
    ("a", "A · Close up", "The deck MAP's own look, zoomed in: 3×3 tiles a half-block pixel, "
     "centred on you, about 120×78 tiles (1½ screens wide, 2½ tall). The view's corners, your "
     "walk in the tracking cyan, the target. No seen/unseen. The bottom row: your district, "
     "and the target's distance and bearing."),
    ("b", "B · Street lines", "The same patch drawn as streets: box-drawing lines, avenues "
     "heavy, streets light, each in its district's accent; blocks tinted by district, district "
     "names where they fall. Your walk in cyan. Reads like a nav unit, not a picture."),
    ("c", "C · Whole city, legible", "Keeps the job of the old one (where am I in the city) "
     "but drawn like the deck MAP: half-blocks at 6×6 tiles, true aspect, every district named, "
     "the main view boxed. Two rows under it say where you are and where the target is."),
]

DRAW = {"now": now, "a": close_up, "b": street_lines, "c": whole_city}


def page():
    parts = ["<style>%s .opt{display:inline-block;vertical-align:top;margin:0 18px 28px 0;"
             "width:430px} .opt h3{margin:0 0 6px} .opt p{font-size:13px;line-height:1.45;"
             "margin:8px 0 0;color:#b8c0cc} .full{margin:10px 0 30px}</style>" % term.CSS,
             "<h2>The minimap: three directions</h2>",
             "<p class='subtitle'>Drawn over the real street frame (seed 8, the drop tracked, "
             "120×40). The right column is the sidebar; MAP is the 13 rows under JOBS.</p>"]
    parts.append("<div class='options'>")
    for key, title, text in CONCEPTS:
        g = DRAW[key]()
        parts.append("<div class='opt option' data-choice='%s' onclick='toggleSelect(this)'>"
                     "<h3>%s</h3>%s<p>%s</p></div>"
                     % (key, html.escape(title), term.render(crop(g), 15), html.escape(text)))
    parts.append("</div>")
    before = Grid.from_svg(os.path.join(S, "atlas_listing_120x40.svg"), 120, 40)
    after = Grid.from_svg(os.path.join(S + "_after", "atlas_listing_120x40.svg"), 120, 40)
    parts.append("<h2 style='margin-top:30px'>The deck MAP without “where you have been”</h2>"
                 "<p class='subtitle'>Every pixel at the one level the walked ones had; what you "
                 "see right now is no brighter either. The header loses “lit: remembered · "
                 "faint: the city grid”.</p>")
    parts.append("<div class='split' style='display:flex;gap:18px;flex-wrap:wrap'>"
                 "<div><div class='label'>Now</div>%s</div><div><div class='label'>Without</div>"
                 "%s</div></div>" % (term.render(before, 10.5), term.render(after, 10.5)))
    parts.append("<h3>In the whole frame</h3>")
    for key, title, _ in CONCEPTS[1:]:
        parts.append("<div class='full'><div class='label'>%s</div>%s</div>"
                     % (html.escape(title), term.render(DRAW[key](), 12)))
    return "\n".join(parts)


if __name__ == "__main__":
    open(OUT, "w").write(page())
    print("wrote", OUT)
