"""Net mockups, round 1: the open Net around your block (seed 8, out of the rig), drawn
over the game's own frame at 120x40.

    python3 -I gen.py <shots dir> <out.html>

Run from this folder with `shots` as the shots dir. The shots dir holds net_now.svg, net_all_visible.svg and net_dump.json from the
throwaway tmp_net_look shot test (dump_tests.diff).

now · the Net as it is: the sight radius and the shadow cast, at the landing.
A · signal: the same layer, no light rule; walls weighed and coloured by security,
    constructs named inside, districts tinted, packets on the lanes, your cord home.
B · relief: the void dark away from data; constructs as stepped terraces, one per
    tier of security, lanes as beams with bloom, the hub ringed.
C · wire map: the district as a transit diagram: constructs as stations on wires, the
    hub an interchange, the neighbouring districts at the ends of the lines."""

import heapq
import html
import json
import math
import os
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..")
sys.path.insert(0, REPO + "/docs/superpowers/specs/2026-10-19-gear-inventory-mock")
sys.path.insert(0, REPO + "/docs/superpowers/specs/2026-11-02-minimap-mock")
from cells import Grid  # noqa: E402
import term  # noqa: E402

S = sys.argv[1]
OUT = sys.argv[2]
J = json.load(open(os.path.join(S, "net_dump.json")))
W, H = J["w"], J["h"]
ROWS, DROWS = J["rows"], J["drows"]
NODES = {(n["x"], n["y"]): n for n in J["nodes"]}
BLD = {b["id"]: b for b in J["buildings"]}
DIS = {d["id"]: d for d in J["districts"]}
OWNER = {}
for b in J["buildings"]:
    for y in range(b["y"], b["y"] + b["h"]):
        for x in range(b["x"], b["x"] + b["w"]):
            OWNER[(x, y)] = b["id"]

ACCENT = {"CorpCore": "#00e5ff", "NightMarket": "#ff00c8", "Megablocks": "#ffbe00",
          "CombatZone": "#ff283c", "Docks": "#00c8be", "Industrial": "#e6e600",
          "NeonStrip": "#ff50b4", "OldTown": "#ff963c"}
SIGNAL = "#00ffc8"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
AMBER = "#ffb000"
RED = "#ff2848"
WHITE = "#e6f0f5"
TEXT = "#cdd4de"
DIM = "#8a93a3"
FAINT = "#4a525e"
PANEL = "#0a0b10"
RULE = "#283c4b"

LIGHT = "─│─└││┌├─┘─┴┐┤┬┼"
HEAVY = "━┃━┗┃┃┏┣━┛━┻┓┫┳╋"
DOUBLE = "═║═╚║║╔╠═╝═╩╗╣╦╬"
DASH = LIGHT.replace("─", "┄").replace("│", "┆")
NODE_GLYPH = {"terminal": "◇", "server": "◆", "backbone": "⬡", "data vault": "⌗", "ICE": "◈"}

# The map: cells x 0..78, y 5..34; tile = (OX + x, OY + y).
OX, OY = 150, 46
MX, MY, MW, MH = 0, 5, 79, 30
PORT = (189, 66)
YOU = (201, 62)
JOB = 83  # Kiroflux Mart
HOME = J["home"]


def rgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def hexc(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in t)


def scale(c, f):
    return hexc(tuple(v * f for v in rgb(c)))


def mix(a, b, f):
    a, b = rgb(a), rgb(b)
    return hexc(tuple(a[i] + (b[i] - a[i]) * f for i in range(3)))


def ter(x, y):
    return ROWS[y][x] if 0 <= x < W and 0 <= y < H else " "


def dist_of(x, y):
    c = DROWS[y][x] if 0 <= x < W and 0 <= y < H else "-"
    return None if c == "-" else int(c)


def accent(d):
    return ACCENT.get(DIS[d]["theme"], "#788ca0") if d is not None else "#788ca0"


def tier(sec):
    return 0 if sec < 100 else 1 if sec < 160 else 2 if sec < 230 else 3


TIER_NAME = ["open", "guarded", "ICE", "black ICE"]


def owner_sec(x, y):
    b = OWNER.get((x, y))
    if b is not None and BLD[b]["sec"] >= 0:
        return BLD[b]["sec"]
    return None


def mask(x, y, joins):
    m = 0
    for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
        if joins(x + dx, y + dy):
            m |= bit
    return m


def wall_joins(x, y):
    o = OWNER.get((x, y))
    return lambda a, b: ter(a, b) in "WG" and OWNER.get((a, b)) == o


def lane_joins(a, b):
    return ter(a, b) in "LG"


def walkable(x, y):
    return ter(x, y) in "LG.:"


def cord_path():
    """The way you came from the port: lanes cheap, open grid dear."""
    best = {PORT: 0}
    prev = {}
    q = [(0, PORT)]
    while q:
        c, p = heapq.heappop(q)
        if p == YOU:
            break
        if c > best.get(p, 1e9):
            continue
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (p[0] + dx, p[1] + dy)
            if not walkable(*n):
                continue
            nc = c + (1 if ter(*n) in "LG" else 3)
            if nc < best.get(n, 1e9):
                best[n] = nc
                prev[n] = p
                heapq.heappush(q, (nc, n))
    path, p = [], YOU
    while p in prev:
        path.append(p)
        p = prev[p]
    path.append(PORT)
    return path[::-1]


CORD = cord_path()


def cells():
    for cy in range(MY, MY + MH):
        for cx in range(MX, MX + MW):
            yield cx, cy, OX + cx, OY + cy


def to_cell(x, y):
    return x - OX, y - OY


def in_view(cx, cy):
    return MX <= cx < MX + MW and MY <= cy < MY + MH


def frame(name):
    g = Grid.from_svg(os.path.join(S, name), 120, 40)
    for row in g.c:
        for c in row:
            if c[2] == "#04060a":
                c[2] = PANEL
    return g


def lane_runs():
    """Straight lane runs in view, 8 tiles or longer: (cells in order)."""
    runs = []
    for cy in range(MY, MY + MH):
        run = []
        for cx in range(MX, MX + MW + 1):
            x, y = OX + cx, OY + cy
            if cx < MX + MW and ter(x, y) == "L" and ter(x, y - 1) != "L" and ter(x, y + 1) != "L":
                run.append((cx, cy))
            else:
                if len(run) >= 8:
                    runs.append(run)
                run = []
    for cx in range(MX, MX + MW):
        run = []
        for cy in range(MY, MY + MH + 1):
            x, y = OX + cx, OY + cy
            if cy < MY + MH and ter(x, y) == "L" and ter(x - 1, y) != "L" and ter(x + 1, y) != "L":
                run.append((cx, cy))
            else:
                if len(run) >= 8:
                    runs.append(run)
                run = []
    return runs


def packets(g, glow):
    """A packet on every long straight run: a white head and a fading tail."""
    for i, run in enumerate(lane_runs()):
        if i % 4 == 3:
            continue
        seq = run if i % 2 == 0 else run[::-1]
        at = (i * 7 + 3) % (len(seq) - 3) + 3
        for k, f in ((0, glow), (1, glow * 0.5), (2, glow * 0.22)):
            cx, cy = seq[at - k]
            c = g.c[cy][cx]
            c[2] = mix(c[2], SIGNAL, f)
            if k == 0:
                c[1], c[3] = "#ffffff", True


def cord(g, colour, strength):
    for x, y in CORD[:-1]:
        cx, cy = to_cell(x, y)
        if not in_view(cx, cy):
            continue
        c = g.c[cy][cx]
        c[2] = mix(c[2], colour, strength)
        if ter(x, y) in ".:":
            c[0], c[1] = "·", mix(colour, "#000000", 0.3)
    cx, cy = to_cell(*PORT)
    g.c[cy][cx][0], g.c[cy][cx][1], g.c[cy][cx][3] = "▣", WHITE, True


def you(g):
    cx, cy = to_cell(*YOU)
    g.c[cy][cx][0], g.c[cy][cx][1], g.c[cy][cx][3] = "@", SIGNAL, True


def wrap(name, w, lines):
    words = name.split()
    out, cur = [], ""
    for wd in words:
        if len(wd) > w:
            wd = wd[: w - 1] + "…"
        if not cur:
            cur = wd
        elif len(cur) + 1 + len(wd) <= w:
            cur += " " + wd
        else:
            out.append(cur)
            cur = wd
    if cur:
        out.append(cur)
    if len(out) > lines:
        out = out[:lines]
        if len(out[-1]) < w:
            out[-1] = out[-1][: w - 1] + "…"
    return out


def label_name(b):
    if b["id"] == HOME:
        return "YOUR BLOCK"
    if b["name"] == "apartment":
        return "FLATS"
    return b["name"].upper()


def put_label(g, b, fg, bg=None, inset=1, extra=None, lines_max=3, bold=False):
    """The construct's name in its inside, as near the middle as its nodes allow."""
    x0, y0 = b["x"] + inset, b["y"] + inset
    w, h = b["w"] - 2 * inset, b["h"] - 2 * inset
    if w < 3 or h < 1:
        return
    lines = wrap(label_name(b), w, min(h, lines_max))
    if extra and h >= len(lines) + 2 and all(len(e) <= w for e in extra):
        lines = lines + [""] + extra

    def spans(top):
        for i, line in enumerate(lines):
            x = x0 + (w - len(line)) // 2
            yield top + i, x, line

    def clear(top):
        return all((x + k, y) not in NODES for y, x, line in spans(top) for k in range(len(line)))

    mid = y0 + (h - len(lines)) // 2
    tops = sorted(range(y0, y0 + h - len(lines) + 1), key=lambda t: abs(t - mid))
    top = next((t for t in tops if clear(t)), mid)
    for y, x, line in spans(top):
        for k, ch in enumerate(line):
            cx, cy = to_cell(x + k, y)
            if not in_view(cx, cy) or (x + k, y) in NODES or ch == " ":
                continue
            c = g.c[cy][cx]
            c[0], c[1], c[3] = ch, fg, bold
            if bg:
                c[2] = bg


def node_cell(n, wall_fg):
    k = n["kind"]
    ch = NODE_GLYPH.get(k, "▪")
    if k == "ICE":
        return ch, RED if n["sec"] >= 230 else WHITE
    if k == "backbone":
        return ch, WHITE
    if k in ("server", "terminal", "data vault"):
        return ch, WHITE
    return ch, mix(wall_fg, WHITE, 0.5)


# ---------------------------------------------------------------- sidebar

def sidebar(g, place, minimap):
    """The right column: a construct card in HERE, the job, the Net minimap, the log."""
    g.fill(80, 5, 40, 30, PANEL)
    g.put(81, 2, "grid · unseen".ljust(37), TEXT)
    k = BLD[JOB]
    nodes = [n for n in J["nodes"] if n["b"] == JOB]
    counts = {}
    for n in nodes:
        ch = NODE_GLYPH.get(n["kind"], "▪")
        counts[ch] = counts.get(ch, 0) + 1
    g.put(81, 5, "KIROFLUX MART", AMBER, bold=True)
    g.put(95, 5, "· shop · gate ↑", DIM)
    g.put(81, 6, "SEC", DIM)
    g.put(85, 6, "%d" % k["sec"], WHITE, bold=True)
    g.put(89, 6, "guarded · no ICE at the gate", TEXT)
    x = 81
    g.put(x, 7, "NODES", DIM)
    x += 6
    for ch, n in sorted(counts.items(), key=lambda t: -t[1]):
        s = "%s %d" % (ch, n)
        g.put(x, 7, s, WHITE)
        x += len(s) + 2
    g.put(80, 8, "─" * 40, RULE)
    g.put(81, 9, "JOB", AMBER, bold=True)
    g.put(85, 9, "lift the ledger · Kiroflux Mart", TEXT)
    g.put(80, 10, "─" * 40, RULE)
    minimap(g, 80, 11, 40, 12)
    g.put(81, 23, place, AMBER)
    g.put(80, 24, "─" * 40, RULE)
    g.put(81, 25, "22:00", DIM)
    g.put(87, 25, "jacked in · your rig", TEXT)
    g.put(81, 26, "22:01", DIM)
    g.put(87, 26, "on the wire · Stack 101", TEXT)


def net_minimap(g, x0, y0, w, h):
    """The Net close up: 3x3 tiles a half-block pixel, centred on you."""
    px, py = YOU
    ph = 2 * h

    def colour(i, j):
        tx0 = px + (i - w // 2) * 3
        ty0 = py + (j - ph // 2) * 3
        tiles = [(tx0 + a, ty0 + b) for a in range(3) for b in range(3)]
        if any(t == YOU for t in tiles):
            return WHITE
        if any(OWNER.get(t) == JOB and ter(*t) in "WGF" for t in tiles):
            return AMBER
        if any(t == PORT for t in tiles):
            return "#9fb4c8"
        walls = [owner_sec(*t) for t in tiles if ter(*t) in "WG"]
        walls = [s for s in walls if s is not None]
        if walls:
            return [scale(SIGNAL, 0.4), scale(SIGNAL, 0.62), "#9fd8e8", "#e0283c"][tier(max(walls))]
        if any(ter(*t) == "L" for t in tiles):
            return scale(SIGNAL, 0.85)
        if any(ter(*t) in "WG" for t in tiles):
            return WHITE
        floors = [owner_sec(*t) for t in tiles if ter(*t) == "F"]
        floors = [s for s in floors if s is not None]
        if floors:
            return [scale(SIGNAL, 0.12), scale(SIGNAL, 0.18), "#24404a", "#4a0e18"][tier(max(floors))]
        d = dist_of(min(max(tx0, 0), W - 1), min(max(ty0, 0), H - 1))
        if not (0 <= tx0 < W and 0 <= ty0 < H):
            return "#05070b"
        return mix("#05070b", accent(d), 0.12)

    for j in range(h):
        for i in range(w):
            top, bot = colour(i, 2 * j), colour(i, 2 * j + 1)
            g.c[y0 + j][x0 + i] = ["▀", top, bot, False]


# ---------------------------------------------------------------- A · signal

A_TIER = ["#3fa88f", SIGNAL, "#c8f6ff", RED]
A_SET = [DASH, LIGHT, HEAVY, DOUBLE]


def draw_a():
    g = frame("net_all_visible.svg")
    for cx, cy, x, y in cells():
        t = ter(x, y)
        d = dist_of(x, y)
        acc = accent(d)
        bg = mix("#03060a", acc, 0.045)
        c = g.c[cy][cx]
        if t in ".:":
            c[:] = ["·" if t == ":" else " ", scale(mix(acc, SIGNAL, 0.35), 0.55), bg, False]
        elif t == "L":
            c[:] = [DOUBLE[mask(x, y, lane_joins)], scale(SIGNAL, 0.8), mix(bg, SIGNAL, 0.06), False]
        elif t in "WGF":
            sec = owner_sec(x, y)
            if sec is None:  # a hub
                col, st, fill = WHITE, DOUBLE, mix(bg, WHITE, 0.05)
            else:
                tr = tier(sec)
                col, st, fill = A_TIER[tr], A_SET[tr], mix(bg, A_TIER[tr], 0.10 if tr < 3 else 0.14)
            if OWNER.get((x, y)) == JOB:
                col = AMBER
                fill = mix(bg, AMBER, 0.10)
            if t == "W":
                c[:] = [st[mask(x, y, wall_joins(x, y))], col, bg, False]
            elif t == "G":
                c[:] = ["▯", mix(col, WHITE, 0.6), fill, True]
            else:
                c[:] = [" ", col, fill, False]
        n = NODES.get((x, y))
        if n:
            col = c[1]
            c[0], c[1] = node_cell(n, col)
    for b in BLD.values():
        if b["sec"] < 0:
            continue
        tr = tier(b["sec"])
        fg = scale(A_TIER[tr], 0.62)
        if b["id"] == JOB:
            fg = AMBER
        extra = None
        if b["id"] == HOME:
            extra = ["VELFORM GROUP", "PLAZA"]
        elif b["h"] >= 7:
            extra = ["SEC %d" % b["sec"]]
        put_label(g, b, fg, extra=extra)
    stencils_a(g)
    packets(g, 0.42)
    cord(g, "#ffffff", 0.22)
    you(g)
    sidebar(g, "Stack 101 · Net", net_minimap)
    return g


def stencil_spot(d, text, near):
    """The open run of ground in district `d` nearest `near` that holds `text` with a
    tile clear each side."""
    best = None
    n = len(text) + 2
    for cy in range(MY, MY + MH):
        for cx in range(MX, MX + MW - n + 1):
            x, y = OX + cx, OY + cy
            if all(ter(x + k, y) in ".:" and dist_of(x + k, y) == d for k in range(n)):
                score = abs(x - near[0]) + 2 * abs(y - near[1])
                if best is None or score < best[0]:
                    best = (score, (x + 1, y))
    return best and best[1]


def stencils_a(g):
    """District names written into the grid, faint, in each district's accent."""
    for d, text, near in ((0, "CROWN CIRCLE", (150, 50)), (4, "STACK 101", (215, 79)),
                          (1, "KILN STACKS", (150, 80))):
        at = stencil_spot(d, text, near)
        if not at:
            continue
        x, y = at
        for k, ch in enumerate(text):
            cx, cy = to_cell(x + k, y)
            if ch != " ":
                c = g.c[cy][cx]
                c[0], c[1], c[3] = ch, scale(accent(d), 0.55), True


# ---------------------------------------------------------------- B · relief

def data_field():
    """Tiles from the nearest lane or construct, up to 7."""
    far = 7
    out = {}
    for cy in range(MY - far, MY + MH + far):
        for cx in range(MX - far, MX + MW + far):
            x, y = OX + cx, OY + cy
            if ter(x, y) in "LWGF":
                out[(x, y)] = 0
    frontier = list(out)
    for step in range(1, far + 1):
        nxt = []
        for x, y in frontier:
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (x + dx, y + dy)
                    if n not in out:
                        out[n] = step
                        nxt.append(n)
        frontier = nxt
    return out


B_TIER = ["#3fa88f", SIGNAL, "#9fe6ff", "#ff2848"]


def terraces(b):
    """How many steps a construct rises: one per tier of security, as many as fit."""
    steps = tier(b["sec"]) + 1
    while steps > 0 and (b["w"] - 2 * (steps + 1) < 3 or b["h"] - 2 * (steps + 1) < 1):
        steps -= 1
    return steps


def draw_b(cord_fn=None):
    g = frame("net_all_visible.svg")
    field = data_field()
    void = "#010204"
    hub = None
    for n in J["nodes"]:
        if n["kind"] == "backbone":
            cx, cy = to_cell(n["x"], n["y"])
            if in_view(cx, cy):
                hub = (n["x"], n["y"])
    for cx, cy, x, y in cells():
        t = ter(x, y)
        d = dist_of(x, y)
        acc = accent(d)
        c = g.c[cy][cx]
        if t in ".:":
            f = field.get((x, y), 7)
            bg = mix(void, SIGNAL, 0.09 if f == 1 else 0.03 if f == 2 else 0.0)
            ch = "·" if t == ":" and f <= 2 else " "
            fg = scale(mix(acc, SIGNAL, 0.4), 0.5 if f == 1 else 0.3)
            if hub:
                r = math.hypot(x - hub[0], (y - hub[1]) * 2)
                for R, f2 in ((5.0, 0.8), (9.0, 0.45), (13.0, 0.22)):
                    if abs(r - R) < 0.5:
                        ch, fg = "·", scale(WHITE, f2)
            c[:] = [ch, fg, bg, False]
        elif t == "L":
            c[:] = [DOUBLE[mask(x, y, lane_joins)], mix(SIGNAL, WHITE, 0.25), mix(void, SIGNAL, 0.22), False]
        elif t in "WGF":
            sec = owner_sec(x, y)
            b = OWNER.get((x, y))
            if sec is None:
                c[:] = [DOUBLE[mask(x, y, wall_joins(x, y))] if t == "W" else ("▯" if t == "G" else " "),
                        WHITE, mix(void, WHITE, 0.12), t == "G"]
            else:
                tr = tier(sec)
                col = AMBER if b == JOB else B_TIER[tr]
                if t == "W":
                    c[:] = [A_SET[tr][mask(x, y, wall_joins(x, y))], scale(col, 0.75), mix(void, col, 0.06), False]
                elif t == "G":
                    c[:] = ["▯", WHITE, mix(void, col, 0.3), True]
                else:
                    c[:] = [" ", col, mix(void, col, 0.05), False]
        n = NODES.get((x, y))
        if n:
            c[0], c[1] = node_cell(n, c[1])
    # The steps: a ring per tier, each brighter and lit higher than the one below.
    for b in BLD.values():
        if b["sec"] < 0:
            continue
        tr = tier(b["sec"])
        col = AMBER if b["id"] == JOB else B_TIER[tr]
        steps = terraces(b)
        for k in range(1, steps + 1):
            x0, y0, w, h = b["x"] + k, b["y"] + k, b["w"] - 2 * k, b["h"] - 2 * k
            ring = set()
            for x in range(x0, x0 + w):
                ring.add((x, y0))
                ring.add((x, y0 + h - 1))
            for y in range(y0, y0 + h):
                ring.add((x0, y))
                ring.add((x0 + w - 1, y))
            lift = k / (steps + 1)
            fg = mix(scale(col, 0.45), col, lift)
            fill = mix(void, col, 0.05 + 0.13 * k)
            for y in range(y0, y0 + h):
                for x in range(x0, x0 + w):
                    cx, cy = to_cell(x, y)
                    if not in_view(cx, cy) or ter(x, y) != "F":
                        continue
                    c = g.c[cy][cx]
                    c[2] = fill
                    if (x, y) in ring and (x, y) not in NODES:
                        m = mask(x, y, lambda a, bb: (a, bb) in ring)
                        c[0], c[1] = LIGHT[m], fg
        top = steps + 1
        plate = mix(void, col, 0.05 + 0.13 * (steps + 1) + 0.08)
        for y in range(b["y"] + top, b["y"] + b["h"] - top):
            for x in range(b["x"] + top, b["x"] + b["w"] - top):
                cx, cy = to_cell(x, y)
                if in_view(cx, cy) and ter(x, y) == "F":
                    g.c[cy][cx][2] = plate
        put_label(g, b, mix(col, WHITE, 0.8), inset=top, lines_max=2, bold=True)
    packets(g, 0.6)
    if cord_fn:
        cord_fn(g)
    else:
        cord(g, "#ffffff", 0.16)
    you(g)
    sidebar(g, "Stack 101 · Net", net_minimap)
    return g


# ---------------------------------------------------------------- C · wire map

def district_links():
    """Districts that touch on the Net."""
    pairs = set()
    for y in range(H):
        for x in range(W):
            a = dist_of(x, y)
            for b in (dist_of(x + 1, y) if x + 1 < W else None, dist_of(x, y + 1) if y + 1 < H else None):
                if a is not None and b is not None and a != b:
                    pairs.add((min(a, b), max(a, b)))
    return pairs


def city_minimap(g, x0, y0, w, h):
    """The city's districts as a wire map: hubs where their backbones are, a line for
    each pair that touches, yours white."""
    ph = 2 * h
    px = [["#05070b"] * w for _ in range(ph)]
    hubs = {}
    for n in J["nodes"]:
        if n["kind"] == "backbone":
            d = dist_of(n["x"], n["y"])
            hubs[d] = (min(w - 1, round(n["x"] * (w - 1) / (W - 1))), min(ph - 1, round(n["y"] * (ph - 1) / (H - 1))))
    for a, b in district_links():
        if a not in hubs or b not in hubs:
            continue
        (ax, ay), (bx, by) = hubs[a], hubs[b]
        steps = max(abs(bx - ax), abs(by - ay))
        for i in range(steps + 1):
            x = round(ax + (bx - ax) * i / max(steps, 1))
            y = round(ay + (by - ay) * i / max(steps, 1))
            px[y][x] = mix("#05070b", SIGNAL, 0.35)
    for d, (x, y) in hubs.items():
        col = WHITE if d == 4 else accent(d)
        for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1)):
            if 0 <= x + dx < w and 0 <= y + dy < ph:
                px[y + dy][x + dx] = col
    for j in range(h):
        for i in range(w):
            g.c[y0 + j][x0 + i] = ["▀", px[2 * j][i], px[2 * j + 1][i], False]


def draw_c():
    g = frame("net_all_visible.svg")
    bg = "#03060a"
    g.fill(MX, MY, MW, MH, bg)
    for cy in range(MY, MY + MH):
        for cx in range(MX, MX + MW):
            if cx % 4 == 2 and cy % 2 == 1:
                g.c[cy][cx][0], g.c[cy][cx][1] = "·", "#0d2420"

    def at(x, y, ch, col, bold=False):
        g.c[y][x][0], g.c[y][x][1], g.c[y][x][3] = ch, col, bold

    wire = scale(SIGNAL, 0.7)
    L, R, T, B = 14, 64, 9, 29
    # The loop: Stack 101's own wire round its hub.
    for x in range(L, R + 1):
        at(x, T, "─", wire)
        at(x, B, "─", wire)
    for y in range(T, B + 1):
        at(L, y, "│", wire)
        at(R, y, "│", wire)
    at(L, T, "╭", wire)
    at(R, T, "╮", wire)
    at(L, B, "╰", wire)
    at(R, B, "╯", wire)
    # Trunks out to the neighbours, each in its district's colour.
    west, east = ACCENT["CorpCore"], "#ff4050"
    north, south = ACCENT["NightMarket"], ACCENT["Industrial"]
    for x in range(0, 32):
        at(x, 19, "━", west)
    for x in range(47, 79):
        at(x, 19, "━", east)
    for y in range(6, 18):
        at(39, y, "┃", north)
    for y in range(21, 34):
        at(39, y, "┃", south)
    at(L, 19, "┿", west)
    at(R, 19, "┿", east)
    at(39, T, "╂", north)
    at(39, B, "╂", south)
    # The hub: an interchange.
    for yy in range(18, 21):
        for xx in range(32, 47):
            g.c[yy][xx][2] = mix(bg, WHITE, 0.07)
    g.put(32, 18, "╔══════╩══════╗", WHITE)
    g.put(32, 19, "╣ ⬡ STACK 101 ╠", WHITE, bold=True)
    g.put(32, 20, "╚══════╦══════╝", WHITE)
    g.put(41, 21, "hub · 117", DIM)

    def station(x, y, glyph, col):
        at(x, y, glyph, col, True)

    def label(x, y, name, kind, col=TEXT, right=False, bold=False, kcol=DIM):
        if right:
            g.put(x - len(name) + 1, y, name, col, bold=bold)
            g.put(x - len(kind) + 1, y + 1, kind, kcol)
        else:
            g.put(x, y, name, col, bold=bold)
            g.put(x, y + 1, kind, kcol)

    # Top of the loop.
    station(18, T, "○", wire)
    g.put(15, T - 1, "flats ×4", DIM)
    station(28, T, "●", SIGNAL)
    label(25, T + 1, "24h Lumform", "shop · 114")
    station(46, T, "◆", AMBER)
    label(42, T + 1, "KIROFLUX MART", "shop · 114 · JOB", AMBER, bold=True, kcol=AMBER)
    # Right side.
    station(R, 12, "●", SIGNAL)
    label(R - 2, 12, "Velmed Supply", "shop · 114", right=True)
    station(R, 15, "●", SIGNAL)
    label(R - 2, 15, "Arasudyne Mart", "shop · 114", right=True)
    station(R, 23, "●", SIGNAL)
    label(R - 2, 23, "Stack 101 Terminal", "flats ×5 · 114", right=True)
    # Bottom.
    station(22, B, "●", SIGNAL)
    label(17, B - 2, "Yukiware Supply", "shop · 114")
    station(48, B, "●", SIGNAL)
    label(44, B - 2, "Novflux Goods", "shop · 114")
    station(58, B, "○", wire)
    g.put(55, B + 1, "flats ×2", DIM)
    # Left: your block, with your rig.
    station(L, 13, "▣", "#7cff9a")
    label(L + 2, 13, "YOUR BLOCK", "Velform Group Plaza", "#7cff9a", bold=True)
    g.put(L + 2, 15, "your rig · the port", DIM)
    station(L, 24, "○", wire)
    g.put(L + 2, 24, "flats ×6", DIM)
    # The line ends.
    g.put(0, 18, "◂ CROWN CIRCLE", west, bold=True)
    g.put(0, 20, "black ICE 255", RED)
    g.put(65, 18, "HOLLOW FLATS ▸", east, bold=True)
    g.put(70, 20, "open · 6", DIM)
    g.put(41, 6, "▴ KOWLOON ROW · open · 75", north, bold=True)
    g.put(41, 34, "▾ KILN STACKS · open · 88", south, bold=True)
    # You, riding the top of the loop east, toward the job.
    at(36, T, "@", SIGNAL, True)
    for k, f in ((1, 0.85), (2, 0.55), (3, 0.3)):
        at(36 - k, T, "━", scale(WHITE, f))
    g.put(31, T - 1, "› 2 beats", WHITE)
    sidebar(g, "the city · wire map", city_minimap)
    return g


# ---------------------------------------------------------------- page

def figure(g, letter, title, lines, choice):
    rows = "".join("<li>%s</li>" % html.escape(s) for s in lines)
    return ('<div class="opt" data-choice="%s" onclick="toggleSelect(this)">'
            '<div class="hd"><span class="lt">%s</span> %s</div>%s<ul>%s</ul></div>'
            % (choice, letter, html.escape(title), term.render(shade(g), 10.5), rows))


IDEAS = [
    ("see", "See it all", "No sight radius, no shadow cast and no fading to memory anywhere on the Net. "
     "What you can't see is what you haven't scanned, not what's round a corner."),
    ("ride", "Ride the lanes", "On a lane, Shift+direction runs you to the next junction or gate in one go. "
     "The open grid stays a walk; lanes are the fast way across."),
    ("card", "Construct card", "HERE reads the construct ahead: name, kind, security, ICE at the gate, its "
     "nodes, alarm, your backdoor. The job's construct is marked on the map."),
    ("cord", "The cord", "A thin line from you back to your port. When a trace runs it crawls back along "
     "the cord toward your body, so you can see how close it is."),
    ("mini", "A minimap of the Net", "The sidebar's minimap shows the Net (constructs by security, lanes, "
     "hubs) instead of the street under it."),
    ("traffic", "Traffic", "Packets run the lanes. Later: couriers you can tap for paydata, sysops "
     "patrolling the corp sectors, other runners."),
    ("scan", "Scan, don't see", "A construct's ICE and nodes read ? until you scan it from its gate "
     "(Ping already does this inside)."),
    ("inside", "No light rule inside either", "Inside a construct you see every room and every piece "
     "of ICE from the gate, so you plan the run against the patrols you can watch."),
]


def page():
    now = frame("net_now.svg")
    css = term.CSS + """
.opt{border:1px solid #1d2733;border-radius:8px;padding:12px;margin:0 0 22px;cursor:pointer;background:#0b0f15}
.opt.selected{border-color:#00ffc8;box-shadow:0 0 0 1px #00ffc8}
.opt .hd{font:600 16px system-ui;margin:0 0 8px;color:#e6edf3}
.opt .lt{display:inline-block;background:#00ffc8;color:#02110c;border-radius:4px;padding:0 7px;margin-right:6px}
.opt ul{margin:10px 0 0 18px;padding:0;color:#b8c2cc;font:13.5px/1.5 system-ui}
.ideas{display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:10px}
.idea{border:1px solid #1d2733;border-radius:8px;padding:10px 12px;cursor:pointer;background:#0b0f15}
.idea.selected{border-color:#00ffc8;box-shadow:0 0 0 1px #00ffc8}
.idea h4{margin:0 0 4px;font:600 14px system-ui;color:#e6edf3}
.idea p{margin:0;font:13px/1.45 system-ui;color:#aab4be}
.now{opacity:.95}
"""
    a = figure(draw_a(), "A", "Signal · the same layer, sharpened", [
        "No light rule: the whole view drawn at full, nothing dimmed to memory.",
        "Wall weight and colour say security: dashed = open, single = guarded, heavy = ICE, red double = black ICE.",
        "Constructs named inside; the ground tinted by district, names stencilled where districts meet.",
        "Packets run the lanes; the cord runs back to your port (▣); the job's construct in amber.",
    ], "a")
    b = figure(draw_b(), "B", "Relief · data glows, the void falls away", [
        "No light rule for you; the Net lights itself: the grid shows only near lanes and constructs.",
        "Constructs as stepped terraces seen from above: one step per tier of security, brightest at the top.",
        "Lanes are beams with bloom; packets and the cord as in A. Black-ICE towers glow red from inside.",
    ], "b")
    c = figure(draw_c(), "C", "Wire map · the district as a transit diagram", [
        "Geography goes; the Net is what's linked to what. Constructs are stations, the hub an interchange.",
        "You ride the wires station to station (a few beats a hop); neighbouring districts sit at the line ends.",
        "Blocks of flats fold into one stop. Inside a station is still the construct you know.",
    ], "c")
    ideas = "".join('<div class="idea" data-choice="%s" onclick="toggleSelect(this)"><h4>%s</h4><p>%s</p></div>'
                    % (k, html.escape(t), html.escape(d)) for k, t, d in IDEAS)
    return ("<style>%s</style>" % css
            + "<h2>The Net · round 1</h2>"
            + '<p class="subtitle">Out of your rig in Stack 101, three blocks along the lane, a job on Kiroflux '
              "Mart. Same frame and data as the game (seed 8), 120×40. Click a look; tick the ideas you want.</p>"
            + '<div class="opt now"><div class="hd">Now · light rule on (radius and shadow cast)</div>'
            + term.render(shade(now), 10.5) + "</div>"
            + '<div class="options">' + a + b + c + "</div>"
            + "<h3>What the Net could do (any look)</h3>"
            + '<div class="ideas" data-multiselect>' + ideas + "</div>")


def shade(g):
    """The window draws light shade as a block; here, a mixed background."""
    for row in g.c:
        for c in row:
            if c[0] == "░":
                c[0], c[2] = " ", mix(c[2], c[1], 0.22)
    return g


if __name__ == "__main__" and os.environ.get("FRAMES"):
    for k, f in (("now", lambda: frame("net_now.svg")), ("a", draw_a), ("b", draw_b), ("c", draw_c)):
        open(os.path.join(os.path.dirname(OUT), "frame_%s.html" % k), "w").write(
            "<style>%s</style>%s" % (term.CSS, term.render(shade(f()), 14)))

if __name__ == "__main__":
    open(OUT, "w").write(page())
    print("ok", OUT)
