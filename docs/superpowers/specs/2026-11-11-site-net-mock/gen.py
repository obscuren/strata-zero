"""#95 mock: a site's own Net, all showing one moment in Meridian Clearing, drawn over
the game's own construct frame at 120x40. Writes the screens as JSON for page.py.

    python3 -I gen.py shots parts.json && python3 -I page.py parts.json site-net.html

Run from this folder. `shots` holds net_inside.svg from the game's net_inside shot test.

D · open tower (agreed): C's section with the level you're on opened out to its plan.

A · floor plan: the level is the floor itself, walls as traces, every device a node
    where it stands; one level at a time, the hub's riser between them.
B · circuit: every level a bus off the riser, zone by zone, its doors gates on the
    line; the whole site at once, as wiring.
C · section: the tower cut open, every floor squashed to three rows, the lifts and
    the riser running through; floors the lift passes folded away.
"""

import heapq
import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.join(HERE, "..", "..", "..", "..")
sys.path.insert(0, REPO + "/docs/superpowers/specs/2026-10-19-gear-inventory-mock")
sys.path.insert(0, HERE)
from cells import Grid  # noqa: E402
import floors as F  # noqa: E402

SHOTS, OUT = sys.argv[1], sys.argv[2]

VOID = "#010204"
PANEL = "#04060a"
WHITE = "#e6f0f5"
OPEN = "#3fa88f"
ICE_BLUE = "#9fe6ff"
RED = "#ff2848"
AMBER = "#ffb000"
TERMINAL = "#8c96ff"
ROOM = "#00e5ff"
YOU = "#468cff"
YOU_GLOW = "#0e2254"
SIGNAL = "#00ffc8"
TEXT = "#cdd4de"
DIM = "#8a93a3"
FAINT = "#4a525e"
RULE = "#283c4b"
LABEL = "#a0aab9"
DOT = "#0d2a24"
GREY_ICE = "#9aa4b0"
ICE = {"white": WHITE, "grey": GREY_ICE, "black": RED}

LIGHT = "─│─└││┌├─┘─┴┐┤┬┼"
DOUBLE = "═║═╚║║╔╠═╝═╩╗╣╦╬"
DASH = LIGHT.replace("─", "┄").replace("│", "┆")

MX, MY, MW, MH = 0, 5, 79, 30   # the map
PX = 81                          # the panel's text column


def rgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def hexc(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in t)


def scale(c, f):
    return hexc(tuple(v * f for v in rgb(c)))


def mix(a, b, f):
    a, b = rgb(a), rgb(b)
    return hexc(tuple(a[i] + (b[i] - a[i]) * f for i in range(3)))


def mask(x, y, joins):
    m = 0
    for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
        if joins(x + dx, y + dy):
            m |= bit
    return m


def cell(g, x, y, ch, fg, bg=None, bold=False):
    if 0 <= x < g.w and 0 <= y < g.h:
        c = g.c[y][x]
        c[0], c[1], c[3] = ch, fg, bold
        if bg:
            c[2] = bg


def text(g, x, y, s, fg, bg=None, bold=False):
    for i, ch in enumerate(s):
        cell(g, x + i, y, ch, fg, bg, bold)


def mtext(g, x, y, s, fg, bg=None, bold=False):
    """Text on the map, cut at the panel's border."""
    for i, ch in enumerate(s):
        if x + i < MX + MW:
            cell(g, x + i, y, ch, fg, bg, bold)


# ------------------------------------------------------------------ the frame

def frame(level_key, place):
    g = Grid.from_svg(os.path.join(SHOTS, "net_inside.svg"), 120, 40)
    for y in range(MY, MY + MH):
        for x in range(MX, MX + MW):
            dot = y % 2 == 0 and x % 4 == 3
            g.c[y][x] = ["·" if dot else " ", DOT, VOID, False]
        for x in range(80, 120):
            g.c[y][x] = [" ", TEXT, PANEL, False]
    for x in range(80, 120):
        for y in (0, 2):
            g.c[y][x] = [" ", TEXT, PANEL, False]
    text(g, PX, 0, "Meridian Clearing", ROOM, PANEL, True)
    text(g, PX + 17, 0, " · ", ROOM, PANEL, True)
    text(g, PX + 20, 0, "construct", ROOM, PANEL, True)
    text(g, 112, 0, "☾ 22:40", TEXT, PANEL)
    text(g, PX, 2, place, TEXT, PANEL)
    for x in range(0, 79):
        g.c[36][x] = [" ", TEXT, PANEL, False]
    text(g, 1, 36, "⚡", ROOM, PANEL)
    text(g, 4, 36, "Meridian Clearing · Crown Circle", TEXT, PANEL)
    text(g, 47, 36, "TRACE", ROOM, PANEL)
    text(g, 53, 36, "▰▰" + "▱" * 8 + " 14", ROOM, PANEL)
    for x in range(0, 79):
        g.c[39][x] = [" ", TEXT, PANEL, False]
    text(g, 1, 39, "PROGRAMS", LABEL, PANEL)
    px = 12
    for key, name in (("[1]", "CRACK ●"), ("[2]", "COPY ●"), ("[3]", "SPOOF ●"), ("[4]", "──"), ("[5]", "──")):
        live = name != "──"
        text(g, px, 39, key, ROOM if live else LABEL, PANEL, live)
        text(g, px + 4, 39, name, TEXT if live else LABEL, PANEL)
        if live:
            cell(g, px + 4 + len(name) - 1, 39, "●", ROOM, PANEL)
        px += 4 + len(name) + 4
    return g


def rule(g, y):
    for x in range(80, 120):
        g.c[y][x] = ["─", RULE, PANEL, False]


def side_head(g, y=5):
    text(g, PX, y, "MERIDIAN CLEARING", RED, PANEL, True)
    text(g, PX + 17, y, " · office", TEXT, PANEL)
    text(g, PX, y + 1, "SEC ", TEXT, PANEL)
    text(g, PX + 4, y + 1, "190", WHITE, PANEL, True)
    text(g, PX + 7, y + 1, " · ICE white · vault black", TEXT, PANEL)
    text(g, PX, y + 2, "FIREWALL", RED, PANEL)
    text(g, PX + 9, y + 2, "▮▮▮", RED, PANEL, True)
    text(g, PX + 13, y + 2, "0/3", RED, PANEL)
    text(g, PX + 17, y + 2, "· in by the kiosk", DIM, PANEL)
    rule(g, y + 3)
    text(g, PX, y + 4, "JOB", LABEL, PANEL)
    text(g, PX + 4, y + 4, "copy the ledger · B1 vault", AMBER, PANEL)
    text(g, PX + 4, y + 5, "B1 has its own Net: jack in there", DIM, PANEL)
    rule(g, y + 6)
    return y + 7


def side_held(g, y):
    text(g, PX, y, "HELD", LABEL, PANEL)
    rows = [
        ("▯", "hub closet door", "cracked"),
        ("▪", "turnstiles", "cracked"),
        ("▪", "lobby camera", "looped"),
        ("▪", "goods lift", "stops at B1"),
    ]
    for i, (gl, name, what) in enumerate(rows):
        cell(g, PX + 1, y + 1 + i, gl, SIGNAL, PANEL, True)
        text(g, PX + 3, y + 1 + i, name, TEXT, PANEL)
        text(g, PX + 3 + len(name), y + 1 + i, " · " + what, SIGNAL, PANEL)
    return y + 1 + len(rows)


def side_body(g, y):
    text(g, PX, y, "BODY", LABEL, PANEL)
    text(g, PX + 5, y, "L · at the kiosk", TEXT, PANEL)
    cell(g, PX + 1, y + 1, "@", RED, PANEL, True)
    text(g, PX + 3, y + 1, "guard · front desk · idle", TEXT, PANEL)
    text(g, PX + 3, y + 2, "15 tiles · can't see you", DIM, PANEL)
    return y + 3


# ------------------------------------------------------------------ plan logic

def dims(lv):
    return len(lv["plan"][0]), len(lv["plan"])


def is_outer(lv, x, y):
    w, h = dims(lv)
    return x in (0, w - 1) or y in (0, h - 1)


def ch_at(lv, x, y):
    w, h = dims(lv)
    if 0 <= x < w and 0 <= y < h:
        return lv["plan"][y][x]
    return " "


WALLISH = set("#=o")
DOORS = set("KPG+T")


def passable(lv, x, y, open_gates=False):
    c = ch_at(lv, x, y)
    if c == " " or (c in WALLISH):
        return False
    if is_outer(lv, x, y):
        return False
    return True


def route(lv, start, goal_set, reuse):
    """Cheapest way over the plan from start to any goal: turns cost, old traces are
    nearly free, so the links merge into a tree."""
    best = {}
    q = [(0.0, start, None, [start])]
    while q:
        cost, p, d, path = heapq.heappop(q)
        if p in goal_set and p != start:
            return path
        key = (p, d)
        if key in best and best[key] <= cost:
            continue
        best[key] = cost
        for dd in ((0, -1), (1, 0), (0, 1), (-1, 0)):
            n = (p[0] + dd[0], p[1] + dd[1])
            if not passable(lv, *n):
                continue
            step = 0.15 if n in reuse else 1.0
            if ch_at(lv, *n) in DOORS and n not in goal_set:
                step += 0.5
            turn = 2.0 if d is not None and d != dd else 0.0
            heapq.heappush(q, (cost + step + turn, n, dd, path + [n]))
    return None


def traces(lv):
    """Tiles carrying a device's link to its floor's hub."""
    hub = next((x, y) for x, y, k in F.nodes(lv) if k == "hub")
    tree = {hub}
    links = set()
    devs = [(x, y) for x, y, k in F.nodes(lv) if k != "hub"]
    devs.sort(key=lambda p: abs(p[0] - hub[0]) + abs(p[1] - hub[1]))
    for d in devs:
        path = route(lv, d, tree, links)
        if not path:
            continue
        for p in path:
            tree.add(p)
            links.add(p)
    return links


def cord_path(lv):
    k = F.JACKED[1:]
    you = F.YOU_NET[1:]
    return route(lv, k, {you}, set()) or []


# ------------------------------------------------------------------ node look

def node_look(lv, x, y, kind):
    st = F.state(lv, x, y)
    held = st is not None
    g = {"camera": "▪", "card door": "▯", "keypad": "▯", "gate switch": "▪",
         "lift": "▪", "terminal": "◇", "server": "◆", "data vault": "⌗",
         "safe": "▪", "turret": "▪", "hub": "⬡"}[kind]
    if held:
        return g, SIGNAL, mix(VOID, SIGNAL, 0.28), True
    if (lv["key"], x, y) == F.JACKED:
        return g, WHITE, mix(VOID, TERMINAL, 0.45), True
    fg = {"terminal": TERMINAL, "server": WHITE, "data vault": AMBER, "safe": AMBER,
          "turret": "#ff7a45", "hub": WHITE}.get(kind, ICE_BLUE)
    bold = kind in ("server", "data vault", "safe", "hub")
    return g, fg, None, bold


def ice_cells(g, sx, sy, grade, bg_of):
    tint = ICE[grade]
    cell(g, sx, sy, "◈", tint, mix(VOID, tint, 0.35), True)


SHORT = {"camera": "cam", "card door": "door", "keypad": "keypad", "gate switch": "gate",
         "lift": "lift", "terminal": "term", "server": "server", "data vault": "vault",
         "safe": "safe", "turret": "turret", "hub": "hub"}


# ------------------------------------------------------------------ A · plan

def look_a():
    lv = F.BY_KEY["L"]
    g = frame("L", "construct · L lobby")
    ox, oy = 14, 15
    links = traces(lv)
    cord = cord_path(lv)
    floor_bg = mix(VOID, ROOM, 0.10)
    wall_fg = scale(ROOM, 0.6)
    ice = {pos: grade for grade, pos in lv["ice"]}
    watch = set()
    for (ix, iy), grade in ice.items():
        for dy in range(-3, 4):
            for dx in range(-5, 6):
                if abs(dx) / 5 + abs(dy) / 3 <= 1 and passable(lv, ix + dx, iy + dy):
                    watch.add((ix + dx, iy + dy))
    kinds = {(x, y): k for x, y, k in F.nodes(lv)}

    def wall_join(a, b):
        return ch_at(lv, a, b) in WALLISH or ch_at(lv, a, b) in DOORS or (a, b) in kinds and ch_at(lv, a, b) in "KPG"

    for y in range(F.H):
        for x in range(F.W):
            c = ch_at(lv, x, y)
            sx, sy = ox + x, oy + y
            if is_outer(lv, x, y):
                m = mask(x, y, lambda a, b: 0 <= a < F.W and 0 <= b < F.H and is_outer(lv, a, b))
                cell(g, sx, sy, DOUBLE[m], RED, mix(VOID, RED, 0.12), True)
                continue
            if c == "#":
                m = mask(x, y, wall_join)
                cell(g, sx, sy, LIGHT[m], wall_fg, mix(VOID, ROOM, 0.05))
                continue
            bg = floor_bg
            if (x, y) in watch:
                bg = mix(bg, ICE["white"], 0.10)
            if c == "T":
                gate_open = F.state(lv, 27, 5) is not None
                cell(g, sx, sy, "┄", SIGNAL if gate_open else ICE_BLUE, mix(VOID, SIGNAL if gate_open else ICE_BLUE, 0.12))
                continue
            if c == "+":
                cell(g, sx, sy, " ", wall_fg, bg)
                continue
            if (x, y) in kinds:
                gl, fg, nbg, bold = node_look(lv, x, y, kinds[(x, y)])
                cell(g, sx, sy, gl, fg, nbg or bg, bold)
                continue
            if (x, y) in links:
                m = mask(x, y, lambda a, b: (a, b) in links or (a, b) in kinds)
                cell(g, sx, sy, LIGHT[m], mix(bg, SIGNAL, 0.38), bg)
                continue
            cell(g, sx, sy, " ", TEXT, bg)
    # The cord home, from the kiosk to where she stands.
    cs = set(cord)
    for (x, y) in cord[1:-1]:
        if (x, y) in kinds or ch_at(lv, x, y) == "T":
            continue
        m = mask(x, y, lambda a, b: (a, b) in cs)
        cell(g, ox + x, oy + y, DASH[m], YOU, None, True)
    for (x, y), grade in ice.items():
        ice_cells(g, ox + x, oy + y, grade, None)
    yx, yy = F.YOU_NET[1:]
    cell(g, ox + yx, oy + yy, "@", YOU, YOU_GLOW, True)
    for lx, ly, word in lv["labels"]:
        for i, ch in enumerate(word):
            c0 = g.c[oy + ly][ox + lx + i]
            if c0[0] == " ":
                cell(g, ox + lx + i, oy + ly, ch, mix(floor_bg, ROOM, 0.35))
    # The riser out of the hub.
    cell(g, ox + 13, oy + 1, "║", WHITE, mix(VOID, ROOM, 0.18), True)
    # Title, and the street under the front.
    for yy in (oy - 3, oy - 2):
        for xx in range(MX, MX + MW):
            g.c[yy][xx] = [" ", DOT, VOID, False]
    text(g, ox, oy - 3, "L", WHITE, VOID, True)
    text(g, ox + 2, oy - 3, "lobby", TEXT, VOID)
    text(g, ox + 9, oy - 3, "· 12 nodes · 4 held", DIM, VOID)
    text(g, ox + 34, oy - 3, "⬡ ", WHITE, VOID, True)
    text(g, ox + 36, oy - 3, ">", ROOM, VOID, True)
    text(g, ox + 38, oy - 3, "up the riser to 7F", DIM, VOID)
    text(g, ox + 34, oy - 2, "⬡ ", WHITE, VOID, True)
    text(g, ox + 36, oy - 2, "<", ROOM, VOID, True)
    text(g, ox + 38, oy - 2, "B1 is cut off", RED, VOID)
    text(g, ox + 15, oy + F.H, "▔▔▔▔▔▔▔▔ street ▔▔▔▔▔▔▔▔", FAINT, VOID)
    # Panel.
    y = side_head(g)
    text(g, PX, y, "LEVELS", LABEL, PANEL)
    rows = [("12F", "executive", "▪10", "◈ grey", ""),
            ("7F", "security", "▪10", "◈ white", "◆"),
            ("L", "lobby", "▪12", "◈ white", "@"),
            ("B1", "servers", "", "", "")]
    for i, (k, name, n, ic, mark) in enumerate(rows):
        yy = y + 1 + i
        here = k == "L"
        bg = mix(PANEL, ROOM, 0.12) if here else PANEL
        for x in range(80, 120):
            g.c[yy][x][2] = bg
        cell(g, PX, yy, "▸" if here else " ", ROOM, bg, True)
        text(g, PX + 2, yy, k, WHITE if here else TEXT, bg, True)
        text(g, PX + 6, yy, name, TEXT if here else DIM, bg)
        if k == "B1":
            text(g, PX + 16, yy, "AIR GAP ", RED, bg, True)
            text(g, PX + 24, yy, "◈ black ⌗", RED, bg)
            continue
        text(g, PX + 16, yy, n, ICE_BLUE, bg)
        text(g, PX + 21, yy, ic, ICE[ic.split()[1]], bg)
        text(g, PX + 30, yy, mark, YOU if mark == "@" else WHITE, bg, True)
    y += 6
    rule(g, y)
    y = side_held(g, y + 1)
    rule(g, y)
    side_body(g, y + 1)
    return g


# ------------------------------------------------------------------ B · circuit

ZONES = {
    # Per level: the hub's chain, zone by zone; ("gate", x, y) breaks the bus.
    "12F": [[], ("gate", 18, 3), [(23, 1), (27, 1), (1, 1), (1, 6), (11, 6), (48, 11), (46, 5)],
            ("gate", 14, 8), [(19, 10), (1, 11)]],
    "7F": [[], ("gate", 18, 3), [(23, 1), (27, 1), (34, 1), (1, 6), (48, 10), (46, 5)],
           ("gate", 4, 5), [(3, 3), (6, 3)]],
    "L": [[], ("gate", 18, 3), [(23, 1), (27, 1), (31, 1), (32, 3)],
          ("gate", 27, 5), [(1, 6), (1, 1), (46, 5), (48, 9), (43, 10)]],
    "B1": [[], ("gate", 18, 3), [(23, 1), (27, 1), (32, 3), (46, 5)],
           ("gate", 28, 5), [(26, 6), (39, 8)], ("gate", 25, 8), [(1, 6), (7, 10)]],
}
NAMES = {
    ("L", 23, 1): "lift", ("L", 27, 1): "goods", ("L", 31, 1): "cam",
    ("L", 32, 3): "office", ("L", 1, 6): "cam", ("L", 1, 1): "cam", ("L", 46, 5): "stair",
    ("L", 48, 9): "cam", ("L", 43, 10): "kiosk", ("L", 27, 5): "gates",
    ("L", 18, 3): "closet",
    ("7F", 34, 1): "cam", ("7F", 1, 6): "cam", ("7F", 48, 10): "cam", ("7F", 46, 5): "stair",
    ("7F", 3, 3): "desk", ("7F", 6, 3): "server", ("7F", 4, 5): "sec",
    ("7F", 18, 3): "closet", ("7F", 23, 1): "lift", ("7F", 27, 1): "goods",
    ("12F", 1, 1): "cam", ("12F", 1, 6): "cam", ("12F", 11, 6): "turret",
    ("12F", 48, 11): "cam", ("12F", 46, 5): "stair", ("12F", 14, 8): "keypad",
    ("12F", 19, 10): "safe", ("12F", 1, 11): "cam", ("12F", 18, 3): "closet",
    ("12F", 23, 1): "lift", ("12F", 27, 1): "goods",
    ("B1", 23, 1): "lift", ("B1", 27, 1): "goods", ("B1", 32, 3): "store",
    ("B1", 46, 5): "stair", ("B1", 28, 5): "noc", ("B1", 26, 6): "cam",
    ("B1", 39, 8): "maint", ("B1", 25, 8): "racks", ("B1", 1, 6): "cam",
    ("B1", 7, 10): "vault", ("B1", 18, 3): "closet",
}


def look_b():
    g = frame("all", "construct · the whole site")
    kinds_of = {lv["key"]: {(x, y): k for x, y, k in F.nodes(lv)} for lv in F.LEVELS}
    riser_x = 12
    bus_l, bus_r = 14, 76
    top = 8
    bus_rows = {}
    order = ["12F", "7F", "L", "B1"]
    for i, key in enumerate(order):
        bus_rows[key] = top + 2 + i * 6 + (1 if key == "B1" else 0)
    # The firewall: a box round the levels on the riser, its three locks on three sides.
    fx0, fx1, fy0, fy1 = 9, 78, top - 1, bus_rows["L"] + 3
    for x in range(fx0, fx1 + 1):
        for y in (fy0, fy1):
            m = mask(x, y, lambda a, b: (b in (fy0, fy1) and fx0 <= a <= fx1) or (a in (fx0, fx1) and fy0 <= b <= fy1))
            cell(g, x, y, DOUBLE[m], RED, mix(VOID, RED, 0.12), True)
    for y in range(fy0, fy1 + 1):
        for x in (fx0, fx1):
            m = mask(x, y, lambda a, b: (b in (fy0, fy1) and fx0 <= a <= fx1) or (a in (fx0, fx1) and fy0 <= b <= fy1))
            cell(g, x, y, DOUBLE[m], RED, mix(VOID, RED, 0.12), True)
    lock_row = bus_rows["7F"] - 3
    locks = [(fx0, lock_row), (40, fy0), (fx1, lock_row)]
    for (x, y) in locks:
        cell(g, x, y, "▮", RED, mix(VOID, RED, 0.3), True)
    text(g, 0, lock_row, "grid ════", FAINT, VOID)
    text(g, 36, fy0 - 1, "grid ║", FAINT, VOID)
    text(g, 41, fy0 - 1, " FIREWALL · 3 locks", RED, VOID)
    # The riser.
    for y in range(bus_rows["12F"], bus_rows["L"] + 1):
        cell(g, riser_x, y, "║", scale(WHITE, 0.7), VOID, True)
    for key in order:
        lv = F.BY_KEY[key]
        by = bus_rows[key]
        kinds = kinds_of[key]
        chain = ZONES[key]
        # Lay the chain out along the bus.
        slots = []
        for part in chain:
            if isinstance(part, tuple):
                slots.append(("gate", (part[1], part[2])))
            else:
                for p in part:
                    slots.append(("dev", p))
        n = len(slots)
        span = bus_r - bus_l - 4
        right = 69
        xs = [bus_l + 4 + round(i * (right - bus_l - 4) / max(1, n - 1)) for i in range(n)]
        bus_bg = VOID
        gap = lv.get("gap")
        line = scale(SIGNAL, 0.55) if not gap else scale(RED, 0.5)
        for x in range(bus_l, bus_r + 1):
            cell(g, x, by, "═", line, mix(VOID, SIGNAL if not gap else RED, 0.10))
        # Level name and hub.
        text(g, 1, by, key, WHITE, VOID, True)
        text(g, 1, by + 1, {"executive": "exec"}.get(lv["name"], lv["name"])[:8], DIM, VOID)
        hub_bg = mix(VOID, ROOM, 0.25)
        cell(g, riser_x, by, "⬡", WHITE, hub_bg, True)
        cell(g, riser_x + 1, by, "═", line, None)
        up = True
        for (what, p), x in zip(slots, xs):
            if what == "gate":
                gl, fg, nbg, bold = node_look(lv, p[0], p[1], kinds.get(p, "card door"))
                gl = "▯"
                cell(g, x, by, gl, fg, nbg or mix(VOID, fg, 0.25), True)
                name = NAMES.get((key, p[0], p[1]), "door")
                mtext(g, x - len(name) // 2, by - 1 if up else by + 1, name, scale(fg, 0.85), VOID)
                up = not up
                continue
            kind = kinds.get(p)
            gl, fg, nbg, bold = node_look(lv, p[0], p[1], kind)
            ry = by - 2 if up else by + 2
            cell(g, x, by, "╧" if up else "╤", line, None)
            cell(g, x, by - 1 if up else by + 1, "│", scale(line, 0.8), VOID)
            cell(g, x, ry, gl, fg, nbg or VOID, bold)
            name = NAMES.get((key, p[0], p[1]), SHORT[kind])
            mtext(g, x + 2, ry, name, scale(fg, 0.85) if kind not in ("terminal",) else TERMINAL, VOID)
            up = not up
        for grade, (ix, iy) in lv["ice"]:
            # ICE rides the bus, near its plan x.
            x = bus_l + 3 + round(ix * span / (F.W - 1))
            while g.c[by][x][0] != "═":
                x += 1
            cell(g, x, by, "◈", ICE[grade], mix(VOID, ICE[grade], 0.35), True)
    # Air gap under L.
    lb, bb = bus_rows["L"], bus_rows["B1"]
    for y in range(lb + 1, fy1):
        cell(g, riser_x, y, "║", scale(WHITE, 0.7), VOID, True)
    cell(g, riser_x, fy1, "╳", RED, mix(VOID, RED, 0.2), True)
    text(g, riser_x + 2, fy1 + 1, "AIR GAP", RED, VOID, True)
    text(g, riser_x + 10, fy1 + 1, "· B1's Net doesn't reach the riser", DIM, VOID)
    cell(g, riser_x, fy1 + 1, " ", RED, VOID)
    # Where she is: the lift lobby's stretch of L's bus, the cord back to the kiosk.
    lby = bus_rows["L"]
    yx = 36
    cell(g, yx, lby, "@", YOU, YOU_GLOW, True)
    kiosk_x = next(x for x in range(bus_r, bus_l, -1)
                   if g.c[lby - 2][x][0] == "◇" or g.c[lby + 2][x][0] == "◇")
    for x in range(yx + 1, kiosk_x + 1):
        if g.c[lby][x][0] == "═":
            cell(g, x, lby, "┅", YOU, mix(VOID, YOU, 0.2), True)
    # Panel: the meat under the node you're on, as a little plan.
    y = side_head(g)
    text(g, PX, y, "UNDER YOU", LABEL, PANEL)
    text(g, PX + 10, y, "L · the lift lobby", TEXT, PANEL)
    lv = F.BY_KEY["L"]
    mini_x, mini_y = PX + 7, y + 2
    pix = []
    wall, floor = scale(ROOM, 0.55), mix(PANEL, ROOM, 0.12)
    for yy in range(F.H):
        rowp = []
        for xx in range(0, F.W, 2):
            pair = {(xx, yy), (xx + 1, yy)}
            chars = [ch_at(lv, *p) for p in sorted(pair)]
            if pair & {(25, 3), (7, 9)}:
                rowp.append(RED)
            elif (20, 4) in pair:
                rowp.append(YOU)
            elif (44, 10) in pair:
                rowp.append(TERMINAL)
            elif any(c in "#=o" for c in chars):
                rowp.append(wall)
            elif "T" in chars or "G" in chars:
                rowp.append(SIGNAL)
            else:
                rowp.append(floor)
        pix.append(rowp)
    if len(pix) % 2:
        pix.append([PANEL] * len(pix[0]))
    for j in range(0, len(pix), 2):
        for i, (t, b) in enumerate(zip(pix[j], pix[j + 1])):
            cell(g, mini_x + i, mini_y + j // 2, "▀", t, b)
    y = mini_y + len(pix) // 2 + 1
    text(g, PX, y, "@", YOU, PANEL, True)
    text(g, PX + 2, y, "you on the Net", DIM, PANEL)
    cell(g, PX + 18, y, "■", RED, PANEL)
    text(g, PX + 20, y, "guards", DIM, PANEL)
    cell(g, PX + 28, y, "■", TERMINAL, PANEL)
    text(g, PX + 30, y, "body", DIM, PANEL)
    rule(g, y + 1)
    y = side_held(g, y + 2)
    rule(g, y)
    side_body(g, y + 1)
    return g


# ------------------------------------------------------------------ C · section

def look_c():
    g = frame("all", "construct · the tower in section")
    bx = 16
    order = ["12F", "7F", "L", "B1"]
    roof = 8
    tops = {}
    y = roof + 1
    folds = {}
    for key in order:
        if key == "B1":
            y += 0
        tops[key] = y
        y += 4
        if key in F.PASSED:
            folds[key] = y
            y += 1
    ground = tops["L"] + 3
    # Earth around the basement.
    for yy in range(ground + 1, tops["B1"] + 5):
        for xx in range(4, 75):
            cell(g, xx, yy, "░", mix(VOID, "#3a2f26", 0.6), VOID)
    for xx in range(2, 77):
        cell(g, xx, ground, "▔", scale(TEXT, 0.35), VOID)
    text(g, 3, ground - 1, "street", FAINT, VOID)
    text(g, 70, ground - 1, "street", FAINT, VOID)
    kinds_of = {lv["key"]: {(x, y): k for x, y, k in F.nodes(lv)} for lv in F.LEVELS}
    floor_bg = mix(VOID, ROOM, 0.10)

    def band(py):
        return 0 if py <= 4 else 1 if py == 5 else 2

    for key in order:
        lv = F.BY_KEY[key]
        t = tops[key]
        gap = lv.get("gap")
        bg = floor_bg if not gap else mix(VOID, RED, 0.07)
        for r in range(3):
            for x in range(F.W):
                cell(g, bx + x, t + r, " ", TEXT, bg)
        # Walls cut by the section.
        for x in range(F.W):
            if all(ch_at(lv, x, yy) in WALLISH for yy in range(1, 5)):
                cell(g, bx + x, t, "│", scale(ROOM, 0.5), bg)
            if all(ch_at(lv, x, yy) in WALLISH for yy in range(6, 12)):
                cell(g, bx + x, t + 2, "│", scale(ROOM, 0.5), bg)
            c = ch_at(lv, x, 5)
            if c in WALLISH:
                cell(g, bx + x, t + 1, "─", scale(ROOM, 0.5), bg)
            elif c == "T":
                cell(g, bx + x, t + 1, "┄", SIGNAL, bg)
        # Slab under the floor.
        for x in range(-1, F.W + 1):
            cell(g, bx + x, t + 3, "━", scale(ROOM, 0.45) if not gap else scale(RED, 0.45), VOID)
        # Nodes, squashed into three rows.
        taken = set()
        kinds = kinds_of[key]
        for (px, py), kind in sorted(kinds.items()):
            r = band(py)
            x = px
            while (x, r) in taken:
                x += 1
            taken.add((x, r))
            gl, fg, nbg, bold = node_look(lv, px, py, kind)
            cell(g, bx + x, t + r, gl, fg, nbg or bg, bold)
        for grade, (ix, iy) in lv["ice"]:
            cell(g, bx + ix, t + band(iy), "◈", ICE[grade], mix(VOID, ICE[grade], 0.35), True)
        # Left: the level; right: what's on it.
        text(g, bx - 3 - len(key), t, key, WHITE, VOID, True)
        name = {"executive": "exec"}.get(lv["name"], lv["name"])
        text(g, bx - 3 - len(name), t + 1, name, DIM, VOID)
        if gap:
            text(g, bx + F.W + 2, t, "AIR GAP", RED, VOID, True)
            text(g, bx + F.W + 2, t + 1, "own Net", DIM, VOID)
            text(g, bx + F.W + 2, t + 2, "jack in", DIM, VOID)
    for key, fy in folds.items():
        for x in range(-1, F.W + 1):
            cell(g, bx + x, fy, "╌", scale(ROOM, 0.25), VOID)
    # Lift shafts through the folds.
    for key, fy in folds.items():
        for x in (23, 27):
            cell(g, bx + x, fy, "║", scale(ICE_BLUE, 0.5), VOID)
        label = " " + F.PASSED[key] + " passed "
        text(g, bx + 31, fy, label, FAINT, VOID)
    # The riser: hub to hub, the roof to L; cut above B1.
    rx = bx + 13
    hubs = [tops[k] for k in order]
    for yy in range(roof, tops["L"] + 1):
        if g.c[yy][rx][0] in (" ", "━", "╌", "─", "│"):
            cell(g, rx, yy, "║", scale(WHITE, 0.7), None, True)
    cell(g, rx, tops["L"] + 3, "╳", RED, mix(VOID, RED, 0.2), True)
    # The firewall: walls and roof above the street, three locks.
    fx0, fx1 = bx - 2, bx + F.W + 1
    fy0, fy1 = roof - 1, ground - 1
    for x in range(fx0, fx1 + 1):
        cell(g, x, fy0, DOUBLE[mask(x, fy0, lambda a, b: b == fy0 and fx0 <= a <= fx1 or a in (fx0, fx1) and fy0 <= b <= fy1)], RED, mix(VOID, RED, 0.12), True)
    for yy in range(fy0, fy1 + 1):
        for x in (fx0, fx1):
            m = mask(x, yy, lambda a, b: b == fy0 and fx0 <= a <= fx1 or a in (fx0, fx1) and fy0 <= b <= fy1)
            cell(g, x, yy, DOUBLE[m], RED, mix(VOID, RED, 0.12), True)
    lock_row = folds["12F"]
    for (x, yy) in ((fx0, lock_row), (rx, fy0), (fx1, lock_row)):
        cell(g, x, yy, "▮", RED, mix(VOID, RED, 0.3), True)
    text(g, rx - 2, fy0 - 1, "grid", FAINT, VOID)
    text(g, rx + 3, fy0 - 1, "FIREWALL · 3 locks", RED, VOID)
    text(g, fx0 - 8, lock_row, "grid ══", FAINT, VOID)
    text(g, fx1 + 1, lock_row, "══ grid", FAINT, VOID)
    # You, and the cord back to the kiosk.
    lt = tops["L"]
    cell(g, bx + 20, lt, "@", YOU, YOU_GLOW, True)
    for x in range(28, 43):
        if g.c[lt + 2][bx + x][0] in " ┅":
            cell(g, bx + x, lt + 2, "┅", YOU, None, True)
    cell(g, bx + 27, lt + 2, "┗", YOU, None, True)
    for x in range(21, 27):
        if g.c[lt][bx + x][0] == " ":
            cell(g, bx + x, lt, "┅", YOU, None, True)
    # Panel.
    y = side_head(g)
    text(g, PX, y, "IN SECTION", LABEL, PANEL)
    lines = [("each floor is three rows:", DIM), ("  its back rooms, the wall", DIM),
             ("  with its doors, the front", DIM)]
    for i, (s, c) in enumerate(lines):
        text(g, PX, y + 1 + i, s, c, PANEL)
    rule(g, y + 4)
    y = side_held(g, y + 5)
    rule(g, y)
    side_body(g, y + 1)
    return g


# ------------------------------------------------------------------ D · open tower

def d_node(kind, pos, held, jacked, dim):
    g = {"camera": "▪", "card door": "▯", "keypad": "▯", "gate switch": "▪",
         "lift": "▪", "terminal": "◇", "server": "◆", "data vault": "⌗",
         "safe": "▪", "turret": "▪", "hub": "⬡"}[kind]
    if pos in held:
        fg, bg, bold = SIGNAL, mix(VOID, SIGNAL, 0.28), True
    elif pos == jacked:
        fg, bg, bold = WHITE, mix(VOID, TERMINAL, 0.45), True
    else:
        fg = {"terminal": TERMINAL, "server": WHITE, "data vault": AMBER, "safe": AMBER,
              "turret": "#ff7a45", "hub": WHITE}.get(kind, ICE_BLUE)
        bg, bold = None, kind in ("server", "data vault", "safe", "hub")
    if dim:
        fg = mix(VOID, fg, 0.45)
        bg = mix(VOID, bg, 0.45) if bg else None
    return g, fg, bg, bold


def d_open(g, lv, ox, oy, held, jacked, you):
    """A level opened out to its full plan."""
    key = lv["key"]
    w, h = dims(lv)
    gap = lv.get("gap")
    links = traces(lv)
    floor_bg = mix(VOID, ROOM, 0.10) if not gap else mix(VOID, RED, 0.06)
    wall_fg = scale(ROOM, 0.6)
    kinds = {(x, y): k for x, y, k in F.nodes(lv)}
    ice = {pos: grade for grade, pos in lv["ice"]}
    watch = {}
    for (ix, iy), grade in ice.items():
        for dy in range(-3, 4):
            for dx in range(-5, 6):
                if abs(dx) / 5 + abs(dy) / 3 <= 1 and passable(lv, ix + dx, iy + dy):
                    watch[(ix + dx, iy + dy)] = grade
    gate_open = any(k[0] == key and ch_at(lv, k[1], k[2]) == "G" for k in held)
    outer_fg = RED if gap else scale(ROOM, 0.8)

    def wall_join(a, b):
        return ch_at(lv, a, b) in WALLISH or ch_at(lv, a, b) in DOORS

    for y in range(h):
        for x in range(w):
            c = ch_at(lv, x, y)
            sx, sy = ox + x, oy + y
            if is_outer(lv, x, y):
                m = mask(x, y, lambda a, b: 0 <= a < w and 0 <= b < h and is_outer(lv, a, b))
                cell(g, sx, sy, DOUBLE[m], outer_fg, mix(VOID, outer_fg, 0.12), gap)
                continue
            if c == "#":
                cell(g, sx, sy, LIGHT[mask(x, y, wall_join)], wall_fg, mix(VOID, ROOM, 0.05))
                continue
            bg = floor_bg
            if (x, y) in watch:
                bg = mix(bg, ICE[watch[(x, y)]], 0.10)
            if c == "T":
                col = SIGNAL if gate_open else ICE_BLUE
                cell(g, sx, sy, "┄", col, mix(VOID, col, 0.12))
                continue
            if c == "+":
                cell(g, sx, sy, " ", wall_fg, bg)
                continue
            if (x, y) in kinds:
                gl, fg, nbg, bold = d_node(kinds[(x, y)], (key, x, y), held, jacked, False)
                cell(g, sx, sy, gl, fg, nbg or bg, bold)
                continue
            if (x, y) in links:
                m = mask(x, y, lambda a, b: (a, b) in links or (a, b) in kinds)
                cell(g, sx, sy, LIGHT[m], mix(bg, SIGNAL, 0.38), bg)
                continue
            cell(g, sx, sy, " ", TEXT, bg)
    for lx, ly, word in lv["labels"]:
        for i, ch in enumerate(word):
            if g.c[oy + ly][ox + lx + i][0] == " ":
                cell(g, ox + lx + i, oy + ly, ch, mix(floor_bg, ROOM, 0.35))
    if jacked and jacked[0] == key and you and you[0] == key:
        path = route(lv, jacked[1:], {you[1:]}, set()) or []
        cs = set(path)
        for (x, y) in path[1:-1]:
            if (x, y) in kinds or ch_at(lv, x, y) == "T":
                continue
            cell(g, ox + x, oy + y, DASH[mask(x, y, lambda a, b: (a, b) in cs)], YOU, None, True)
    for (x, y), grade in ice.items():
        ice_cells(g, ox + x, oy + y, grade, None)
    if you and you[0] == key:
        cell(g, ox + you[1], oy + you[2], "@", YOU, YOU_GLOW, True)


def d_strip(g, lv, bx, t, held, dim):
    """A level folded to three rows: its back rooms, the wall with its doors, the front."""
    key = lv["key"]
    w, h = dims(lv)
    wall_row = F.ny(5)
    gap = lv.get("gap")
    bg = mix(VOID, ROOM, 0.10) if not gap else mix(VOID, RED, 0.07)
    if dim:
        bg = mix(VOID, bg, 0.5)
    line = scale(ROOM, 0.5 if not dim else 0.25)
    for r in range(3):
        for x in range(w):
            cell(g, bx + x, t + r, " ", TEXT, bg)
    for x in range(w):
        if all(ch_at(lv, x, yy) in WALLISH for yy in range(1, wall_row)):
            cell(g, bx + x, t, "│", line, bg)
        if all(ch_at(lv, x, yy) in WALLISH for yy in range(wall_row + 1, h - 1)):
            cell(g, bx + x, t + 2, "│", line, bg)
        c = ch_at(lv, x, wall_row)
        if c in WALLISH:
            cell(g, bx + x, t + 1, "─", line, bg)
        elif c == "T":
            open_ = any(k[0] == key and ch_at(lv, k[1], k[2]) == "G" for k in held)
            col = SIGNAL if open_ else ICE_BLUE
            cell(g, bx + x, t + 1, "┄", mix(VOID, col, 0.45) if dim else col, bg)
    taken = set()
    for (px, py, kind) in sorted(F.nodes(lv)):
        r = 0 if py < wall_row else 1 if py == wall_row else 2
        x = px
        while (x, r) in taken:
            x += 1
        taken.add((x, r))
        gl, fg, nbg, bold = d_node(kind, (key, px, py), held, None, dim)
        cell(g, bx + x, t + r, gl, fg, nbg or bg, bold)
    for grade, (ix, iy) in lv["ice"]:
        r = 0 if iy < wall_row else 1 if iy == wall_row else 2
        col = ICE[grade] if not dim else mix(VOID, ICE[grade], 0.45)
        cell(g, bx + ix, t + r, "◈", col, mix(VOID, col, 0.35), True)


def look_d(moment):
    order = ["12F", "7F", "L", "B1"]
    B = F.BIG_BY_KEY
    base_held = {F.big_pos(*k): v for k, v in F.HELD.items()}
    if moment == 1:
        open_key, place = "L", "construct · L lobby"
        held = base_held
        jacked, you = F.big_pos(*F.JACKED), F.big_pos(*F.YOU_NET)
        reach = {"12F", "7F", "L"}
    else:
        open_key, place = "B1", "construct · B1 servers"
        held = dict(base_held)
        held[F.big_pos("B1", 25, 8)] = "cracked"
        held[F.big_pos("B1", 26, 6)] = "looped"
        jacked, you = F.big_pos("B1", 39, 8), F.big_pos("B1", 21, 8)
        reach = {"B1"}
    g = frame(open_key, place)
    bx = 11
    w, h = F.W2, F.H2
    fw_top = MY + 1
    y = fw_top + 1
    tops, seps = {}, {}
    for key in order:
        tops[key] = y
        y += h if key == open_key else 3
        seps[key] = y
        y += 1
    ground = seps["L"]
    rx = bx + F.nx(13)
    # The ground and the earth round the basement.
    for yy in range(ground + 1, MY + MH):
        for xx in range(bx - 3, bx + w + 3):
            cell(g, xx, yy, "░", mix(VOID, "#3a2f26", 0.55), VOID)
    for xx in range(1, 78):
        cell(g, xx, ground, "▔", scale(TEXT, 0.35), VOID)
    text(g, 1, ground - 1, "street", FAINT, VOID)
    text(g, 72, ground - 1, "street", FAINT, VOID)
    # Floors.
    for key in order:
        lv = B[key]
        t = tops[key]
        dim = key not in reach
        if key == open_key:
            d_open(g, lv, bx, t, held, jacked, you)
        else:
            d_strip(g, lv, bx, t, held, dim)
        gap = lv.get("gap")
        slab = scale(RED, 0.45) if gap else scale(ROOM, 0.45 if not dim else 0.22)
        for x in range(-1, w + 1):
            cell(g, bx + x, seps[key], "━", slab, VOID)
        if key in F.PASSED:
            for x in (F.nx(23), F.nx(27)):
                cell(g, bx + x, seps[key], "║", scale(ICE_BLUE, 0.5), VOID)
            text(g, bx + F.nx(31), seps[key], " " + F.PASSED[key] + " passed ", FAINT, VOID)
        # The level's name on the left.
        hi = key == open_key
        text(g, bx - 3 - len(key), t, key, WHITE if not dim else FAINT, VOID, True)
        name = {"executive": "exec"}.get(lv["name"], lv["name"])
        text(g, bx - 3 - len(name), t + 1, name, DIM if not dim else FAINT, VOID)
        if hi:
            cell(g, bx - 2, t, "▸", ROOM, VOID, True)
            nn = sum(1 for n in F.nodes(lv) if n[2] != "hub")
            nh = sum(1 for k in held if k[0] == key)
            text(g, bx - 3 - len("%d ▪" % nn), t + 3, "%d ▪" % nn, ICE_BLUE, VOID)
            text(g, bx - 3 - len("%d held" % nh), t + 4, "%d held" % nh, SIGNAL, VOID)
    # The riser: the roof down through every floor on it; cut under L.
    last = tops["L"] + (F.ny(2) if open_key == "L" else 0)
    for yy in range(fw_top, last):
        if g.c[yy][rx][0] in (" ", "━", "─", "│", "═", "╦", "╤", "┬"):
            cell(g, rx, yy, "║", scale(WHITE, 0.7), None, True)
    cell(g, rx, ground, "╳", RED, mix(VOID, RED, 0.2), True)
    # The firewall: above the street, three locks.
    fx0, fx1 = bx - 2, bx + w + 1
    fy0, fy1 = fw_top, ground - 1

    def fw(a, b):
        return (b == fy0 and fx0 <= a <= fx1) or (a in (fx0, fx1) and fy0 <= b <= fy1)

    for x in range(fx0, fx1 + 1):
        cell(g, x, fy0, DOUBLE[mask(x, fy0, fw)], RED, mix(VOID, RED, 0.12), True)
    for yy in range(fy0, fy1 + 1):
        for x in (fx0, fx1):
            cell(g, x, yy, DOUBLE[mask(x, yy, fw)], RED, mix(VOID, RED, 0.12), True)
    lock_row = seps["12F"]
    for (x, yy) in ((fx0, lock_row), (rx, fy0), (fx1, lock_row)):
        cell(g, x, yy, "▮", RED, mix(VOID, RED, 0.3), True)
    text(g, rx + 4, fy0, " FIREWALL · 3 locks ", RED, mix(VOID, RED, 0.12))
    # Each lock on a lane of the city's grid: crack it from out there as well as in here.
    lanes = {(x, lock_row) for x in range(0, fx0)} | {(x, lock_row) for x in range(fx1 + 1, MX + MW)}
    lanes |= {(rx, yy) for yy in range(MY, fy0)}
    ends = lanes | {(fx0, lock_row), (fx1, lock_row), (rx, fy0)}
    lane_fg, lane_bg = mix(SIGNAL, WHITE, 0.25), mix(VOID, SIGNAL, 0.2)
    for (x, yy) in lanes:
        cell(g, x, yy, DOUBLE[mask(x, yy, lambda a, b: (a, b) in ends)], lane_fg, lane_bg)
    text(g, 1, lock_row - 1, "grid", scale(SIGNAL, 0.6), VOID)
    text(g, fx1 + 2, lock_row - 1, "grid", scale(SIGNAL, 0.6), VOID)
    text(g, rx + 2, MY, "grid", scale(SIGNAL, 0.6), VOID)
    # The air gap, said on the right.
    if open_key == "L":
        t = tops["B1"]
        text(g, fx1 + 1, t, "AIR GAP", RED, VOID, True)
        text(g, fx1 + 1, t + 1, "own Net", DIM, VOID)
        text(g, fx1 + 1, t + 2, "jack in", DIM, VOID)
    else:
        t = tops["L"]
        text(g, fx1 + 1, t, "out of", FAINT, VOID)
        text(g, fx1 + 1, t + 1, "reach", FAINT, VOID)
        text(g, fx1 + 1, ground + 1, "AIR GAP", RED, VOID, True)
    # Panel.
    y = side_head(g)
    text(g, PX, y, "LEVELS", LABEL, PANEL)
    rows = [("12F", "executive"), ("7F", "security"), ("L", "lobby"), ("B1", "servers")]
    count = {k: "▪%d" % sum(1 for n in F.nodes(B[k]) if n[2] != "hub") for k in order}
    marks = {"12F": (count["12F"], "◈ grey", ""), "7F": (count["7F"], "◈ white", "◆"),
             "L": (count["L"], "◈ white", "@" if open_key == "L" else ""),
             "B1": (count["B1"], "◈ black", "@" if open_key == "B1" else "⌗")}
    for i, (k, name) in enumerate(rows):
        yy = y + 1 + i
        here = k == open_key
        bg = mix(PANEL, ROOM, 0.12) if here else PANEL
        for x in range(80, 120):
            g.c[yy][x][2] = bg
        cell(g, PX, yy, "▸" if here else " ", ROOM, bg, True)
        reachable = k in reach
        text(g, PX + 2, yy, k, WHITE if reachable else FAINT, bg, True)
        text(g, PX + 6, yy, name, TEXT if here else (DIM if reachable else FAINT), bg)
        n, ic, mk = marks[k]
        if not reachable:
            text(g, PX + 16, yy, "out of reach" if k != "B1" else "AIR GAP", FAINT if k != "B1" else RED, bg, k == "B1")
            continue
        text(g, PX + 16, yy, n, ICE_BLUE, bg)
        text(g, PX + 21, yy, ic, ICE[ic.split()[1]], bg)
        text(g, PX + 30, yy, mk, YOU if mk == "@" else (AMBER if mk == "⌗" else WHITE), bg, True)
    y += 6
    rule(g, y)
    if moment == 1:
        y = side_held(g, y + 1)
        rule(g, y)
        side_body(g, y + 1)
    else:
        text(g, PX, y + 1, "HELD", LABEL, PANEL)
        for i, (gl, name, what) in enumerate([("▯", "server room door", "cracked"),
                                              ("▪", "NOC camera", "looped"),
                                              ("▪", "goods lift", "waiting at B1")]):
            cell(g, PX + 1, y + 2 + i, gl, SIGNAL, PANEL, True)
            text(g, PX + 3, y + 2 + i, name, TEXT, PANEL)
            text(g, PX + 3 + len(name), y + 2 + i, " · " + what, SIGNAL, PANEL)
        y += 6
        rule(g, y)
        text(g, PX, y + 1, "BODY", LABEL, PANEL)
        text(g, PX + 5, y + 1, "B1 · at the NOC terminal", TEXT, PANEL)
        text(g, PX + 3, y + 2, "nobody on B1 · its camera", DIM, PANEL)
        text(g, PX + 3, y + 3, "looped", DIM, PANEL)
        rule(g, y + 4)
        text(g, PX, y + 5, "VAULT", LABEL, PANEL)
        text(g, PX + 6, y + 5, "black ICE on it", RED, PANEL)
        text(g, PX + 3, y + 6, "16 tiles off · hasn't seen you", DIM, PANEL)
    return g


# ------------------------------------------------------------------ the grid strip

def grid_strip():
    g = Grid(120, 16, VOID)
    for y in range(16):
        for x in range(79):
            dot = y % 2 == 0 and x % 4 == 3
            g.c[y][x] = ["·" if dot else " ", DOT, VOID, False]
        g.c[y][79] = ["│", RULE, PANEL, False]
        for x in range(80, 120):
            g.c[y][x] = [" ", TEXT, PANEL, False]
    lane_fg, lane_bg = mix(SIGNAL, WHITE, 0.25), mix(VOID, SIGNAL, 0.2)
    lanes = set()
    for x in range(0, 79):
        lanes.add((x, 8))
    for y in range(16):
        lanes.add((8, y))
        lanes.add((72, y))
    for x in range(42, 43):
        for y in range(0, 2):
            lanes.add((x, y))
    for x in range(42, 73):
        lanes.add((x, 0))
    for (x, y) in lanes:
        cell(g, x, y, DOUBLE[mask(x, y, lambda a, b: (a, b) in lanes)], lane_fg, lane_bg)

    def construct(x0, y0, x1, y1, col, name, sec_walls=DOUBLE):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                edge = x in (x0, x1) or y in (y0, y1)
                inner = x in (x0 + 1, x1 - 1) or y in (y0 + 1, y1 - 1)
                if edge:
                    m = mask(x, y, lambda a, b: (a in (x0, x1) and y0 <= b <= y1) or (b in (y0, y1) and x0 <= a <= x1))
                    cell(g, x, y, sec_walls[m], col, mix(VOID, col, 0.12), True)
                elif inner:
                    m = mask(x, y, lambda a, b: (a in (x0 + 1, x1 - 1) and y0 + 1 <= b <= y1 - 1) or (b in (y0 + 1, y1 - 1) and x0 + 1 <= a <= x1 - 1))
                    cell(g, x, y, LIGHT[m], scale(col, 0.6), mix(VOID, col, 0.18))
                else:
                    cell(g, x, y, " ", col, mix(VOID, col, 0.26))
        cx = (x0 + x1) // 2
        for i, w in enumerate(name):
            text(g, cx - len(w) // 2, (y0 + y1) // 2 - len(name) // 2 + i + (0 if len(name) > 1 else 0), w, col if col != RED else RED, None, True)

    construct(12, 1, 24, 6, OPEN, ["FLATS"], LIGHT)
    construct(12, 10, 26, 14, OPEN, ["24H"], LIGHT)
    construct(56, 10, 68, 14, OPEN, ["STACK 9"], LIGHT)
    construct(32, 2, 52, 14, RED, ["MERIDIAN", "CLEARING"])
    # Its three locks, each on a lane of its own.
    cell(g, 32, 8, "▯", SIGNAL, mix(VOID, SIGNAL, 0.3), True)
    cell(g, 42, 2, "▮", RED, mix(VOID, RED, 0.3), True)
    cell(g, 52, 8, "▮", RED, mix(VOID, RED, 0.3), True)
    # A backdoor in the flats, your cord from it to the broken lock.
    cell(g, 18, 4, "◆", WHITE, mix(VOID, OPEN, 0.26), True)
    cord = [(18, 5), (18, 6), (18, 7)] + [(x, 7) for x in range(18, 32)]
    cs = set(cord) | {(18, 4), (32, 8)}
    for (x, y) in cord:
        if g.c[y][x][0] in "═║╬╠╣╦╩" and (x, y) in lanes:
            continue
        m = mask(x, y, lambda a, b: (a, b) in cs)
        cell(g, x, y, DASH[m], YOU, None, True)
    cell(g, 31, 8, "@", YOU, YOU_GLOW, True)
    text(g, 4, 12, "", DIM, VOID)
    # Panel.
    px = 81
    text(g, px, 1, "MERIDIAN CLEARING", RED, PANEL, True)
    text(g, px + 17, 1, " · office", TEXT, PANEL)
    text(g, px, 2, "SEC ", TEXT, PANEL)
    text(g, px + 4, 2, "190", WHITE, PANEL, True)
    text(g, px + 7, 2, " · ICE white · vault black", TEXT, PANEL)
    text(g, px, 3, "FIREWALL", RED, PANEL)
    cell(g, px + 9, 3, "▯", SIGNAL, PANEL, True)
    text(g, px + 10, 3, "▮▮", RED, PANEL, True)
    text(g, px + 13, 3, "1/3 broken", DIM, PANEL)
    text(g, px, 4, "TRACE", ROOM, PANEL)
    text(g, px + 6, 4, "▰▰▰▰" + "▱" * 6 + " 38", ROOM, PANEL)
    for x in range(80, 120):
        g.c[5][x] = ["─", RULE, PANEL, False]
    text(g, px, 6, "Each lock sits on its own lane, so", DIM, PANEL)
    text(g, px, 7, "you break them from 3 sides:", DIM, PANEL)
    text(g, px, 8, "your rig, a backdoor, a jack box.", DIM, PANEL)
    text(g, px, 9, "Every break adds trace. Break all", DIM, PANEL)
    text(g, px, 10, "3 and you're in, at the riser's", DIM, PANEL)
    text(g, px, 11, "top: the building's server on 7F.", DIM, PANEL)
    for x in range(80, 120):
        g.c[12][x] = ["─", RULE, PANEL, False]
    cell(g, px, 13, "◆", WHITE, PANEL, True)
    text(g, px + 2, 13, "backdoor · Flats 41", TEXT, PANEL)
    text(g, px, 14, "@", YOU, PANEL, True)
    text(g, px + 2, 14, "you, at the west lock", TEXT, PANEL)
    return g


# ------------------------------------------------------------------ the page

def css_classes(grids):
    styles = {}
    for g in grids:
        for row in g.c:
            for ch, fg, bg, bold in row:
                if ch == "▀":
                    k = ("hb", fg, bg)
                elif ch == "▄":
                    k = ("hb", bg, fg)
                elif ch == "█":
                    k = ("hb", fg, fg)
                else:
                    k = ("t", fg, bg, bold)
                styles.setdefault(k, "s%d" % len(styles))
    css = []
    for k, name in styles.items():
        if k[0] == "hb":
            css.append(".%s{background:linear-gradient(%s 50%%,%s 50%%)}" % (name, k[1], k[2]))
        else:
            css.append(".%s{color:%s;background:%s%s}" % (name, k[1], k[2], ";font-weight:700" if k[3] else ""))
    return styles, "\n".join(css)


def render(g, styles):
    out = ['<div class="tm">']
    for row in g.c:
        out.append('<div class="r">')
        for ch, fg, bg, bold in row:
            if ch == "▀":
                out.append('<i class="%s"></i>' % styles[("hb", fg, bg)])
            elif ch == "▄":
                out.append('<i class="%s"></i>' % styles[("hb", bg, fg)])
            elif ch == "█":
                out.append('<i class="%s"></i>' % styles[("hb", fg, fg)])
            else:
                out.append('<i class="%s">%s</i>' % (styles[("t", fg, bg, bold)], html.escape(ch)))
        out.append("</div>")
    out.append("</div>")
    return "".join(out)


if __name__ == "__main__":
    import json
    grids = {"grid": grid_strip(), "d1": look_d(1), "d2": look_d(2), "a": look_a(), "b": look_b(), "c": look_c()}
    styles, css = css_classes(grids.values())
    parts = {k: render(g, styles) for k, g in grids.items()}
    json.dump({"css": css, "parts": parts}, open(OUT, "w"))
    if len(sys.argv) > 3:
        for k, g in grids.items():
            print("=====", k)
            print(g.text())
