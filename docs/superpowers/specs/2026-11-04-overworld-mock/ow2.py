"""Overworld mockups, round 2: one level down, at 22:00.

    python3 -I ow2.py <out.html>

1 · open ground: Kilo-9 truck stop on Route 9, the ground two ways (A sparse, B textured);
    the minimap pane shows the overworld round you in round 1's A.
2 · the same ground in the Net: A the dead grid, B a grid that fades away from signal.
3 · where Lowshore ends: now (the game's own frame at the map's east edge, seed 8, from
    the throwaway tmp_overworld_edge shot test) and grown out into the flats.
4 · the same edge in the Net: now and grown."""

import html
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "round2.html")
sys.argv = [sys.argv[0], os.devnull]
sys.path.insert(0, HERE)
import ow  # noqa: E402
from ow import Grid, fbm, h32, mix, scale, term  # noqa: E402

SHOTS = os.path.join(HERE, "shots")
STREET = Grid.from_svg(os.path.join(SHOTS, "edge_street.svg"), 120, 40)
NETF = Grid.from_svg(os.path.join(SHOTS, "edge_net.svg"), 120, 40)
VW, VH = 79, 30  # the map area, at x 0, y 5
BG = "#04060a"
YOU_FG = STREET.c[25][51][1]

LIGHT = "─│─└││┌├─┘─┴┐┤┬┼"
HEAVY = "━┃━┗┃┃┏┣━┛━┻┓┫┳╋"
DOUBLE = "═║═╚║║╔╠═╝═╩╗╣╦╬"


class Ground:
    """A patch of detail map: terrain and feature per cell, and the light on it."""

    def __init__(self, w=VW, h=VH, salt=0):
        self.w, self.h, self.salt = w, h, salt
        self.t = [["dirt"] * w for _ in range(h)]
        self.f = [[None] * w for _ in range(h)]
        self.k = [[None] * w for _ in range(h)]  # wall kind / accent
        self.lamps = []

    def inb(self, x, y):
        return 0 <= x < self.w and 0 <= y < self.h

    def set(self, x, y, t, k=None):
        if self.inb(x, y):
            self.t[y][x] = t
            if k is not None:
                self.k[y][x] = k

    def feat(self, x, y, f):
        if self.inb(x, y):
            self.f[y][x] = f
            if f == "lamp":
                self.lamps.append((x, y, 1.0))
            if f == "fire":
                self.lamps.append((x, y, 0.6))

    def rect(self, x0, y0, w, h, t, k=None):
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                self.set(x, y, t, k)

    def building(self, x0, y0, w, h, kind, door=None, mass="mass"):
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                edge = x in (x0, x0 + w - 1) or y in (y0, y0 + h - 1)
                corner = x in (x0, x0 + w - 1) and y in (y0, y0 + h - 1)
                if edge:
                    win = not corner and (x + y) % 2 == 0
                    self.set(x, y, "window" if win else "wall", kind)
                else:
                    self.set(x, y, mass, kind)
        if door:
            self.set(door[0], door[1], "door", kind)

    def fence(self, x0, y0, x1, y1):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if x in (x0, x1) or y in (y0, y1):
                    self.set(x, y, "fence")

    def light(self, x, y, ambient):
        v = ambient
        for lx, ly, s in self.lamps:
            d2 = (x - lx) ** 2 + (2 * (y - ly)) ** 2
            v += s * 0.45 / (1 + d2 / 8)
        return min(1.2, v)


WALLISH = ("wall", "window", "door")
WALL_FG = {"stop": "#80868e", "shanty": "#6e5a48", "mega": "#78787d", "low": "#7a6e5e"}
WIN_FG = {"stop": "#c07a30", "shanty": "#a0602a", "mega": "#bf8f00", "low": "#b08a50"}
MASS_FG = {"stop": "#18191c", "shanty": "#1e1813", "mega": "#171718", "low": "#1a1815"}


def mask(g, x, y, kinds):
    m = 0
    for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
        if g.inb(x + dx, y + dy) and g.t[y + dy][x + dx] in kinds:
            m |= bit
    return m


def ground_cell(g, x, y, look):
    """The cell for terrain at (x, y) before light: [ch, fg, bg]."""
    t = g.t[y][x]
    r = h32(x, y, 300 + g.salt)
    n = fbm(x / 9, y / 4.5, 310 + g.salt, 3)
    if t in ("dirt", "scrub") and look == "T":
        # A's sparse ground with a little relief: the rises a shade lighter, a few ░
        e = fbm(x / 8, y / 4, 320 + g.salt, 4)
        bg = mix("#0b0a08", "#16130f", 0.3 * n + 0.7 * e)
        if t == "scrub":
            if r < 0.35:
                return ['"' if r < 0.15 else ",", "#4a5a30" if r < 0.15 else "#3a4628", bg]
            return [" ", bg, bg]
        if e > 0.66 and r < 0.4:
            return ["░", mix(bg, "#3a3226", 0.38), bg]
        if r < 0.06:
            return ["·", "#433c30", bg]
        if r < 0.075:
            return [",", "#38402a", bg]
        return [" ", bg, bg]
    if t == "wash" and look == "T":
        bg = mix("#14110c", "#1c170f", n)
        if r < 0.3:
            return ["░", mix(bg, "#4a3e2a", 0.35), bg]
        return ["·" if r < 0.42 else " ", "#4a4030", bg]
    if t in ("dirt", "scrub"):
        if look == "A":
            bg = mix("#0b0a08", "#14110d", n)
            if t == "scrub":
                if r < 0.35:
                    return ['"' if r < 0.15 else ",", "#4a5a30" if r < 0.15 else "#3a4628", bg]
                return [" ", bg, bg]
            if r < 0.06:
                return ["·", "#433c30", bg]
            if r < 0.075:
                return [",", "#38402a", bg]
            return [" ", bg, bg]
        e = fbm(x / 8, y / 4, 320 + g.salt, 4)
        bg = mix("#0b0a08", "#18140f", e)
        if t == "scrub":
            bg = mix(bg, "#11140c", 0.5)
            if r < 0.3:
                return ['"' if r < 0.15 else ",", "#4a5832", bg]
        if e > 0.64:
            return ["▒" if e > 0.74 else "░", mix(bg, "#3e3426", 0.5), bg]
        if e < 0.36 and r < 0.3:
            return ["·", mix(bg, "#3a3226", 0.8), bg]
        if r < 0.05:
            return ["·", "#4a4232", bg]
        return [" ", bg, bg]
    if t == "wash":
        if look in ("A", "T"):
            bg = mix("#14110c", "#1a160f", n)
            return ["·" if r < 0.12 else " ", "#4a4030", bg]
        bg = mix("#1a160f", "#241e14", n)
        return ["░" if r < 0.55 else ("·" if r < 0.8 else " "), mix(bg, "#5a4c34", 0.5), bg]
    if t == "rock":
        if look in ("A", "T"):
            return ["▪" if r < 0.6 else "■", "#5e5850", mix("#0b0a08", "#14110d", n)]
        return ["▓" if r < 0.5 else "█", "#2e2a24", "#0c0a08"]
    if t == "gravel":
        return ["∙" if r < 0.45 else " ", "#4a453c", "#141310"]
    if t == "track":
        bg = mix("#15120e", "#1b1712", n)
        return ["·" if r < 0.18 else " ", "#3e3629", bg]
    if t == "road":
        bg = mix("#18181a", "#1d1d1f", n)
        return ["·" if r < 1 / 16 else " ", "#2e2e32", bg]
    if t == "side":
        return ["░", "#38383a", "#1d1d1f"]
    if t == "plaza":
        return ["▫" if r < 0.3 else " ", "#3e4248", "#141519"]
    if t == "lot":
        bg = mix("#121214", "#17171a", n)
        if y % 3 == 0 and x % 5 != 0:
            return ["─", "#26262a", bg]
        return ["·" if r < 0.08 else " ", "#2c2c30", bg]
    if t in WALLISH:
        kind = g.k[y][x] or "mega"
        if t == "door":
            return ["▯", "#d2d2c8", "#222224"]
        if t == "window":
            return ["▫", WIN_FG[kind], BG]
        m = mask(g, x, y, WALLISH)
        sets = HEAVY if kind == "mega" else LIGHT
        return [sets[m], WALL_FG[kind], BG]
    if t == "mass":
        kind = g.k[y][x] or "mega"
        return ["█▓▒░"[int(r * 4)], MASS_FG[kind], "#040507"]
    if t == "fence":
        m = mask(g, x, y, ("fence",))
        ch = "┼" if (x + y) % 4 == 0 or m in (3, 6, 9, 12) else LIGHT[m]
        return [ch, "#50545c", mix("#0b0a08", "#14110d", n)]
    if t == "void":
        return [" ", "#000000", "#000000"]
    return ["?", "#ff00ff", BG]


FEATURES = {
    "lamp": ("●", "#fff0c8"), "trash": ("%", "#736955"), "puddle": ("~", "#5073a5"),
    "camera": ("◉", "#ff3c46"), "pump": ("◘", "#b8b8a8"), "awning": ("▔", "#995a24"),
    "pylon": ("╫", "#6a6e76"), "cable": ("┄", "#2a2e36"), "fire": ("•", "#ff8c3a"),
    "scrap": ("▪", "#5a5248"), "grate": ("▦", "#4a4e50"), "post": ("║", "#4a4a50"),
    "vent": ("☰", "#7a7e86"),
}


def paint(g, look, ambient=0.82, keep=None):
    """`g` as cells for the 79x30 map area; `keep(x, y)` leaves a cell to the base frame."""
    out = {}
    for y in range(g.h):
        for x in range(g.w):
            if keep and keep(x, y):
                continue
            ch, fg, bg = ground_cell(g, x, y, look)
            f = g.f[y][x]
            bold = False
            if f:
                if isinstance(f, tuple):
                    ch, fg = f
                else:
                    ch, fg = FEATURES[f]
                bold = f in ("lamp", "fire", "camera")
            L = g.light(x, y, ambient)
            if f not in ("lamp", "fire", "camera") and not isinstance(f, tuple):
                fg = scale(fg, L)
            bg = scale(bg, L)
            out[(x, y)] = [ch, fg, bg, bold]
    return out


def apply(base, cells, x0=0, y0=5):
    g = copy(base)
    for (x, y), c in cells.items():
        g.c[y0 + y][x0 + x] = list(c)
    return g


def copy(base):
    g = Grid(120, 40)
    g.c = [[list(c) for c in row] for row in base.c]
    return g


# ---- 1 · Kilo-9, out on Route 9

ROAD_ROWS = (19, 20)


def wash_x(y):
    return 10 + 5 * math.sin(y / 6.5) + 0.4 * y


def kilo(look):
    g = Ground(salt=1)
    for y in range(VH):
        for x in range(VW):
            if fbm(x / 7, y / 3.5, 401, 3) > 0.6:
                g.set(x, y, "scrub")
            wx = wash_x(y)
            if abs(x - wx) < 2.2 + fbm(y / 3, 0.5, 403, 2):
                g.set(x, y, "wash")
                if abs(x - wx) > 1.5 and h32(x, y, 404) < 0.5:
                    g.set(x, y, "scrub")
            if h32(x // 3, y // 2, 405) < 0.03 and h32(x, y, 406) < 0.7:
                g.set(x, y, "rock")
    # the highway, its shoulders, a culvert where the wash passes under
    for x in range(VW):
        g.set(x, 18, "gravel")
        g.set(x, 21, "gravel")
        for y in ROAD_ROWS:
            g.set(x, y, "road")
    cx = round(wash_x(19))
    for x in (cx - 1, cx, cx + 1):
        g.feat(x, 18, "grate")
        g.feat(x, 21, "grate")
    # traffic
    for x0, y, col in ((4, 19, "#8c4d00"), (5, 19, "#8c4d00"), (63, 20, "#1e5aa0"),
                       (64, 20, "#1e5aa0"), (65, 20, "#1e5aa0")):
        g.feat(x0, y, ("▬", col))
    # the stop: forecourt, canopy and pumps, the store, the back lot with rigs
    g.rect(22, 12, 36, 6, "plaza")
    for x in range(28, 46):
        g.feat(x, 12, "awning")
    for x in (30, 34, 38, 42):
        g.feat(x, 14, "pump")
        g.feat(x, 16, "pump")
    g.building(36, 4, 19, 7, "stop", door=(45, 10))
    g.rect(45, 11, 1, 1, "plaza")
    g.rect(22, 3, 13, 9, "lot")
    g.fence(21, 2, 35, 12)
    g.set(28, 12, "plaza")
    g.set(29, 12, "plaza")
    g.set(33, 2, "track")
    g.set(34, 2, "track")
    for y in range(0, 2):
        for x in (32, 33, 34):
            g.set(x, y, "track")
    for y, col in ((5, "#5e6470"), (7, "#7a4a2a"), (9, "#3a5a7a")):
        for x in range(24, 32):
            g.feat(x, y, ("▬", col))
    for x, y in ((26, 17), (36, 17), (46, 17), (56, 17), (44, 11), (28, 4), (52, 12)):
        g.feat(x, y, "lamp")
    g.feat(48, 13, "camera")
    g.feat(53, 4, "vent")
    for x, y in ((23, 11), (55, 11), (35, 13), (57, 15)):
        g.feat(x, y, "trash")
    # the roadside sign
    for y in (15, 16, 17):
        g.feat(60, y, "post")
    for i, ch in enumerate("▣ｷﾛ9▣"):
        g.feat(58 + i, 14, (ch, "#ff8c00" if ch != "▣" else "#3c8cff"))
    # power line south of the road
    for x in range(VW):
        g.feat(x, 26, "pylon" if x % 16 == 6 else "cable")
    g.feat(68, 24, ("▬", "#5a3a20"))
    g.feat(69, 24, ("▬", "#5a3a20"))
    g.feat(70, 25, "scrap")
    return g


def kilo_frame(look):
    g = kilo(look)
    cells = paint(g, look, ambient=0.78)
    f = apply(STREET, cells)
    f.c[5 + 17][40] = ["@", YOU_FG, f.c[5 + 17][40][2], True]
    panel(f, "Dust Flats · Route 9", "dim · seen", "Kilo-9 truck stop · open 24h",
          overworld_pane=True)
    f.fill(0, 36, 40, 1, BG)
    f.put(1, 36, "on foot · Dust Flats", "#c8d0dc")
    return f


# ---- the panel: HERE lines, and the overworld round you in the minimap pane

def panel(f, title, sub, here, overworld_pane=False, net=False):
    f.fill(80, 0, 32, 1, BG)
    f.put(81, 0, title, "#00e5ff", bold=True)
    f.fill(80, 2, 40, 1, BG)
    f.put(81, 2, sub, "#a0aab9")
    f.fill(80, 5, 40, 30, BG)
    f.put(81, 5, here, "#c8d0dc")
    f.put(80, 6, "─" * 40, "#283c4b")
    f.put(81, 7, "no job on · find a fixer", "#c8d0dc")
    f.put(80, 8, "─" * 40, "#283c4b")
    f.put(80, 23, "─" * 40, "#283c4b")
    if overworld_pane:
        you = ow.cell_of(ow.YOU)
        for r in range(13):
            for c in range(40):
                cx, cy = you[0] - 20 + c, you[1] - 6 + r
                cell = ow.atlas_cell(cx, cy) if not net else net_atlas(cx, cy)
                f.c[9 + r][80 + c] = list(cell)
        for (cx, cy), (gl, col, name, p, note) in ow.POI_AT.items():
            c, r = cx - you[0] + 20, cy - you[1] + 6
            if 0 <= c < 40 and 0 <= r < 13:
                f.c[9 + r][80 + c] = [gl, col, f.c[9 + r][80 + c][2], True]
        f.c[15][100] = ["@", "#ffffff", "#1e3a4a", True]
        f.put(81, 22, "Dust Flats", "#d9a200")
        f.put(105, 22, "overworld", "#4a525e")


def net_atlas(cx, cy):
    tiles = [p for p in ow.pair(cx, cy) if ow.inb(*p)]
    if not tiles:
        return [" ", BG, BG, False]
    if (cx, cy) in ow.ROAD_E and any(p in ow.ROAD for p in tiles):
        return [ow.box(ow.ROAD_E[(cx, cy)], "double"), "#3afbd3", "#013a2f", False]
    if any(ow.urban(*p) for p in tiles):
        return ["·", "#4d6c28", "#011916", False]
    return ["·" if cx % 4 == 0 and cy % 2 == 0 else " ", "#2a3a38", "#010607", False]


# ---- 2 · the same ground in the Net

VOID = "#010204"
LANE = ("#3afbd3", "#013a2f")
PACKET = ("#ffffff", "#00b08b")


def net_kilo(look):
    lanes = {}

    def lane(x, y):
        lanes[(x, y)] = True

    for x in range(VW):
        lane(x, 20)
    for y in range(11, 20):
        lane(45, y)
    for y in range(15, 20):
        lane(60, y)
    cells = {}
    # what carries signal: lanes, the construct, the lone nodes
    sources = list(lanes) + [(x, y) for x in range(36, 55) for y in range(4, 11)]
    for y in range(VH):
        for x in range(VW):
            lattice = x % 4 == 0 and y % 2 == 0
            if look == "A":
                cells[(x, y)] = ["·" if lattice else " ", "#3a5250", "#010808", False]
            else:
                d = min(abs(x - sx) / 2 + abs(y - sy) for sx, sy in sources[::2])
                k = max(0.0, 1 - d / 6.5)
                bg = mix(VOID, "#011412", k)
                fg = mix(VOID, "#5a8a84", k)
                cells[(x, y)] = ["·" if lattice and k > 0.05 else " ", fg, bg, False]
                if k == 0 and h32(x, y, 77) < 0.012:
                    cells[(x, y)] = ["·", "#1a2422", VOID, False]
    for (x, y) in lanes:
        m = 0
        for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
            if (x + dx, y + dy) in lanes or (dx == 0 and dy == -1 and (x, y) == (45, 11)) or (
                    (x, y) == (60, 15) and dy == -1):
                m |= bit
        if x in (0, VW - 1) and y == 20:
            m |= 2 | 8
        cells[(x, y)] = [DOUBLE[m], LANE[0], LANE[1], False]
    for x in (9, 31, 52, 71):
        cells[(x, 20)] = ["═", PACKET[0], PACKET[1], True]
    # the stop's construct
    x0, y0, w, h = 36, 4, 19, 7
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            edge_x, edge_y = x in (x0, x0 + w - 1), y in (y0, y0 + h - 1)
            if edge_x or edge_y:
                m = 0
                if edge_x and y > y0:
                    m |= 1
                if edge_x and y < y0 + h - 1:
                    m |= 4
                if edge_y and x < x0 + w - 1:
                    m |= 2
                if edge_y and x > x0:
                    m |= 8
                ch = LIGHT[m]
                if (x - x0) % 5 == 2 and not (edge_x and edge_y):
                    ch = "▪"
                cells[(x, y)] = [ch, "#00d9aa" if ch != "▪" else "#73f8df", "#011110", False]
            else:
                cells[(x, y)] = [" ", "#c8d0dc", "#014437", False]
    cells[(45, 10)] = ["▯", "#e6f0f5", "#014e3f", True]
    for i, ch in enumerate("KILO-9"):
        cells[(43 + i, 7)] = [ch, "#b8f3ec", "#014437", True]
    cells[(39, 6)] = ["◆", "#e6f0f5", "#014437", True]
    cells[(51, 8)] = ["◆", "#e6f0f5", "#014437", True]
    # the pump camera and the sign's ad server, each on a spur
    cells[(48, 13)] = ["◇", "#9fe6ff", "#011916", True]
    for x in (46, 47):
        cells[(x, 13)] = ["─", "#00a27f", cells[(x, 13)][2], False]
    cells[(45, 13)] = ["╟", LANE[0], LANE[1], False]
    cells[(60, 14)] = ["◇", "#9fe6ff", "#011916", True]
    cells[(60, 15)] = ["║", LANE[0], LANE[1], False]
    f = apply(NETF, cells)
    f.c[5 + 17][40] = ["@", NETF.c[25][51][1], f.c[5 + 17][40][2], True]
    panel(f, "Dust Flats · Net", "dead grid · unseen", "Route 9 trunk · 3 nodes in reach",
          overworld_pane=True, net=True)
    f.fill(0, 36, 79, 1, BG)
    f.put(1, 36, "⚡ Dust Flats · dead grid", "#c8d0dc")
    f.fill(30, 36, 50, 1, BG)
    return f


# ---- 3 · where Lowshore ends

def city_edge():
    """The ground east of the city, painted over the game's own frame. The frame keeps
    the city proper; the lower blocks past column 38 give way to a lot and a scrapyard,
    the avenue runs on as Route 9, shanties crowd its north side."""
    g = Ground(salt=3)
    for y in range(VH):
        for x in range(VW):
            if fbm(x / 7, y / 3.5, 411, 3) > 0.62:
                g.set(x, y, "scrub")
            if h32(x // 3, y // 2, 415) < 0.03 and x > 62:
                g.set(x, y, "rock")
            wx = 70 + 4 * math.sin(y / 5)
            if y > 15 and abs(x - wx) < 2:
                g.set(x, y, "wash")
    # the avenue (rows 11..14 of the map area) running on as Route 9
    for x in range(52, VW):
        g.set(x, 11, "gravel" if x > 55 else "side")
        g.set(x, 14, "gravel" if x > 55 else "side")
        g.set(x, 12, "road")
        g.set(x, 13, "road")
    g.feat(54, 11, "lamp")
    g.feat(62, 14, "lamp")
    for x, y, col in ((66, 12, "#8c006e"), (67, 12, "#8c006e"), (58, 13, "#8c4d00"),
                      (59, 13, "#8c4d00"), (60, 13, "#8c4d00")):
        g.feat(x, y, ("▬", col))
    # below the avenue: a parking lot, a body shop in it, a fenced scrapyard
    g.rect(38, 15, 13, 15, "lot")
    g.building(40, 22, 8, 5, "low", door=(43, 22))
    g.feat(39, 16, "lamp")
    g.fence(52, 16, 66, 27)
    for y in range(17, 27):
        for x in range(53, 66):
            r = h32(x, y, 420)
            g.set(x, y, "track")
            if r < 0.22:
                g.feat(x, y, "scrap")
            elif r < 0.34:
                g.feat(x, y, "trash")
            elif r < 0.42 and x < 65:
                g.feat(x, y, ("▬", ["#5a3a20", "#4a4e56", "#3a2e28"][int(r * 100) % 3]))
    g.set(58, 16, "track")
    g.set(59, 16, "track")
    g.set(58, 15, "track")
    g.set(59, 15, "track")
    for x in range(38, 52):
        g.set(x, 15, "lot") if g.t[15][x] == "dirt" else None
    # north of the avenue: shanties past the last tower
    for (x0, y0, w, h) in ((53, 5, 5, 3), (59, 4, 6, 4), (54, 1, 4, 3), (66, 6, 4, 3),
                           (60, 0, 5, 3), (52, 8, 4, 2)):
        g.building(x0, y0, w, h, "shanty", door=(x0 + w // 2, y0 + h - 1))
    for x, y in ((58, 9), (65, 9), (51, 3), (66, 3)):
        g.feat(x, y, "trash")
    g.feat(58, 7, "fire")
    g.feat(64, 9, "fire")
    for y in range(0, 11):
        if g.t[y][57] == "dirt":
            g.set(57, y, "track")
    for x in range(52, 70):
        if g.t[10][x] == "dirt":
            g.set(x, 10, "track")
    # a billboard facing the city
    for y in (9, 10):
        g.feat(70, y, "post")
    for i, ch in enumerate("▤ﾄﾞｯｸ▤"):
        g.feat(68 + i, 8, (ch, "#ff50b4" if ch != "▤" else "#3c8cff"))
    return g


def keep_city(x, y):
    """The frame's own city: everything up to the edge above the avenue, the avenue to
    column 52, and below it only up to column 38."""
    if y <= 14:
        return x < 52
    return x < 38


def edge_frame(look="A"):
    g = city_edge()
    cells = paint(g, look, ambient=0.8, keep=keep_city)
    f = apply(STREET, cells)
    grow_minimap(f, net=False)
    return f


def grow_minimap(f, net):
    """Fill the minimap pane's empty east with ground and the road on."""
    for r in range(9, 22):
        for c in range(80, 120):
            ch, fg, bg, b = f.c[r][c]
            empty_top = ch in (" ", "▀") and fg in (BG, "#c8d0dc") or (ch == " ")
            if ch == " " and bg == BG:
                n = fbm(c / 3, r / 2, 5, 2)
                if net:
                    f.c[r][c] = ["▀", mix("#010607", "#011916", 0.2 * n),
                                 mix("#010607", "#011916", 0.2 * n), False]
                else:
                    top = mix("#1a1610", "#2a241a", n)
                    bot = mix("#1a1610", "#2a241a", fbm(c / 3, r / 2 + 0.5, 5, 2))
                    f.c[r][c] = ["▀", top, bot, False]
    # the road out, on the @'s row in the pane
    for r in range(9, 22):
        for c in range(80, 120):
            if f.c[r][c][0] == "@":
                for cc in range(c + 1, 120):
                    if f.c[r][cc][0] == "▀":
                        col = "#1f8a76" if net else "#4a4a4e"
                        f.c[r][cc] = ["▀", f.c[r][cc][1], col, False]
                return


# ---- 4 · the same edge in the Net

def net_edge():
    base = NETF
    cells = {}
    for y in range(VH):
        for x in range(VW):
            if keep_city(x, y):
                continue
            lattice = x % 4 == 0 and y % 2 == 0
            # the district's tint and the city's grid thin out eastward
            k = max(0.0, min(1.0, (60 - x) / 22 + 0.25 * (fbm(x / 6, y / 3, 431, 2) - 0.5)))
            bg = mix("#010607", "#011916", k * k)
            fg = mix("#2a3a38", "#4d6c28", k)
            cells[(x, y)] = ["·" if lattice else " ", fg, bg, False]
    lanes = set()
    for x in range(46, VW):
        lanes.add((x, 11))
    for y in range(9, 11):
        lanes.add((70, y))
    for (x, y) in lanes:
        m = 0
        for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
            if (x + dx, y + dy) in lanes:
                m |= bit
        if x == VW - 1:
            m |= 2
        if (x, y) == (46, 11):
            m |= 1 | 8
        cells[(x, y)] = [DOUBLE[m], LANE[0], LANE[1], False]
    cells[(70, 8)] = ["◇", "#9fe6ff", "#011916", True]
    for x in (55, 74):
        cells[(x, 11)] = ["═", PACKET[0], PACKET[1], True]
    # the body shop's little construct; the shanties are off the grid but one taps a line
    for (x0, y0, w, h, label) in ((40, 22, 8, 5, "BODY"),):
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                ex, ey = x in (x0, x0 + w - 1), y in (y0, y0 + h - 1)
                if ex or ey:
                    m = (1 if ex and y > y0 else 0) | (4 if ex and y < y0 + h - 1 else 0)
                    m |= (2 if ey and x < x0 + w - 1 else 0) | (8 if ey and x > x0 else 0)
                    cells[(x, y)] = [LIGHT[m], "#00a27f", "#011110", False]
                else:
                    cells[(x, y)] = [" ", "#c8d0dc", "#013027", False]
        for i, ch in enumerate(label):
            cells[(x0 + 2 + i, y0 + 2)] = [ch, "#b8f3ec", "#013027", True]
        cells[(x0 + 3, y0)] = ["▯", "#e6f0f5", "#014e3f", True]
    for y in range(15, 22):
        cells[(43, y)] = ["│", "#00a27f", cells[(43, y)][2], False]
    cells[(59, 6)] = ["◇", "#ff8c3a", "#0a0603", True]
    for y in range(7, 11):
        cells[(59, y)] = ["┆", "#6a4a2a", cells[(59, y)][2], False]
    f = apply(base, cells)
    # the trunk's old end at the edge now runs on east
    f.c[5 + 11][46] = ["╩", LANE[0], LANE[1], False]
    # the body shop's spur up to the avenue's lane
    for y in range(12, 15):
        f.c[5 + y][43] = ["│", "#00a27f", f.c[5 + y][43][2], False]
    f.c[5 + 11][43] = ["╤", LANE[0], LANE[1], False]
    grow_minimap(f, net=True)
    return f


# ---- the page

CSS = term.CSS + """
body{background:#0d1117;color:#c9d1d9;font-family:-apple-system,system-ui,sans-serif;
 margin:24px 32px 60px}
h1{font-size:22px;margin:0 0 6px}h2{font-size:19px;margin:40px 0 4px}
h3{font-size:15px;margin:22px 0 4px;color:#e6edf3}
p{max-width:1100px;line-height:1.5;margin:6px 0}
.note{color:#8b949e;font-size:13px}
code{background:#161b22;padding:1px 5px;border-radius:4px}
"""


def main():
    secs = [
        ("1 · Out on Route 9", "Kilo-9 truck stop at 22:00, one level down from the "
         "overworld tile you stood on in round 1. The highway is the overworld's double line "
         "made real: two lanes and gravel shoulders, the dry wash passing under it at a "
         "culvert, the power line south of it. The stop is the only light for tiles. The "
         "minimap pane shows the overworld round you (round 1's A), since down here the "
         "ground near you says little.",
         [("A · Sparse ground", kilo_frame("A"),
           "The flats as the atlas has them: near-black earth, a dot of dust here and there, "
           "scrub as <code>\"</code> and <code>,</code>, rocks <code>▪</code>, the wash a "
           "paler bed. Quiet; the stop and the road carry the screen."),
          ("B · Textured ground", kilo_frame("B"),
           "The same ground with relief: low rises as <code>░▒</code> in earth tones, the "
           "wash a sandy <code>░</code> bed, rock outcrops as dark <code>▓█</code>. More to "
           "read, busier around what matters.")]),
        ("2 · The same ground in the Net", "Jacked in at the same spot. Route 9's trunk "
         "runs along the highway with packets on it; the stop is a small construct with two "
         "nodes; the pump camera and the sign's ad server hang off the trunk on spurs. "
         "Everything else is empty Net, drawn two ways.",
         [("A · Dead grid", net_kilo("A"),
           "The grid carries on everywhere at the city's spacing, dimmed and untinted. You "
           "can walk it all; there is just nothing on it."),
          ("B · Fading grid", net_kilo("B"),
           "The grid only shows near signal: lit round the trunk and the construct, thinning "
           "to void a few tiles out, with the odd stray dot. Barren reads as barren.")]),
        ("3 · Where Lowshore ends", "Seed 8's city at its east edge, as the game draws "
         "it today and grown into the flats.",
         [("Now", copy(STREET),
           "The game's own frame: the city is a rectangle, so at x 239 it stops dead and "
           "the rest of the view is nothing."),
          ("Grown", edge_frame(),
           "The towers stop where the city's shape says (here unevenly: above the avenue "
           "the last tower block still stands, below it the blocks end a street earlier). "
           "Past them: a parking lot with a body shop, a fenced scrapyard, shanties crowding "
           "the avenue's north side round two fire barrels, one last lamp, a billboard "
           "facing the city. The avenue's sidewalks end and it runs on as Route 9.")]),
        ("4 · The same edge in the Net", "",
         [("Now", copy(NETF), "Today the Net ends at the same line."),
          ("Grown", net_edge(),
           "The district's green tint and the grid's brightness fade out past the last "
           "constructs; the avenue's lane runs on east as Route 9's trunk with packets on "
           "it. The body shop is a small construct; the shanties aren't on the grid, but one "
           "has spliced a stolen line (<code>┆</code>, the amber node) off the trunk; the "
           "billboard's ad server is a lone node on a spur.")]),
    ]
    out = ["<!doctype html><meta charset=utf-8><title>Overworld · round 2</title>",
           "<style>%s</style>" % CSS,
           "<h1>The overworld · round 2: one level down</h1>",
           "<p>Open ground at walking scale, the same ground in the Net, and the city's edge. "
           "All at 22:00 in the game's own frame. The open ground is drawn with the "
           "street's palettes plus new ground terrains (dirt, scrub, wash, rock, gravel, "
           "track, fence, lot).</p>"]
    chk = os.environ.get("CHECK_DIR")
    n = 0
    for title, intro, frames in secs:
        out.append("<h2>%s</h2>" % html.escape(title))
        if intro:
            out.append("<p>%s</p>" % intro)
        for sub, g, text in frames:
            out.append("<h3>%s</h3><p>%s</p>" % (html.escape(sub), text))
            out.append(term.render(g, 13.0))
            n += 1
            if chk:
                open(os.path.join(chk, "r2_%d.html" % n), "w").write(
                    "<!doctype html><meta charset=utf-8><style>%s body{margin:0;"
                    "background:#000}</style>%s" % (term.CSS, term.render(g, 14.0)))
    open(OUT, "w").write("\n".join(out))
    print(OUT)


if __name__ == "__main__":
    main()
