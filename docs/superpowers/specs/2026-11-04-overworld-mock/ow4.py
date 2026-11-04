"""THROWAWAY preview (tiles spike): generated worlds drawn one level down by the rules
in 2026-11-04-world-tiles-design.md and -cities-in-the-world-design.md. Real worlds,
dumped into dumps/ by `tmp_dump_world.rs` here (a throwaway example: copy it to crates/citygen/examples/ and run `cargo run -p citygen --example tmp_dump_world -- <seed>`).

    python3 -I ow4.py <out.html>

One overworld tile is 8x8 detail tiles. Everything shared across a tile edge (road and
river crossings, noise) comes from world-wide detail coordinates and the seed, so the
tiles join with no seam. Not the real generator: a rough idea of what it will draw."""

import html
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "round4.html")
sys.argv = [sys.argv[0], os.devnull]
sys.path.insert(0, HERE)
import ow2  # noqa: E402
from ow import fbm, h32, mix, scale, term  # noqa: E402

K = 8
VW, VH = 79, 30
BG = "#04060a"


class World:
    def __init__(self, path):
        self.t, self.cities, self.routes, self.rivers, self.pois, self.regions = {}, [], [], [], [], []
        for line in open(path):
            a = line.rstrip("\n").split(" ")
            if a[0] == "world":
                self.seed, self.w, self.h = int(a[1]), int(a[2]), int(a[3])
            elif a[0] == "t":
                self.t[(int(a[1]), int(a[2]))] = dict(
                    water=a[3], ground=a[4], elev=int(a[5]) / 255, road=int(a[6]),
                    river=int(a[7]), city=int(a[8]), density=int(a[9]) / 255, theme=a[10])
            elif a[0] == "c":
                self.cities.append(dict(x=int(a[1]), y=int(a[2]), size=float(a[3]),
                                        home=a[4] == "true", name=" ".join(a[5:])))
            elif a[0] == "p":
                self.pois.append(dict(x=int(a[1]), y=int(a[2]), kind=a[3], name=" ".join(a[4:])))
            elif a[0] == "r":
                name, path = line[2:].rstrip("\n").split("|")
                self.routes.append((name, [tuple(map(int, q.split(","))) for q in path.split()]))
            elif a[0] == "g":
                self.regions.append(dict(x=int(a[1]), y=int(a[2]), kind=a[3], name=" ".join(a[4:])))

    def tile(self, tx, ty):
        return self.t.get((tx, ty))


# ---- the detail world, from the plan

def bilinear(w, x, y, f):
    """`f(tile)` blended between tile centres at detail point (x, y)."""
    fx, fy = x / K - 0.5, y / K - 0.5
    x0, y0 = math.floor(fx), math.floor(fy)
    ux, uy = fx - x0, fy - y0
    v = 0.0
    for dx, dy, wt in ((0, 0, (1 - ux) * (1 - uy)), (1, 0, ux * (1 - uy)),
                       (0, 1, (1 - ux) * uy), (1, 1, ux * uy)):
        t = w.tile(x0 + dx, y0 + dy)
        v += wt * (f(t) if t else 0.0)
    return v


def crossing(w, tx, ty, side):
    """Where a link out of tile (tx, ty) on `side` crosses the shared edge: the same
    number from both tiles (the edge's own hash), in the edge's middle half."""
    if side == "W":
        return crossing(w, tx - 1, ty, "E")
    if side == "N":
        return crossing(w, tx, ty - 1, "S")
    o = 3 + int(h32(w.seed, tx, ty, 1 if side == "E" else 2) * 2) % 2
    return (K * tx + K, K * ty + o) if side == "E" else (K * tx + o, K * ty + K)


def straight(a, b):
    """Four-connected cells from a to b, stepping along the longer axis."""
    (x, y), out = a, [a]
    while (x, y) != b:
        dx, dy = b[0] - x, b[1] - y
        if abs(dx) * 2 >= abs(dy) and dx:
            x += 1 if dx > 0 else -1
        else:
            y += 1 if dy > 0 else -1
        out.append((x, y))
    return out


def centreline(w, tx, ty, mask, salt):
    """The cells a road (or river) runs on through tile (tx, ty): straight from crossing
    to crossing when it only passes through, else from each crossing to a point near the
    centre, four-connected."""
    def inside(side):
        ex, ey = crossing(w, tx, ty, side)
        return (ex - 1, ey) if side == "E" else (ex, ey - 1) if side == "S" else (ex, ey)
    if mask in (2 | 8, 1 | 4):
        a, b = (inside("W"), inside("E")) if mask == 10 else (inside("N"), inside("S"))
        return straight(a, b)
    cx = K * tx + 3 + int(h32(w.seed, tx, ty, salt) * 2)
    cy = K * ty + 3 + int(h32(w.seed, tx, ty, salt + 1) * 2)
    out = []
    for bit, side in ((1, "N"), (2, "E"), (4, "S"), (8, "W")):
        if not mask & bit:
            continue
        ex, ey = crossing(w, tx, ty, side)
        # step into the tile from the edge point
        if side == "E":
            ex -= 1
        if side == "S":
            ey -= 1
        x, y = ex, ey
        cells = [(x, y)]
        horizontal = side in ("E", "W")
        while (x, y) != (cx, cy):
            if horizontal and x != cx or not horizontal and y == cy:
                x += 1 if cx > x else -1
            else:
                y += 1 if cy > y else -1
            cells.append((x, y))
        out += cells
    return out


def build(w, X0, Y0):
    """The ground of the view at detail (X0, Y0): a Ground of terrains and features."""
    g = ow2.Ground(VW, VH, salt=w.seed)
    road, river = set(), set()
    txs = range((X0 // K) - 1, (X0 + VW) // K + 2)
    tys = range((Y0 // K) - 1, (Y0 + VH) // K + 2)
    for ty in tys:
        for tx in txs:
            t = w.tile(tx, ty)
            if not t:
                continue
            if t["river"]:
                for (x, y) in centreline(w, tx, ty, t["river"], 7):
                    river.add((x, y))
                    river.add((x + 1, y))
            if t["road"]:
                for (x, y) in centreline(w, tx, ty, t["road"], 3):
                    road.update({(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)})
    wob = fbm  # world-wide noise: (x, y) are detail coordinates
    for vy in range(VH):
        for vx in range(VW):
            x, y = X0 + vx, Y0 + vy
            sea = bilinear(w, x, y, lambda t: t["water"] == "Sea") + 0.45 * (wob(x / 7, y / 3.5, 11, 3) - 0.5)
            lake = bilinear(w, x, y, lambda t: t["water"] == "Lake") + 0.45 * (wob(x / 5, y / 2.5, 12, 3) - 0.5)
            jx = x + int(6 * (wob(x / 9, y / 4.5, 13, 2) - 0.5))
            jy = y + int(3 * (wob(x / 9, y / 4.5, 14, 2) - 0.5))
            t = w.tile(jx // K, jy // K) or w.tile(x // K, y // K)
            ground = t["ground"] if t else "Flats"
            terr = {"Flats": "dirt", "Scrub": "scrub", "Fields": "crop", "Salt": "salt",
                    "Spill": "spill", "Hills": "dirt", "Mountains": "rock", "Peak": "rock"}[ground]
            if ground == "Hills" and h32(x, y, 21) < 0.12:
                terr = "rock"
            if ground == "Mountains" and wob(x / 4, y / 2, 15, 3) < 0.45:
                terr = "dirt"
            if sea > 0.5:
                terr = "sea"
            elif lake > 0.5:
                terr = "lake"
            if (x, y) in river and terr not in ("sea",):
                terr = "river"
            g.set(vx, vy, terr)
    # cities: the world's density, smoothed across tiles
    for vy in range(VH):
        for vx in range(VW):
            x, y = X0 + vx, Y0 + vy
            if g.t[vy][vx] in ("sea", "lake", "river"):
                continue
            d = bilinear(w, x, y, lambda t: t["density"] if t["city"] >= 0 else 0.0)
            d += 0.18 * (wob(x / 6, y / 3, 16, 3) - 0.5)
            th = (w.tile(x // K, y // K) or {}).get("theme", "-")
            kind = "mega" if th in ("CorpCore", "Megablocks") else "low"
            bx, by = x % 14, y % 7
            street = bx in (0, 1) or by == 0
            if d > 0.42:
                if street:
                    g.set(vx, vy, "road" if bx == 0 or by == 0 else "side")
                else:
                    g.set(vx, vy, "mass", kind)
                    edge = bx in (2, 13) or by in (1, 6)
                    if edge:
                        g.set(vx, vy, "wall", kind)
                if by == 1 and bx == 2 and h32(x // 14, y // 7, 31) < 0.6:
                    g.feat(vx, vy, "lamp")
            elif d > 0.16:
                cell = h32(x // 14, y // 7, 32)
                if street:
                    g.set(vx, vy, "track")
                elif cell < 0.3:
                    g.set(vx, vy, "lot")
                elif cell < 0.6:
                    inner = (x % 14) in range(4, 9) and (y % 7) in range(2, 5)
                    inner2 = (x % 14) in range(10, 13) and (y % 7) in range(3, 6) and cell < 0.5
                    if inner or inner2:
                        g.set(vx, vy, "mass", "shanty")
                        if (x % 14) in (4, 8, 10, 12) or (y % 7) in (2, 4, 3, 5):
                            g.set(vx, vy, "wall", "shanty")
                    elif h32(x, y, 33) < 0.06:
                        g.feat(vx, vy, "trash")
                elif cell < 0.8:
                    if bx in (2, 13) or by in (1, 6):
                        g.set(vx, vy, "fence")
                    elif h32(x, y, 34) < 0.25:
                        g.feat(vx, vy, "scrap")
                if by == 0 and bx == 2 and h32(x // 14, y // 7, 35) < 0.3:
                    g.feat(vx, vy, "lamp")
    # highways, over everything but the sea; bridges over water
    for (x, y) in road:
        vx, vy = x - X0, y - Y0
        if not g.inb(vx, vy):
            continue
        under = g.t[vy][vx]
        g.set(vx, vy, "road")
        if under in ("river", "lake", "sea"):
            g.k[vy][vx] = "bridge"
    for (x, y) in road:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            vx, vy = x - X0 + dx, y - Y0 + dy
            if g.inb(vx, vy) and (x + dx, y + dy) not in road:
                if g.t[vy][vx] in ("river", "lake", "sea"):
                    g.set(vx, vy, "rail")
                elif g.t[vy][vx] in ("dirt", "scrub", "crop", "salt", "rock", "track"):
                    g.set(vx, vy, "gravel")
    # traffic now and then
    for (x, y) in sorted(road):
        vx, vy = x - X0, y - Y0
        if g.inb(vx, vy) and h32(x // 3, y, 41) < 0.025 and g.t[vy][vx] == "road":
            col = ["#8c4d00", "#1e5aa0", "#8c006e", "#5e6470"][int(h32(x, y, 42) * 4)]
            for k in range(3):
                if g.inb(vx + k, vy) and g.t[vy][vx + k] == "road":
                    g.feat(vx + k, vy, ("▬", col))
    return g, road


def poi_layout(w, g, road, X0, Y0, poi):
    """The POI's place one level down, beside its road."""
    cx, cy = K * poi["x"] + 4, K * poi["y"] + 4
    near = min(road, key=lambda p: (p[0] - cx) ** 2 + (2 * (p[1] - cy)) ** 2, default=None)
    if not near:
        return []
    rx, ry = near
    horizontal = (rx + 2, ry) in road or (rx - 2, ry) in road
    if poi["kind"] == "TruckStop":
        side = 1 if cy >= ry else -1
        # u runs along the road, v away from it
        def at(u, v):
            if horizontal:
                return rx + u - X0, (ry + (2 if side > 0 else -1)) + side * v - Y0
            return (rx + (2 if side > 0 else -1)) + side * v - X0, ry + u - Y0
        for u in range(-14, 16):
            for v in range(1, 5):
                if g.inb(*at(u, v)):
                    g.set(*at(u, v), "plaza")
        for u in range(-8, 8):
            if g.inb(*at(u, 4)):
                g.feat(*at(u, 4), "awning")
        for u in (-6, -2, 2, 6):
            for v in (2, 3):
                if g.inb(*at(u, v)):
                    g.feat(*at(u, v), "pump")
        x0, y0 = at(-4, 5)
        x1, y1 = at(10, 10)
        bx, by = min(x0, x1), min(y0, y1)
        g.building(bx, by, abs(x1 - x0) + 1, abs(y1 - y0) + 1, "stop")
        dx, dy = at(3, 5)
        if g.inb(dx, dy):
            g.set(dx, dy, "door", "stop")
        lx0, ly0 = at(-15, 5)
        lx1, ly1 = at(-6, 12)
        g.fence(min(lx0, lx1), min(ly0, ly1), max(lx0, lx1), max(ly0, ly1))
        for v, col in ((7, "#5e6470"), (9, "#7a4a2a"), (11, "#3a5a7a")):
            for u in range(-13, -7):
                p = at(u, v)
                if g.inb(*p):
                    g.set(*p, "lot")
                    g.feat(*p, ("▬", col))
        for u, v in ((-14, 1), (-4, 1), (6, 1), (15, 1), (-10, 6)):
            p = at(u, v)
            if g.inb(*p):
                g.feat(*p, "lamp")
        p = at(9, 2)
        if g.inb(*p):
            g.feat(*p, "camera")
        sx, sy = at(17, 1)
        for i, ch in enumerate("▣ｼﾞｭﾘ9▣"):
            if g.inb(sx + i, sy):
                g.feat(sx + i, sy, (ch, "#ff8c00" if ch != "▣" else "#3c8cff"))
        return [(bx, by, abs(x1 - x0) + 1, abs(y1 - y0) + 1, poi["name"].split()[0].upper())]
    return []


def frame(w, X0, Y0, title, sub, here, pois=(), net=False):
    g, road = build(w, X0, Y0)
    constructs = []
    for poi in pois:
        constructs += poi_layout(w, g, road, X0, Y0, poi)
    cells = ow2.paint(g, "T", ambient=0.8)
    for (vx, vy), c in cells.items():
        t = g.t[vy][vx]
        x, y = X0 + vx, Y0 + vy
        r = h32(x, y, 51)
        if t in ("sea", "lake", "river"):
            ch = "≈~∼"[int((r * 3)) % 3] if r < 0.5 or t == "river" else " "
            fg = "#3a6ea8" if t != "sea" else "#2e5c8e"
            bg = "#081628" if t != "river" else "#0a1c34"
            cells[(vx, vy)] = [ch, fg, bg, False]
        elif t == "rock" and (w.tile((X0 + vx) // K, (Y0 + vy) // K) or {}).get("ground") in ("Mountains", "Peak"):
            cells[(vx, vy)] = ["▓" if r < 0.6 else "█", "#34312c", "#0c0b0a", False]
        elif t == "rail":
            cells[(vx, vy)] = ["─", "#6a6e76", "#081628", False]
        elif t == "crop":
            cells[(vx, vy)] = ["‗" if False else ("▬" if (x + 2 * y) % 3 else " "), "#3a6a30", "#0b0c08", False]
        elif t == "salt":
            cells[(vx, vy)] = ["░" if r < 0.5 else "·", "#5a5852", "#121210", False]
        elif t == "spill":
            cells[(vx, vy)] = ["≈" if r < 0.2 else ("·" if r < 0.5 else " "),
                               "#4a8a22" if r < 0.9 else "#9aff5a", "#060c04", False]
        elif t == "road" and g.k[vy][vx] == "bridge":
            c[2] = mix(c[2], "#1a2a3a", 0.4)
    f = ow2.apply(ow2.NETF if net else ow2.STREET, ow2.paint_net(g, road, constructs, X0, Y0) if net else cells)
    # the walker
    open_ = ("plaza", "gravel", "side", "track", "road")
    yx, yy = min(((x, y) for y in range(VH) for x in range(VW) if g.t[y][x] in open_),
                 key=lambda p: (p[0] - VW // 2) ** 2 + (2 * (p[1] - VH // 2)) ** 2,
                 default=(VW // 2, VH // 2))
    f.c[5 + yy][yx] = ["@", "#ffffff", f.c[5 + yy][yx][2], True]
    ow2.panel(f, title, sub, here)
    overworld_pane(f, w, X0, Y0, net)
    f.fill(0, 36, 79, 1, BG)
    f.put(1, 36, ("⚡ " if net else "on foot · ") + title.split(" · ")[0], "#c8d0dc")
    return f


def atlas(w, tx, ty, net=False):
    t = w.tile(tx, ty)
    if not t:
        return [" ", BG, BG, False]
    poi = next((p for p in w.pois if p["x"] == tx and p["y"] == ty), None)
    if poi and not net:
        g = {"TruckStop": ("⌂", "#ffb000"), "Checkpoint": ("╪", "#ff2848"), "Relay": ("⬡", "#00ffc8"),
             "Camp": ("△", "#ff8040"), "Wreck": ("×", "#ff2848"), "Arcology": ("▣", "#a890d8"),
             "Solar": ("▦", "#7ab0e0"), "WaterPlant": ("◎", "#00e5ff"), "TestRange": ("◈", "#ff00c8"),
             "Rig": ("◆", "#c8a050")}[poi["kind"]]
        return [g[0], g[1], BG, True]
    dbl = "═║═╚║║╔╠═╝═╩╗╣╦╬"
    rnd = "─│─╰││╭├─╯─┴╮┤┬┼"
    if t["road"]:
        return [dbl[t["road"] & 15], "#3afbd3" if net else "#c8a050", "#013a2f" if net else BG, False]
    if t["water"] == "River":
        return [rnd[t["river"] & 15], "#3a8ad8", "#06121f", False]
    if t["city"] >= 0:
        v = t["density"]
        return ["█" if v > 0.75 else "▓" if v > 0.45 else "▒" if v > 0.22 else "░",
                mix("#2a2e38", "#9aa0ae", v), BG, False]
    if t["water"] in ("Sea", "Lake"):
        return ["~", "#2a5a8a", "#0a1a2e", False]
    return {"Peak": ["▲", "#c8ccd8", BG, False], "Mountains": ["▲", "#787c8a", BG, False],
            "Hills": ["∩", "#7a6648", BG, False], "Spill": ["·", "#4a8a22", "#060c04", False],
            "Salt": ["░", "#5a5852", BG, False], "Fields": ["▬", "#3a6a30", BG, False],
            "Scrub": ['"', "#4a5a34", BG, False]}.get(t["ground"], ["·" if h32(tx, ty, 9) < 0.3 else " ", "#4a4234", BG, False])


def overworld_pane(f, w, X0, Y0, net):
    """The panel's minimap: the overworld round the view, the view's tiles framed."""
    f.fill(80, 9, 40, 14, BG)
    ctx, cty = (X0 + VW // 2) // K, (Y0 + VH // 2) // K
    for r in range(13):
        for c in range(40):
            f.c[9 + r][80 + c] = list(atlas(w, ctx - 20 + c, cty - 6 + r, net))
    x0 = 80 + X0 // K - (ctx - 20)
    x1 = 80 + (X0 + VW) // K - (ctx - 20)
    y0 = 9 + Y0 // K - (cty - 6)
    y1 = 9 + (Y0 + VH) // K - (cty - 6)
    for (x, y), ch in (((x0 - 1, y0), "┌"), ((x1, y0), "┐"), ((x0 - 1, y1), "└"), ((x1, y1), "┘")):
        if 80 <= x < 120 and 9 <= y < 22:
            f.c[y][x][0], f.c[y][x][1] = ch, "#ffffff"
    f.put(81, 22, "seed %d · tile %d,%d" % (w.seed, ctx, cty), "#d9a200")
    f.put(108, 22, "overworld", "#4a525e")


# ---- the Net over the same ground

LANE = ("#3afbd3", "#013a2f")


def paint_net(g, road, constructs, X0, Y0):
    cells = {}
    lanes = {(x - X0, y - Y0) for (x, y) in road if (x - X0) >= 0}
    # one lane per highway: the road's first row/column
    lane = {p for p in lanes if (p[0], p[1] - 1) not in lanes or (p[0] - 1, p[1]) not in lanes}
    lane = {p for p in lane if (p[0], p[1] - 1) not in lanes}
    src = list(lane)
    for (bx, by, bw, bh, _) in constructs:
        src += [(bx, by), (bx + bw, by + bh)]
    for vy in range(VH):
        for vx in range(VW):
            d = min((abs(vx - sx) / 2 + abs(vy - sy) for sx, sy in src[::3]), default=99)
            k = max(0.0, 1 - d / 6.5)
            lattice = (X0 + vx) % 4 == 0 and (Y0 + vy) % 2 == 0
            cells[(vx, vy)] = ["·" if lattice and k > 0.05 else " ", mix("#010204", "#5a8a84", k),
                               mix("#010204", "#011412", k), False]
            if g.t[vy][vx] in ("mass", "wall") and g.k[vy][vx] != "shanty":
                cells[(vx, vy)] = [" ", "#c8d0dc", "#011916", False]
    dbl = "═║═╚║║╔╠═╝═╩╗╣╦╬"
    for (x, y) in lane:
        if not (0 <= x < VW and 0 <= y < VH):
            continue
        m = 0
        for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
            if (x + dx, y + dy) in lane:
                m |= bit
        cells[(x, y)] = [dbl[m], LANE[0], LANE[1], False]
        if h32(X0 + x, Y0 + y, 61) < 0.04:
            cells[(x, y)] = [dbl[m], "#ffffff", "#00b08b", True]
    for (bx, by, bw, bh, name) in constructs:
        for y in range(by, by + bh):
            for x in range(bx, bx + bw):
                if not (0 <= x < VW and 0 <= y < VH):
                    continue
                ex, ey = x in (bx, bx + bw - 1), y in (by, by + bh - 1)
                if ex or ey:
                    m = (1 if ex and y > by else 0) | (4 if ex and y < by + bh - 1 else 0)
                    m |= (2 if ey and x < bx + bw - 1 else 0) | (8 if ey and x > bx else 0)
                    cells[(x, y)] = ["─│─└││┌├─┘─┴┐┤┬┼"[m], "#00d9aa", "#011110", False]
                else:
                    cells[(x, y)] = [" ", "#c8d0dc", "#014437", False]
        for i, ch in enumerate(name[:bw - 4]):
            if 0 <= bx + 2 + i < VW and 0 <= by + bh // 2 < VH:
                cells[(bx + 2 + i, by + bh // 2)] = [ch, "#b8f3ec", "#014437", True]
        if 0 <= bx + 2 < VW and 0 <= by + 1 < VH:
            cells[(bx + 2, by + 1)] = ["◆", "#e6f0f5", "#014437", True]
    return cells


ow2.paint_net = paint_net


CSS = term.CSS + """
body{background:#0d1117;color:#c9d1d9;font-family:-apple-system,system-ui,sans-serif;margin:24px 32px 60px}
h1{font-size:22px;margin:0 0 6px}h2{font-size:18px;margin:34px 0 4px}
p{max-width:1100px;line-height:1.5;margin:6px 0}.note{color:#8b949e;font-size:13px}
"""


def main():
    w = World(os.path.join(HERE, "dumps", "world_0.txt"))
    stop = next(p for p in w.pois if p["kind"] == "TruckStop")
    home = next(c for c in w.cities if c["home"])
    r17 = next(p for n, p in w.routes if n == "Route 17")
    edge = next(p for p in r17 if w.tile(*p)["city"] < 0)
    inside = next(p for p in r17 if w.tile(*p)["city"] >= 0 and w.tile(*p)["density"] > 0.35 and p[0] < edge[0] + 12)
    frames = [
        ("%s on its highway" % stop["name"], "The world's own truck stop, beside the road the generator ran past it. "
         "One cell a detail tile; the view spans about 10×4 overworld tiles (the minimap pane frames them).",
         frame(w, K * stop["x"] - 32, K * stop["y"] - 13, "%s · Route 3" % "Dust Flats", "dim · seen",
               stop["name"], pois=[stop])),
        ("The same, jacked in", "The trunk along the highway, the stop's construct, the fading grid.",
         frame(w, K * stop["x"] - 32, K * stop["y"] - 13, "Dust Flats · Net", "dead grid · unseen",
               "Route trunk · 1 construct in reach", pois=[stop], net=True)),
        ("The bridge east of it", "Where the highway crosses the river: the road's and the river's crossings "
         "come from each tile edge's hash, so the tiles join.",
         frame(w, K * 61 - 36, K * 36 - 12, "the Bottoms · Route 3", "dim · seen", "bridge over the river")),
        ("Where %s ends, on Route 17" % home["name"], "The city's density, smoothed across tile lines, decides "
         "towers, outskirts and open ground; Route 17 runs out west.",
         frame(w, K * ((edge[0] + inside[0]) // 2) - 40, K * edge[1] - 14, "%s · Route 17" % home["name"], "bright · seen",
               "the edge of %s" % home["name"])),
    ]
    out = ["<!doctype html><meta charset=utf-8><title>Overworld · round 4</title><style>%s</style>" % CSS,
           "<h1>Round 4 · generated worlds, one level down (rough)</h1>",
           "<p>Seed %d, the first world on the sheet, drawn one level down by the tile and city specs' "
           "rules: 8×8 detail tiles a tile, crossings from each edge's hash, the coast and the city's edge "
           "blended across tile lines. A throwaway spike in Python, not the real generator: rough, to see "
           "the idea from real worlds.</p>" % w.seed]
    chk = os.environ.get("CHECK_DIR")
    for i, (title, text, g) in enumerate(frames):
        out.append("<h2>%s</h2><p>%s</p>" % (html.escape(title), text))
        out.append(term.render(g, 13.0))
        if chk:
            open(os.path.join(chk, "r4_%d.html" % i), "w").write(
                "<!doctype html><meta charset=utf-8><style>%s body{margin:0;background:#000}</style>%s"
                % (term.CSS, term.render(g, 14.0)))
    open(OUT, "w").write("\n".join(out))
    print(OUT)


if __name__ == "__main__":
    main()
