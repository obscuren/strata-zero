"""Overworld mockups, round 1: one seeded world, drawn into the play screen's map area
(the game's own street frame, seed 8, 120x40) three ways, and the same ground in the Net.

    python3 -I ow.py <out.html>

A · atlas: one cell a tile, the ground as glyphs, highways as double lines.
B · from orbit: one half-block a tile, the ground dark, city light and lit roads.
C · survey chart: contour dots, a drawn coast, cities as tinted ground, names on the map.
D · the Net over the same ground: backbones down the highways, dead grid between."""

import heapq
import html
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SPECS = os.path.join(HERE, "..")
sys.path.insert(0, os.path.join(SPECS, "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(SPECS, "2026-11-02-minimap-mock"))
import term  # noqa: E402

SHOT = os.path.join(SPECS, "2026-11-04-net-nodes-mock", "shots", "s2_street.svg")
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "round1.html")

# The world in tiles; one tile is a half-block (B), two tiles stacked are a cell (A, C, D).
W, H = 160, 104
VX, VY, VW, VH = 32, 28, 79, 60  # the view onto it
MX, MY, MW, MH = 0, 5, 79, 30  # the map area in the frame

BG = "#04060a"
PANEL_TEXT = "#c8d0dc"
LABEL = "#a0aab9"
CYAN = "#00e5ff"
SIGNAL = "#00ffc8"
AMBER = "#ffb000"
RED = "#ff2848"
MAGENTA = "#ff00c8"
WHITE = "#e6f0f5"
DIM = "#8a93a3"
FAINT = "#4a525e"
RULE = "#283c4b"
ACCENT = {"CorpCore": "#00e5ff", "NightMarket": "#ff00c8", "Megablocks": "#ffbe00",
          "CombatZone": "#ff283c", "Docks": "#00c8be", "Industrial": "#e6e600",
          "NeonStrip": "#ff50b4", "OldTown": "#ff963c"}


def rgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def hexc(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in t)


def mix(a, b, t):
    a, b = rgb(a), rgb(b)
    return hexc(tuple(a[i] + (b[i] - a[i]) * t for i in range(3)))


def scale(c, k):
    return hexc(tuple(v * k for v in rgb(c)))


# ---- noise

def h32(*a):
    h = 2166136261
    for v in a:
        h ^= v & 0xffffffff
        h = (h * 16777619) & 0xffffffff
        h ^= h >> 15
        h = (h * 2246822519) & 0xffffffff
        h ^= h >> 13
    return h / 0xffffffff


def vnoise(x, y, s):
    ix, iy = math.floor(x), math.floor(y)
    fx, fy = x - ix, y - iy
    ux, uy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a, b = h32(ix, iy, s), h32(ix + 1, iy, s)
    c, d = h32(ix, iy + 1, s), h32(ix + 1, iy + 1, s)
    return a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy


def fbm(x, y, s, o=4):
    t, amp, f, norm = 0.0, 1.0, 1.0, 0.0
    for i in range(o):
        t += vnoise(x * f, y * f, s + i * 101) * amp
        norm += amp
        amp *= 0.5
        f *= 2
    return t / norm


def seg_dist(p, a, b):
    (px, py), (ax, ay), (bx, by) = p, a, b
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - ax - t * dx, py - ay - t * dy)


def poly_dist(p, pts):
    return min(seg_dist(p, pts[i], pts[i + 1]) for i in range(len(pts) - 1))


def inb(x, y):
    return 0 <= x < W and 0 <= y < H


N4 = ((1, 0), (-1, 0), (0, 1), (0, -1))

# ---- the ground

RIDGE = [(78, 22), (92, 29), (108, 33), (126, 35), (160, 39)]
RIVER = [(124, 24), (116, 32), (110, 40), (104, 45), (96, 49), (86, 52), (76, 56),
         (66, 60), (56, 63)]


def coast_x(y):
    return (40 + 7 * (fbm(y / 16, 0.5, 7, 3) - 0.5) * 2 + 0.05 * (y - 50)
            + 15 * math.exp(-((y - 63) / 6.0) ** 2))


ELEV = [[0.0] * W for _ in range(H)]
WATER = [[0] * W for _ in range(H)]  # 0 land, 1 sea, 2 river, 3 lake
BIOME = [["flats"] * W for _ in range(H)]

for y in range(H):
    for x in range(W):
        if x < coast_x(y) + 4 * (fbm(x / 5, y / 5, 9, 3) - 0.5):
            WATER[y][x] = 1
        m = max(0.0, 1 - poly_dist((x, y), RIDGE) / 11)
        e = 0.32 * fbm(x / 26, y / 26, 1) + 0.08 * x / W
        e += 0.78 * m * (0.55 + 0.45 * fbm(x / 7, y / 7, 3))
        ELEV[y][x] = e
        if math.hypot((x - 72) / 3.4, (y - 38) / 2.4) + 0.9 * fbm(x / 2, y / 2, 21) < 1.25:
            WATER[y][x] = 3

# the river: walked along its line with a meander, kept four-connected
RIVER_PATH = []
for i in range(len(RIVER) - 1):
    (ax, ay), (bx, by) = RIVER[i], RIVER[i + 1]
    n = int(math.hypot(bx - ax, by - ay) * 4)
    for k in range(n):
        t = k / n
        px, py = ax + (bx - ax) * t, ay + (by - ay) * t
        L = math.hypot(bx - ax, by - ay)
        nx, ny = -(by - ay) / L, (bx - ax) / L
        off = 2.6 * (fbm(px / 9, py / 9, 33) - 0.5) * 2
        q = (round(px + nx * off), round(py + ny * off))
        if RIVER_PATH and q == RIVER_PATH[-1]:
            continue
        if RIVER_PATH:
            lx, ly = RIVER_PATH[-1]
            if abs(q[0] - lx) + abs(q[1] - ly) > 1:
                RIVER_PATH.append((q[0], ly))
        RIVER_PATH.append(q)
dedup = []
for p in RIVER_PATH:
    if p in dedup:
        i = dedup.index(p)
        dedup = dedup[:i + 1]
    else:
        dedup.append(p)
RIVER_PATH = dedup
for (x, y) in RIVER_PATH:
    if inb(x, y) and WATER[y][x] != 1:
        WATER[y][x] = 2

DRIVER = {}
q = [(0, p) for p in RIVER_PATH]
for d, p in q:
    DRIVER.setdefault(p, d)

for y in range(H):
    for x in range(W):
        if WATER[y][x]:
            BIOME[y][x] = "water"
            continue
        e = ELEV[y][x]
        moist = fbm(x / 18, y / 18, 5)
        near_river = min((abs(x - rx) + abs(y - ry) for rx, ry in RIVER_PATH[::3]), default=99)
        if e > 0.74:
            b = "peak"
        elif e > 0.58:
            b = "mount"
        elif e > 0.47:
            b = "hills"
        elif near_river <= 3 and moist > 0.42:
            b = "farm" if fbm(x / 3, y / 3, 41) > 0.45 else "scrub"
        elif moist > 0.6:
            b = "scrub"
        else:
            b = "flats"
        if math.hypot((x - 102) / 11, (y - 78) / 5.5) + 0.7 * fbm(x / 5, y / 5, 13) < 1.25:
            b = "salt"
        if math.hypot((x - 54) / 8, (y - 80) / 5) + 0.8 * fbm(x / 4, y / 4, 17) < 1.3:
            b = "toxic"
        BIOME[y][x] = b

# ---- highways

ROUTES = []  # (name, path)


def astar(a, b):
    cost0 = {"flats": 1, "scrub": 1.2, "farm": 1.4, "salt": 1, "hills": 3, "mount": 9,
             "peak": 40, "toxic": 12}
    used = {p for _, path in ROUTES for p in path}

    def cost(x, y):
        w = WATER[y][x]
        if w == 1:
            return 80
        if w == 3:
            return 50
        if w == 2:
            return 7
        c = cost0[BIOME[y][x]] + 0.8 * fbm(x / 6, y / 6, 51)
        return 0.35 if (x, y) in used else c

    dist = {a: 0}
    prev = {}
    pq = [(0, 0, a)]
    while pq:
        _, d, p = heapq.heappop(pq)
        if p == b:
            break
        if d > dist[p]:
            continue
        for dx, dy in N4:
            q = (p[0] + dx, p[1] + dy)
            if not inb(*q):
                continue
            nd = d + cost(*q)
            if nd < dist.get(q, 1e18):
                dist[q] = nd
                prev[q] = p
                heapq.heappush(pq, (nd + 0.3 * (abs(q[0] - b[0]) + abs(q[1] - b[1])), nd, q))
    path = [b]
    while path[-1] != a:
        path.append(prev[path[-1]])
    return path[::-1]


CITIES = [
    dict(name="Lowshore", c=(62, 60), size=14, seeds=6, home=True),
    dict(name="Ashgate", c=(106, 44), size=9, seeds=4),
    dict(name="Greyfield", c=(82, 80), size=4.5, seeds=2),
    dict(name="Ironreach", c=(140, 92), size=9, seeds=4),
    dict(name="Northwick", c=(42, 14), size=6, seeds=3),
]
CC = {c["name"]: c["c"] for c in CITIES}
for name, a, b in [
    ("Route 9", CC["Lowshore"], CC["Ashgate"]),
    ("Route 3", CC["Lowshore"], CC["Greyfield"]),
    ("Route 3", CC["Greyfield"], CC["Ironreach"]),
    ("Route 14", CC["Ashgate"], CC["Ironreach"]),
    ("Coast Road", CC["Lowshore"], CC["Northwick"]),
    ("Coast Road", CC["Northwick"], (44, 0)),
    ("Route 9", CC["Ashgate"], (159, 30)),
    ("Route 3", CC["Greyfield"], (76, 103)),
    ("Route 14", CC["Ironreach"], (159, 98)),
]:
    ROUTES.append((name, astar(a, b)))

ROAD = {}
for i, (name, path) in enumerate(ROUTES):
    for p in path:
        ROAD.setdefault(p, name)


def bfs(srcs):
    d = {p: 0 for p in srcs}
    q = list(srcs)
    for p in q:
        for dx, dy in N4:
            n = (p[0] + dx, p[1] + dy)
            if inb(*n) and n not in d and d[p] < 8:
                d[n] = d[p] + 1
                q.append(n)
    return d


DROAD = bfs(list(ROAD))
DWATER = bfs([(x, y) for y in range(H) for x in range(W) if WATER[y][x] in (1, 3)])

# ---- cities grow: out from the core, along the roads and the shore, not up the hills

URB = [[None] * W for _ in range(H)]  # (city index, density)


def grow(ci, centre, size, salt, dense=1.0):
    cx, cy = centre
    best = {}
    R = int(size * 3)
    for y in range(cy - R, cy + R + 1):
        for x in range(cx - R, cx + R + 1):
            if not inb(x, y) or WATER[y][x] in (1, 3):
                continue
            b = BIOME[y][x]
            if b in ("peak", "toxic"):
                continue
            d = math.hypot(x - cx, y - cy)
            s = size * (0.7 + 0.6 * fbm(x / 5, y / 5, salt)) - d
            dr = DROAD.get((x, y), 9)
            if dr <= 3 and d < 2.7 * size:
                s += (3.2 - dr) * 1.5 * (1.15 - d / (2.7 * size))
            dw = DWATER.get((x, y), 9)
            if dw <= 2:
                s += (2.5 - dw) * 1.0
            s -= {"hills": 4, "mount": 12}.get(b, 0)
            if s > 0:
                best[(x, y)] = min(1.0, s / size) * dense
    keep, q = {centre}, [centre]
    for p in q:
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                n = (p[0] + dx, p[1] + dy)
                if n in best and n not in keep:
                    keep.add(n)
                    q.append(n)
    for p in keep:
        if p in best and p != centre or p == centre:
            v = best.get(p, 1.0)
            cur = URB[p[1]][p[0]]
            if cur is None or cur[1] < v:
                URB[p[1]][p[0]] = (ci, v)


for i, c in enumerate(CITIES):
    grow(i, c["c"], c["size"], 60 + i)

# outlying towns: where a highway leaves a city, a little way out
SATS = []
for name, path in ROUTES:
    for ci, c in enumerate(CITIES):
        cx, cy = c["c"]
        if c["size"] < 8:
            continue
        for k, (x, y) in enumerate(path):
            d = math.hypot(x - cx, y - cy)
            if abs(d - 2.1 * c["size"]) < 0.5 and h32(x, y, 77) < 0.55:
                if all(math.hypot(x - sx, y - sy) > 8 for sx, sy, _ in SATS):
                    SATS.append((x, y, ci))
for sx, sy, ci in SATS:
    grow(ci, (sx, sy), 2.6, 90 + sx, 0.55)

# districts
DIST = {}
THEMES = ["CorpCore", "Docks", "NeonStrip", "Megablocks", "NightMarket", "Industrial",
          "OldTown", "CombatZone"]
for ci, c in enumerate(CITIES):
    tiles = [(x, y) for y in range(H) for x in range(W) if URB[y][x] and URB[y][x][0] == ci]
    tiles.sort(key=lambda p: h32(p[0], p[1], 5 + ci))
    seeds = [c["c"]]
    for p in tiles:
        if len(seeds) >= c["seeds"]:
            break
        if all(math.hypot(p[0] - s[0], p[1] - s[1]) > c["size"] * 0.55 for s in seeds):
            seeds.append(p)
    themes = ["CorpCore"] + [THEMES[1 + (j + ci) % 7] for j in range(len(seeds) - 1)]
    for p in tiles:
        j = min(range(len(seeds)), key=lambda j: math.hypot(p[0] - seeds[j][0], p[1] - seeds[j][1]))
        th = themes[j]
        if th == "CorpCore" and math.hypot(p[0] - c["c"][0], p[1] - c["c"][1]) > c["size"] * 0.4:
            th = themes[1 + j % (len(themes) - 1)] if len(themes) > 1 else "OldTown"
        if math.hypot(p[0] - c["c"][0], p[1] - c["c"][1]) > c["size"] * 1.3:
            th = "Megablocks" if h32(p[0] // 4, p[1] // 4, 3) < 0.5 else "OldTown"
        if DWATER.get(p, 9) <= 1 and th not in ("CorpCore",) and WATER[p[1]][p[0]] == 0:
            th = "Docks" if h32(p[0] // 3, p[1] // 3, 4) < 0.7 else th
        DIST[p] = th


def urban(x, y):
    return inb(x, y) and URB[y][x] is not None


# ---- points of interest, out in the open

def on_route(name, frac, side=0):
    for n, path in ROUTES:
        if n == name:
            p = path[int(len(path) * frac)]
            return (p[0], p[1] + side)
    raise KeyError(name)


POIS = [
    ("⌂", AMBER, "Kilo-9 truck stop", on_route("Route 9", 0.5, -1), "camera on the pump"),
    ("╪", RED, "Halcor checkpoint", on_route("Route 9", 0.74), "corp gate · cameras"),
    ("⬡", SIGNAL, "relay mast R7", (88, 33), "Net relay · open"),
    ("△", "#ff8040", "nomad camp", (98, 66), "no Net"),
    ("×", RED, "AV wreck", (70, 72), "black box"),
    ("▣", "#a890d8", "Carrow arcology", (44, 84), "dead · sealed"),
    ("▦", "#7ab0e0", "Sunfield array", (118, 60), "maintenance node"),
    ("◎", CYAN, "Brine water plant", (92, 76), "server · guarded"),
    ("◈", MAGENTA, "Velk test range", (108, 84), "black ICE"),
    ("◆", "#c8a050", "Rig 4", (38, 50), "offshore · data vault"),
]
POIS = [p for p in POIS if inb(*p[3])]
REGIONS = [("the Teeth", (104, 30)), ("Brine Flats", (100, 79)), ("the Spill", (50, 79)),
           ("Sable Bay", (40, 63)), ("Dust Flats", (74, 44)), ("the Silt", (88, 56))]

# where the player stands: on Route 9, just past the city's edge
r9 = next(path for n, path in ROUTES if n == "Route 9")
out = [p for p in r9 if not urban(*p)]
YOU = out[3]

# ---- road and river edges, kept per path so parallel lines never join

def edges(paths, cell):
    """Box-drawing directions per cell: {cell: set('NESW')}."""
    e = {}
    for path in paths:
        cells = []
        for p in path:
            c = cell(p)
            if not cells or cells[-1] != c:
                cells.append(c)
        for a, b in zip(cells, cells[1:]):
            dx, dy = b[0] - a[0], b[1] - a[1]
            da = {(1, 0): "E", (-1, 0): "W", (0, 1): "S", (0, -1): "N"}.get((dx, dy))
            if not da:
                continue
            db = {"E": "W", "W": "E", "S": "N", "N": "S"}[da]
            e.setdefault(a, set()).add(da)
            e.setdefault(b, set()).add(db)
        for c in (cells[0], cells[-1]):
            e.setdefault(c, set())
    return e


def box(dirs, kind):
    key = "".join(d for d in "NESW" if d in dirs)
    sets = {
        "double": {"": "═", "N": "║", "S": "║", "NS": "║", "E": "═", "W": "═", "EW": "═",
                   "NE": "╚", "NW": "╝", "ES": "╔", "SW": "╗", "NES": "╠", "NSW": "╣",
                   "ESW": "╦", "NEW": "╩", "NESW": "╬"},
        "heavy": {"": "━", "N": "┃", "S": "┃", "NS": "┃", "E": "━", "W": "━", "EW": "━",
                  "NE": "┗", "NW": "┛", "ES": "┏", "SW": "┓", "NES": "┣", "NSW": "┫",
                  "ESW": "┳", "NEW": "┻", "NESW": "╋"},
        "round": {"": "─", "N": "│", "S": "│", "NS": "│", "E": "─", "W": "─", "EW": "─",
                  "NE": "╰", "NW": "╯", "ES": "╭", "SW": "╮", "NES": "├", "NSW": "┤",
                  "ESW": "┬", "NEW": "┴", "NESW": "┼"},
    }
    return sets[kind][key]


def cell_of(p):
    return (p[0] - VX, (p[1] - VY) // 2) if p[1] >= VY else (p[0] - VX, -1 - (VY - p[1] - 1) // 2)


ROAD_E = edges([p for _, p in ROUTES], cell_of)
RIVER_E = edges([RIVER_PATH], cell_of)


def pair(cx, cy):
    """The two tiles under cell (cx, cy) of the view."""
    x = VX + cx
    return [(x, VY + 2 * cy), (x, VY + 2 * cy + 1)]


def pick(cx, cy, f):
    for p in pair(cx, cy):
        if inb(*p) and f(*p):
            return p
    return None


POI_AT = {}
for g, col, name, p, note in POIS:
    POI_AT[cell_of(p)] = (g, col, name, p, note)
YOU_CELL = cell_of(YOU)

# ---- the frame

BASE = Grid.from_svg(SHOT, 120, 40)


def frame(here, near, footer, mode="on foot · overworld"):
    g = Grid(120, 40, BG)
    for y in range(40):
        for x in range(120):
            g.c[y][x] = list(BASE.c[y][x])
    g.fill(MX, MY, MW, MH, BG)
    g.fill(80, 0, 40, 3, BG)
    g.fill(80, 5, 40, 30, BG)
    g.put(81, 0, here[0], CYAN)
    g.put(112, 0, "☾ 22:00", PANEL_TEXT)
    for x in range(80, 120):
        g.c[1][x] = list(BASE.c[1][x])
    g.put(81, 2, here[1], LABEL)
    g.put(81, 5, here[2], PANEL_TEXT)
    g.put(80, 6, "─" * 40, RULE)
    g.put(81, 7, "no job on · find a fixer", PANEL_TEXT)
    g.put(80, 8, "─" * 40, RULE)
    minimap(g)
    g.put(81, 22, footer[0], PANEL_TEXT)
    g.put(119 - len(footer[1]), 22, footer[1], CYAN)
    g.put(80, 23, "─" * 40, RULE)
    g.put(81, 24, "NEAR", LABEL)
    for i, (glyph, col, text, dist) in enumerate(near):
        g.put(81, 25 + i, glyph, col, bold=True)
        g.put(83, 25 + i, text, PANEL_TEXT)
        g.put(119 - len(dist), 25 + i, dist, DIM)
    g.fill(0, 36, 120, 1, BG)
    g.put(1, 36, mode, LABEL)
    g.put(98, 36, "[↵] go down  [?]", DIM)
    return g


def minimap(g):
    """The whole world, four tiles a pixel, the view framed."""
    px = lambda x, y: tile_night(x, y, small=True)  # noqa: E731
    for r in range(13):
        for c in range(40):
            cols = []
            for half in (0, 1):
                acc = None
                best = 0
                for dy in range(4):
                    for dx in range(4):
                        x, y = c * 4 + dx, (r * 2 + half) * 4 + dy
                        if not inb(x, y):
                            continue
                        if urban(x, y):
                            v = 2 + URB[y][x][1]
                        elif (x, y) in ROAD:
                            v = 1.5
                        else:
                            v = 0
                        if acc is None or v > best:
                            acc, best = (x, y), v
                cols.append(px(*acc) if acc else BG)
            g.c[9 + r][80 + c] = ["▀", cols[0], cols[1], False]
    x0, x1 = VX // 4, (VX + VW) // 4
    y0, y1 = 9 + VY // 8, 9 + (VY + VH) // 8
    for (x, y), ch in (((80 + x0, y0), "┌"), ((80 + x1, y0), "┐"), ((80 + x0, y1), "└"),
                       ((80 + x1, y1), "┘")):
        g.c[y][x][0], g.c[y][x][1] = ch, WHITE
    yx, yy = 80 + YOU[0] // 4, 9 + YOU[1] // 8
    g.c[yy][yx] = ["@", WHITE, g.c[yy][yx][2], True]


def near_list():
    out = []
    for gl, col, name, p, note in POIS:
        d = math.hypot(p[0] - YOU[0], (p[1] - YOU[1]))
        out.append((d, gl, col, name, p))
    out.sort()
    arrows = {(0, -1): "↑", (1, -1): "↗", (1, 0): "→", (1, 1): "↘", (0, 1): "↓", (-1, 1): "↙",
              (-1, 0): "←", (-1, -1): "↖"}
    res = []
    for d, gl, col, name, p in out[:6]:
        a = math.atan2(p[1] - YOU[1], p[0] - YOU[0])
        k = (round(math.cos(a) * 1.2), round(math.sin(a) * 1.2))
        k = (max(-1, min(1, k[0])), max(-1, min(1, k[1])))
        res.append((gl, col, name, "%d %s" % (round(d), arrows.get(k, "·"))))
    return res


# ---- B · from orbit: one tile, one colour

def tile_night(x, y, small=False):
    w = WATER[y][x]
    n = fbm(x / 3, y / 3, 81, 2)
    if w == 2:
        return "#0c2440" if not urban(x, y) else "#08182c"
    if urban(x, y):
        ci, v = URB[y][x]
        th = DIST.get((x, y), "OldTown")
        h = h32(x, y, 9)
        lit = min(1.0, (0.25 + 0.75 * v) * (0.12 + 0.88 * h ** 1.6))
        if (x, y) in ROAD:
            lit = min(1.0, lit + 0.4)
        base = mix("#120a05", "#ffb860", lit)
        if lit > 0.72:
            base = mix(base, "#fff0d0", (lit - 0.72) * 3)
        return mix(base, ACCENT[th], 0.06 + 0.16 * v)
    if (x, y) in ROAD:
        return "#5a3a14" if not small else "#4a3010"
    if w == 1:
        dw = min(4, max(0, x - coast_x(y) + 4)) if False else 0
        return mix("#03070d", "#071425", n * 0.8)
    if w in (2, 3):
        return "#0e2a4c"
    b = BIOME[y][x]
    e = ELEV[y][x]
    sh = 0
    if inb(x - 1, y - 1) and inb(x + 1, y + 1):
        sh = (ELEV[y - 1][x - 1] - ELEV[y + 1][x + 1]) * 6
    base = {"flats": "#15120d", "scrub": "#11160d", "farm": "#142012", "salt": "#24221f",
            "hills": "#1c1812", "mount": "#22222a", "peak": "#3a3c48", "toxic": "#0e1a08"}[b]
    c = scale(base, 0.9 + 0.25 * n + max(-0.35, min(0.6, sh)))
    if any(urban(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)):
        c = mix(c, "#2a1a0a", 0.5)
    if b == "toxic" and h32(x, y, 12) < 0.10:
        c = "#2e6a12"
    if b == "farm" and (x + y // 2) % 3 == 0:
        c = scale(c, 1.25)
    return c


def draw_orbit(g):
    for cy in range(MH):
        for cx in range(MW):
            (x0, y0), (x1, y1) = pair(cx, cy)
            top = tile_night(x0, y0) if inb(x0, y0) else BG
            bot = tile_night(x1, y1) if inb(x1, y1) else BG
            g.c[MY + cy][MX + cx] = ["▀", top, bot, False]
    for (cx, cy), (gl, col, name, p, note) in POI_AT.items():
        if 0 <= cx < MW and 0 <= cy < MH:
            c = g.c[MY + cy][MX + cx]
            g.c[MY + cy][MX + cx] = [gl, col, mix(c[1], c[2], 0.5), True]
    put_you(g)


def put_you(g):
    cx, cy = YOU_CELL
    g.c[MY + cy][MX + cx] = ["@", "#ffffff", "#1e3a4a", True]


# ---- A · atlas: one cell a tile, a glyph each

def atlas_cell(cx, cy):
    tiles = [p for p in pair(cx, cy) if inb(*p)]
    if not tiles:
        return [" ", BG, BG, False]
    key = (cx, cy)
    if key in ROAD_E and any(p in ROAD for p in tiles):
        on_water = all(WATER[p[1]][p[0]] for p in tiles)
        in_city = any(urban(*p) for p in tiles)
        fg = "#f0c870" if in_city else "#c8a050"
        bg = "#0a1a2e" if on_water else ("#1a1408" if in_city else BG)
        return [box(ROAD_E[key], "double"), fg, bg, in_city]
    if key in RIVER_E and any(WATER[p[1]][p[0]] == 2 for p in tiles):
        return [box(RIVER_E[key], "round"), "#3a8ad8", "#06121f", False]
    u = [p for p in tiles if urban(*p)]
    if u:
        p = max(u, key=lambda p: URB[p[1]][p[0]][1])
        v = URB[p[1]][p[0]][1]
        th = DIST.get(p, "OldTown")
        acc = ACCENT[th]
        r = h32(p[0], p[1], 31)
        if v > 0.75:
            ch = "█" if r < 0.45 else "▓"
        elif v > 0.45:
            ch = "▓" if r < 0.4 else "▒"
        elif v > 0.22:
            ch = "▒" if r < 0.35 else "░"
        else:
            ch = "░" if r < 0.45 else ("▫" if r < 0.75 else "·")
        fg = mix(mix("#2a2e38", "#9aa0ae", v), acc, 0.22 + 0.38 * v)
        if v > 0.45 and r > 0.92:
            ch, fg = "▪", acc
        return [ch, fg, mix(BG, acc, 0.03 + 0.06 * v), False]
    if key in RIVER_E and any(WATER[p[1]][p[0]] == 2 for p in tiles):
        return [box(RIVER_E[key], "round"), "#3a8ad8", "#06121f", False]
    if any(WATER[p[1]][p[0]] == 3 for p in tiles):
        return ["≈", "#2a5a8a", "#0a1a2e", False]
    if all(WATER[p[1]][p[0]] == 1 for p in tiles):
        x, y = tiles[0]
        r = h32(x, y // 2, 7)
        shore = DWATER.get((x, y), 0) == 0 and any(
            inb(x + dx, y + dy) and WATER[y + dy][x + dx] == 0 for dx, dy in N4 for _ in [0])
        if shore:
            return ["~", "#2a5a8a", "#0a1a2e", False]
        return ["~" if r < 0.12 else " ", "#1a3a5c", "#050c18", False]
    land = [p for p in tiles if WATER[p[1]][p[0]] == 0]
    if not land:
        return ["~", "#2a5a8a", "#0a1a2e", False]
    x, y = land[0]
    b = BIOME[y][x]
    r = h32(x, y, 13)
    if b == "peak":
        return ["▲", "#c8ccd8", BG, False]
    if b == "mount":
        return ["▲", "#787c8a", BG, False]
    if b == "hills":
        return ["∩" if r < 0.7 else "·", "#7a6648", BG, False]
    if b == "toxic":
        return ["≈" if r < 0.18 else ("·" if r < 0.6 else " "), "#4a8a22" if r < 0.9 else "#9aff5a",
                "#060c04", False]
    if b == "salt":
        return ["░" if r < 0.5 else "·", "#5a5852", BG, False]
    if b == "farm":
        return ["▬", "#3a6a30", BG, False]
    if b == "scrub":
        return ['"' if r < 0.35 else ("," if r < 0.6 else " "), "#4a5a34", BG, False]
    return ["·" if r < 0.3 else ("." if r < 0.45 else " "), "#4a4234", BG, False]


def draw_atlas(g):
    for cy in range(MH):
        for cx in range(MW):
            g.c[MY + cy][MX + cx] = atlas_cell(cx, cy)
    for (cx, cy), (gl, col, name, p, note) in POI_AT.items():
        if 0 <= cx < MW and 0 <= cy < MH:
            g.c[MY + cy][MX + cx] = [gl, col, g.c[MY + cy][MX + cx][2], True]
    put_you(g)


# ---- C · survey chart

def band(x, y):
    if not inb(x, y) or WATER[y][x]:
        return -1
    return int(ELEV[y][x] * 9)


def draw_chart(g):
    for cy in range(MH):
        for cx in range(MW):
            tiles = [p for p in pair(cx, cy) if inb(*p)]
            x, y = tiles[0]
            cell = [" ", BG, BG, False]
            water = all(WATER[p[1]][p[0]] in (1, 3) for p in tiles)
            if water:
                shore = any(inb(x + dx, y + dy) and not WATER[y + dy][x + dx]
                            for dx in (-1, 0, 1) for dy in (-2, -1, 0, 1, 2))
                cell = ["░" if shore else ("·" if (cx + cy * 3) % 6 == 0 else " "),
                        "#1c4a6c" if shore else "#123048", "#050b14", False]
            else:
                b0 = band(x, y)
                edge = any(band(x + dx, y + dy) not in (-1,) and band(x + dx, y + dy) < b0
                           for dx, dy in ((1, 0), (0, 2), (-1, 0), (0, -2)))
                if edge and b0 >= 3:
                    cell = ["·", mix("#2a2418", "#8a7048", min(1, (b0 - 3) / 4)), BG, False]
                if BIOME[y][x] == "toxic":
                    cell = ["╳" if (cx + cy) % 3 == 0 else " ", "#3e7a18", "#07100a", False]
                if BIOME[y][x] == "salt":
                    cell = ["─" if cy % 2 == 0 and cx % 3 else " ", "#3a3934", BG, False]
            u = [p for p in tiles if urban(*p)]
            if u:
                p = max(u, key=lambda p: URB[p[1]][p[0]][1])
                v = URB[p[1]][p[0]][1]
                th = DIST.get(p, "OldTown")
                bgc = mix(BG, ACCENT[th], 0.10 + 0.22 * v)
                cell = [" ", BG, bgc, False]
                # the city's edge as a fine line where the next cell out is open ground
                if not any(urban(*q) for q in pair(cx, cy - 1) if inb(*q)):
                    cell = ["▔", mix(bgc, ACCENT[th], 0.6), bgc, False]
            if (cx, cy) in RIVER_E and any(WATER[p[1]][p[0]] == 2 for p in tiles):
                cell = [box(RIVER_E[(cx, cy)], "round"), "#3a8ad8", cell[2], False]
            if (cx, cy) in ROAD_E and any(p in ROAD for p in tiles):
                cell = [box(ROAD_E[(cx, cy)], "heavy"), "#e0b060", cell[2], False]
            g.c[MY + cy][MX + cx] = cell
    # route shields: a short label where each route runs in the open
    seen = set()
    for name, path in ROUTES:
        num = name.split()[-1] if name.startswith("Route") else "C"
        if (name, path[0]) in seen:
            continue
        seen.add((name, path[0]))
        open_ = [p for p in path if not urban(*p)]
        if len(open_) < 10:
            continue
        p = open_[len(open_) // 2]
        cx, cy = cell_of(p)
        lab = "[%s]" % num
        if 0 <= cx < MW - len(lab) and 0 <= cy < MH:
            g.put(MX + cx + 1, MY + cy, lab, "#e0b060", bg="#1a1408", bold=True)
    for (cx, cy), (gl, col, name, p, note) in POI_AT.items():
        if 0 <= cx < MW and 0 <= cy < MH:
            g.c[MY + cy][MX + cx] = [gl, col, g.c[MY + cy][MX + cx][2], True]
            lab = name
            if cx + 2 + len(lab) < MW:
                g.put(MX + cx + 2, MY + cy, lab, mix(col, DIM, 0.45), right=MW)
            else:
                g.put(MX + cx - 1 - len(lab), MY + cy, lab, mix(col, DIM, 0.45))
    for c in CITIES:
        cx, cy = cell_of(c["c"])
        lab = c["name"].upper()
        x = cx - len(lab) // 2
        if 0 <= cy < MH and -len(lab) < x < MW:
            g.put(MX + max(0, x), MY + cy - 1, lab[max(0, -x):], WHITE, bold=True, right=MW)
    for name, p in REGIONS:
        cx, cy = cell_of(p)
        if 0 <= cy < MH and 0 <= cx < MW:
            g.put(MX + cx - len(name) // 2, MY + cy, name, "#6a7284", right=MW)
    put_you(g)


# ---- D · the Net over the same ground

VOID = "#010204"


def draw_net(g):
    hops = {}
    for (cx, cy), dirs in ROAD_E.items():
        hops[(cx, cy)] = dirs
    for cy in range(MH):
        for cx in range(MW):
            tiles = [p for p in pair(cx, cy) if inb(*p)]
            x, y = tiles[0]
            cell = [" ", VOID, VOID, False]
            if cx % 4 == 0 and cy % 2 == 0:
                cell = ["·", "#0f3330", VOID, False]
            if all(WATER[p[1]][p[0]] == 1 for p in tiles):
                cell = [" ", VOID, "#010309", False]
                if cx % 4 == 0 and cy % 2 == 0:
                    cell = ["·", "#0a1a28", "#010309", False]
            u = [p for p in tiles if urban(*p)]
            if u:
                p = max(u, key=lambda p: URB[p[1]][p[0]][1])
                v = URB[p[1]][p[0]][1]
                th = DIST.get(p, "OldTown")
                r = h32(p[0], p[1], 71)
                acc = ACCENT[th]
                bgc = mix(VOID, acc, 0.05 + 0.10 * v)
                if r < 0.18 + 0.4 * v:
                    ch = "◆" if r < 0.06 * (1 + v) else ("◇" if r < 0.2 else "▫")
                    cell = [ch, mix("#1a4a48", acc, 0.4 + 0.5 * v), bgc, False]
                else:
                    cell = ["·", mix(VOID, acc, 0.25), bgc, False]
            if (cx, cy) in ROAD_E and any(p in ROAD for p in tiles):
                in_city = bool(u)
                cell = [box(ROAD_E[(cx, cy)], "heavy"),
                        SIGNAL if in_city else "#1f8a76", cell[2], in_city]
            g.c[MY + cy][MX + cx] = cell
    # a few packets on the backbones
    for name, path in ROUTES:
        for k in range(7, len(path), 23):
            cx, cy = cell_of(path[k])
            if 0 <= cx < MW and 0 <= cy < MH and not urban(*path[k]):
                g.c[MY + cy][MX + cx][1] = "#c8fff0"
                g.c[MY + cy][MX + cx][3] = True
    net_of = {"⌂": "◇", "╪": "◈", "⬡": "⬡", "△": None, "×": "◇", "▣": "⌗", "▦": "◇",
              "◎": "◆", "◈": "◈", "◆": "⌗"}
    for (cx, cy), (gl, col, name, p, note) in POI_AT.items():
        ng = net_of.get(gl)
        if not ng or not (0 <= cx < MW and 0 <= cy < MH):
            continue
        tier = RED if "black" in note else (SIGNAL if "open" in note or "camera" in note
                                            or "node" in note or "black box" in note
                                            else "#9fe6ff")
        g.c[MY + cy][MX + cx] = [ng, tier, mix(VOID, tier, 0.18), True]
        # a short spur to the nearest backbone
        best = None
        for (rx, ry) in ROAD_E:
            d = abs(rx - cx) + abs(ry - cy)
            if d and (best is None or d < best[0]):
                best = (d, rx, ry)
        if best and best[0] <= 6:
            _, rx, ry = best
            x, y = cx, cy
            while (x, y) != (rx, ry):
                if x != rx:
                    x += 1 if rx > x else -1
                elif y != ry:
                    y += 1 if ry > y else -1
                if (x, y) == (rx, ry):
                    break
                c = g.c[MY + y][MX + x]
                if c[0] in ("·", " "):
                    g.c[MY + y][MX + x] = ["┄" if x != rx or y == cy else "┆", "#2a6a60",
                                           c[2], False]
    put_you(g)


# ---- the page

def here_lines(net=False):
    if net:
        return ["Net · Route 9 backbone", "dead grid · 2 nodes in reach",
                "Route 9 trunk · Lowshore to Ashgate"]
    return ["Dust Flats · Route 9", "open ground · clear night",
            "Route 9 · 2 tiles out of Lowshore"]


def render_all():
    near = near_list()
    foot = ("%d,%d · the world" % YOU, "↵ down")
    A = frame(here_lines(), near, foot)
    draw_atlas(A)
    B = frame(here_lines(), near, foot)
    draw_orbit(B)
    C = frame(here_lines(), near, foot)
    draw_chart(C)
    D = frame(here_lines(True), near, foot, mode="jacked in · overworld")
    draw_net(D)
    return A, B, C, D


CSS = term.CSS + """
body{background:#0d1117;color:#c9d1d9;font-family:-apple-system,system-ui,sans-serif;
 margin:24px 32px 60px}
h1{font-size:22px;margin:0 0 6px}h2{font-size:18px;margin:34px 0 4px}
p{max-width:1100px;line-height:1.5;margin:6px 0}
.note{color:#8b949e;font-size:13px}
code{background:#161b22;padding:1px 5px;border-radius:4px}
"""


def main():
    A, B, C, D = render_all()
    out = ["<!doctype html><meta charset=utf-8><title>Overworld · round 1</title>",
           "<style>%s</style>" % CSS,
           "<h1>The overworld · round 1</h1>",
           "<p>One seeded world, drawn into the play screen's map area (the game's own "
           "frame, HUD and all). You stand on Route 9 just outside <b>Lowshore</b> "
           "(the city we have now, on its bay); <b>Ashgate</b> is up the river at the foot "
           "of the hills, <b>Greyfield</b> a small town at a junction. The cities grew: out "
           "from the core, along the highways and the shore, not up the hills or into the "
           "Spill, with outlying towns where a highway leaves. The minimap pane shows the "
           "whole world with the view framed; NEAR lists what's around. ↵ goes down onto "
           "the detail map of the tile you're on.</p>",
           "<p class=note>My guesses, all yours to change: a tile is a 20×20 patch of detail "
           "map in A, C and D (one cell), a 20×10 patch in B (one half-block, so twice the "
           "detail and the same on-screen size). Lowshore covers about 30×30 tiles here, so "
           "the city's 240×120 detail map grows to fit its shape. Names are placeholders "
           "from the flavor pack's parts.</p>"]
    for title, g, text in [
        ("A · Atlas", A,
         "One cell a tile, every tile a glyph: dust <code>·</code>, scrub <code>\"</code>, "
         "fields <code>▬</code>, hills <code>∩</code>, mountains <code>▲</code>, the Spill "
         "<code>░</code> in poison green. Highways as double lines, the river as a thin blue "
         "line, cities as shaded blocks in their districts' colours. Reads like the "
         "walking map, one level up."),
        ("B · From orbit", B,
         "One half-block a tile: the ground dark with the hills shaded, the sea black, "
         "cities as sodium light tinted by district, highways as faint lit lines. Twice "
         "the detail of A in the same space; the shape of each city reads best here. "
         "POIs and you are glyphs on top."),
        ("C · Survey chart", C,
         "A map you read: contour dots up the hills, the coast hatched, cities as tinted "
         "ground with a fine edge, highways heavy with route shields, every POI and "
         "region named on the map. Closest to the deck's MAP and HELIX's look."),
        ("D · The same ground, jacked in", D,
         "The Net over the world (in A's scale): cities as dense node fields in their "
         "districts' colours, the highways' backbones running between them with packets "
         "on them, and the dead grid everywhere else. Out in the open only a few lone "
         "nodes: the truck stop's camera, relay mast R7, the checkpoint, the water "
         "plant's server, the test range's black ICE, each on a spur to the nearest "
         "backbone. The nomad camp isn't on the Net at all."),
    ]:
        out.append("<h2>%s</h2>" % html.escape(title))
        out.append("<p>%s</p>" % text)
        out.append(term.render(g, 13.0))
    open(OUT, "w").write("\n".join(out))
    chk = os.environ.get("CHECK_DIR")
    if chk:
        for k, g in zip("ABCD", (A, B, C, D)):
            open(os.path.join(chk, "frame_%s.html" % k), "w").write(
                "<!doctype html><meta charset=utf-8><style>%s body{margin:0;background:#000}"
                "</style>%s" % (term.CSS, term.render(g, 14.0)))
    print(OUT)


if __name__ == "__main__":
    main()
