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
            elif a[0] == "v":
                self.rivers.append([tuple(map(int, q.split(","))) for q in a[1:]])
            elif a[0] == "g":
                self.regions.append(dict(x=int(a[1]), y=int(a[2]), kind=a[3], name=" ".join(a[4:])))

    def tile(self, tx, ty):
        return self.t.get((tx, ty))

    def river_width(self, tx, ty):
        """Detail tiles wide: 3 near the spring, 7 at the mouth."""
        for r in self.rivers:
            if (tx, ty) in r:
                return round(3 + 4 * r.index((tx, ty)) / max(1, len(r) - 1))
        return 3


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


def crossing(w, tx, ty, side, road=False):
    """Where a link out of tile (tx, ty) on `side` crosses the shared edge, the same
    from both tiles: a road always at the edge's middle, a river at the edge's hash."""
    if side == "W":
        return crossing(w, tx - 1, ty, "E", road)
    if side == "N":
        return crossing(w, tx, ty - 1, "S", road)
    o = 3 if road else 2 + int(h32(w.seed, tx, ty, 1 if side == "E" else 2) * 4) % 4
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


def centreline(w, tx, ty, mask, salt, road=False):
    """The cells a road (or river) runs on through tile (tx, ty): straight from crossing
    to crossing when it only passes through, else from each crossing to a point near the
    centre, four-connected."""
    def inside(side):
        ex, ey = crossing(w, tx, ty, side, road)
        return (ex - 1, ey) if side == "E" else (ex, ey - 1) if side == "S" else (ex, ey)
    if mask in (2 | 8, 1 | 4):
        a, b = (inside("W"), inside("E")) if mask == 10 else (inside("N"), inside("S"))
        return straight(a, b)
    # a road bends at the tile's middle, where its crossings line up; a river anywhere near
    cx = K * tx + 3 + (0 if road else int(h32(w.seed, tx, ty, salt) * 2))
    cy = K * ty + 3 + (0 if road else int(h32(w.seed, tx, ty, salt + 1) * 2))
    out = []
    for bit, side in ((1, "N"), (2, "E"), (4, "S"), (8, "W")):
        if not mask & bit:
            continue
        ex, ey = crossing(w, tx, ty, side, road)
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
    road, river, lane = set(), set(), set()
    txs = range((X0 // K) - 1, (X0 + VW) // K + 2)
    tys = range((Y0 // K) - 1, (Y0 + VH) // K + 2)
    for ty in tys:
        for tx in txs:
            t = w.tile(tx, ty)
            if not t:
                continue
            if t["river"]:
                wd = w.river_width(tx, ty)
                for (x, y) in centreline(w, tx, ty, t["river"], 7):
                    for dy in range(-(wd // 2), wd - wd // 2):
                        for dx in range(-(wd // 2), wd - wd // 2):
                            river.add((x + dx, y + dy))
            if t["road"]:
                for (x, y) in centreline(w, tx, ty, t["road"], 3, road=True):
                    lane.add((x, y))
                    road.update({(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)})
    wob = fbm  # world-wide noise: (x, y) are detail coordinates
    for vy in range(VH):
        for vx in range(VW):
            x, y = X0 + vx, Y0 + vy
            # shores curved at three scales: coves, points, the odd spit
            shore = (0.34 * (wob(x / 16, y / 8, 11, 3) - 0.5) + 0.22 * (wob(x / 6, y / 3, 17, 3) - 0.5)
                     + 0.1 * (wob(x / 2.5, y / 1.25, 18, 2) - 0.5))
            sea = bilinear(w, x, y, lambda t: t["water"] == "Sea") + shore
            lake = bilinear(w, x, y, lambda t: t["water"] == "Lake") + shore
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
    # cities: the world's density, smoothed across tiles. No building is cut by water:
    # a block the shore reaches gets a building fitted to its dry part (or none), and the
    # waterfront is a paved promenade with lamps.
    wet = {(X0 + vx, Y0 + vy) for vy in range(VH) for vx in range(VW)
           if g.t[vy][vx] in ("sea", "lake", "river")}

    def near_water(x, y, r=2):
        return any((x + dx, y + dy) in wet for dx in range(-r, r + 1) for dy in (-1, 0, 1)
                   if abs(dx) + 2 * abs(dy) <= r)
    fitted = {}

    def dry_rect(bk):
        if bk in fitted:
            return fitted[bk]
        l, t = bk[0] * 14 + 2, bk[1] * 7 + 1
        r, b = l + 11, t + 5
        while r - l >= 3 and b - t >= 2:
            bad = [(x, y) for x in range(l, r + 1) for y in range(t, b + 1) if near_water(x, y)]
            if not bad:
                fitted[bk] = (l, t, r, b)
                return fitted[bk]
            sides = {"l": sum(x == l for x, _ in bad), "r": sum(x == r for x, _ in bad),
                     "t": sum(y == t for _, y in bad), "b": sum(y == b for _, y in bad)}
            side = max(sorted(sides), key=lambda k: sides[k])
            if side == "l":
                l += 1
            elif side == "r":
                r -= 1
            elif side == "t":
                t += 1
            else:
                b -= 1
        fitted[bk] = None
        return None
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
                    rect = dry_rect((x // 14, y // 7))
                    if rect and rect[0] <= x <= rect[2] and rect[1] <= y <= rect[3]:
                        l, t, r, b = rect
                        if x in (l, r) or y in (t, b):
                            corner = x in (l, r) and y in (t, b)
                            g.set(vx, vy, "window" if not corner and (x + y) % 2 == 0 else "wall", kind)
                        else:
                            g.set(vx, vy, "mass", kind)
                    else:
                        g.set(vx, vy, "side" if near_water(x, y, 3) else "plaza")
                        if near_water(x, y, 2) and not near_water(x, y, 1) and x % 7 == 0:
                            g.feat(vx, vy, "lamp")
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
    # the shore band: land a tile or two from the water
    wet = {(vx, vy) for vy in range(VH) for vx in range(VW) if g.t[vy][vx] in ("sea", "lake", "river")}
    for vy in range(VH):
        for vx in range(VW):
            if (vx, vy) in wet:
                continue
            near = [g.t[vy + dy][vx + dx] for dx in (-2, -1, 0, 1, 2) for dy in (-1, 0, 1)
                    if (vx + dx, vy + dy) in wet and abs(dx) + 2 * abs(dy) <= 2]
            if near and g.t[vy][vx] in ("dirt", "scrub", "crop", "salt", "rock", "gravel", "track", "lot"):
                g.set(vx, vy, "sand" if "sea" in near else "reeds")
    g.lane = lane
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
        return [dict(rect=(bx, by, abs(x1 - x0) + 1, abs(y1 - y0) + 1),
                     name=poi["name"].split()[0].upper(), gate=(dx, dy), camera=at(9, 2),
                     sign=(sx, sy))]
    return []


def depth_of(g, vx, vy):
    """How far (tiles, rows count double) the water at (vx, vy) is from land."""
    for d in range(1, 8):
        for dy in range(-((d + 1) // 2), (d + 1) // 2 + 1):
            for dx in range(-d, d + 1):
                x, y = vx + dx, vy + dy
                if g.inb(x, y) and g.t[y][x] not in ("sea", "lake", "river") and abs(dx) + 2 * abs(dy) <= d:
                    return d
    return 8


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
            depth = depth_of(g, vx, vy)
            k = min(1.0, (depth - 1) / 5)
            bg = mix("#123c5e", "#050d1a", k)
            fg = mix("#6aa6d8", "#1e4a78", k)
            ch = "≈~∼"[int(r * 3) % 3] if r < 0.55 - 0.35 * k else " "
            cells[(vx, vy)] = [ch, fg, bg, False]
        elif t == "sand":
            cells[(vx, vy)] = ["░" if r < 0.4 else "·" if r < 0.7 else " ", "#8a7a58", "#1c1812", False]
        elif t == "reeds":
            cells[(vx, vy)] = ['"' if r < 0.45 else "·" if r < 0.7 else " ", "#5e7040", "#0f120b", False]
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
    """The Net over the view: the highways' trunks (one lane down each road's line),
    each POI's construct wired to the trunk by a spur from its gate, its lone nodes on
    lines of their own, and the grid fading away from all of it."""
    lane = {(x - X0, y - Y0) for (x, y) in g.lane}
    spur, light, nodes = set(), set(), []

    def nearest(p, among):
        return min(among, key=lambda q: abs(q[0] - p[0]) + 2 * abs(q[1] - p[1]), default=None)

    for c in constructs:
        if lane:
            spur.update(straight(c["gate"], nearest(c["gate"], lane))[1:])
        nodes.append((c["camera"], "#9fe6ff"))
        nodes.append((c["sign"], "#9fe6ff"))
        if lane:
            spur.update(straight(c["sign"], nearest(c["sign"], lane))[1:])
        if spur:
            light.update(straight(c["camera"], nearest(c["camera"], spur | lane))[1:-1])
    wires = lane | spur
    src = list(wires) + [n for n, _ in nodes]
    for c in constructs:
        bx, by, bw, bh = c["rect"]
        src += [(bx, by), (bx + bw, by + bh), (bx + bw // 2, by + bh // 2)]
    cells = {}
    for vy in range(VH):
        for vx in range(VW):
            d = min((abs(vx - sx) / 2 + abs(vy - sy) for sx, sy in src[::2]), default=99)
            k = max(0.0, 1 - d / 6.5)
            lattice = (X0 + vx) % 4 == 0 and (Y0 + vy) % 2 == 0
            cells[(vx, vy)] = ["·" if lattice and k > 0.05 else " ", mix("#010204", "#5a8a84", k),
                               mix("#010204", "#011412", k), False]
            if g.t[vy][vx] in ("mass", "wall") and g.k[vy][vx] not in ("shanty", "stop"):
                cells[(vx, vy)] = [" ", "#c8d0dc", "#011916", False]
    dbl = "═║═╚║║╔╠═╝═╩╗╣╦╬"
    thin = "─│─└││┌├─┘─┴┐┤┬┼"

    def mask(p, among):
        m = 0
        for bit, (dx, dy) in ((1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))):
            if (p[0] + dx, p[1] + dy) in among:
                m |= bit
        return m
    for (x, y) in light:
        if 0 <= x < VW and 0 <= y < VH:
            cells[(x, y)] = [thin[mask((x, y), light | wires) & 15], "#00a27f", cells[(x, y)][2], False]
    for (x, y) in wires:
        if not (0 <= x < VW and 0 <= y < VH):
            continue
        m = mask((x, y), wires)
        if (x, y) in lane and (x in (0, VW - 1)):
            m |= 2 | 8
        cells[(x, y)] = [dbl[m], LANE[0], LANE[1], False]
        if (x, y) in lane and h32(X0 + x, Y0 + y, 61) < 0.05:
            cells[(x, y)] = [dbl[m], "#ffffff", "#00b08b", True]
    for c in constructs:
        bx, by, bw, bh = c["rect"]
        for y in range(by, by + bh):
            for x in range(bx, bx + bw):
                if not (0 <= x < VW and 0 <= y < VH):
                    continue
                ex, ey = x in (bx, bx + bw - 1), y in (by, by + bh - 1)
                if ex or ey:
                    m = (1 if ex and y > by else 0) | (4 if ex and y < by + bh - 1 else 0)
                    m |= (2 if ey and x < bx + bw - 1 else 0) | (8 if ey and x > bx else 0)
                    ch = thin[m]
                    if (x - bx) % 5 == 2 and not (ex and ey):
                        ch = "▪"
                    cells[(x, y)] = [ch, "#00d9aa" if ch != "▪" else "#73f8df", "#011110", False]
                else:
                    cells[(x, y)] = [" ", "#c8d0dc", "#014437", False]
        name = c["name"]
        nx, ny = bx + (bw - len(name)) // 2, by + bh // 2
        for i, ch in enumerate(name):
            if 0 <= nx + i < VW and 0 <= ny < VH and bx < nx + i < bx + bw - 1:
                cells[(nx + i, ny)] = [ch, "#b8f3ec", "#014437", True]
        for (px, py) in ((bx + 2, by + 1), (bx + bw - 3, by + bh - 2)):
            if 0 <= px < VW and 0 <= py < VH:
                cells[(px, py)] = ["◆", "#e6f0f5", "#014437", True]
        gx, gy = c["gate"]
        if 0 <= gx < VW and 0 <= gy < VH:
            cells[(gx, gy)] = ["▯", "#e6f0f5", "#014e3f", True]
    for (x, y), col in nodes:
        if 0 <= x < VW and 0 <= y < VH:
            cells[(x, y)] = ["◇", col, "#011916", True]
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
    hi = w.cities.index(home)
    # the home city's longest road out, its last dense tile and its first open one
    out_of = max((p if p[0] == (home["x"], home["y"]) else p[::-1]
                  for n, p in w.routes if (home["x"], home["y"]) in (p[0], p[-1])), key=len)
    edge = next(p for p in out_of if w.tile(*p)["city"] != hi)
    # where it thins: the last tile along the road out that is still properly city
    thick = [p for p in out_of[:out_of.index(edge)] if w.tile(*p)["density"] >= 0.3]
    thin_at = thick[-1] if thick else edge
    mid = (thin_at[0] - 1, thin_at[1])
    bridge = next(p for p, t in sorted(w.t.items()) if t["road"] and t["water"] == "River" and t["city"] < 0)
    lake = [p for p, t in sorted(w.t.items()) if t["water"] == "Lake"]
    lx = sum(p[0] for p in lake) // len(lake)
    ly = sum(p[1] for p in lake) // len(lake)
    sx, sy = K * stop["x"] - 36, K * stop["y"] - 13
    frames = [
        ("%s on its highway" % stop["name"], "The world's own truck stop beside the highway the generator "
         "ran past it. The highway crosses every tile edge at its middle, so it runs dead straight "
         "and turns only in clean corners. One cell a detail tile; the minimap frames the view's tiles.",
         frame(w, sx, sy, "Dust Flats · highway", "dim · seen", stop["name"], pois=[stop])),
        ("The same, jacked in", "The trunk down the highway's line; the stop's construct wired to it by a "
         "spur from its gate; the pump camera off the spur, the sign's ad server on its own; the grid "
         "fading away from it all (round 3's Kilo-9).",
         frame(w, sx, sy, "Dust Flats · Net", "dead grid · unseen", "trunk · 1 construct in reach",
               pois=[stop], net=True)),
        ("A bridge over a wide river", "Rivers widen from 3 detail tiles at the spring to 7 at the mouth, "
         "with reeds on the banks, shallows at the edges and depth down the middle; the highway spans it.",
         frame(w, K * bridge[0] - 36, K * bridge[1] - 12, "the Bottoms · bridge", "dim · seen",
               "bridge over the river")),
        ("A lake's shore", "Shores curved at three scales, a band of reeds and mud, shallows, then deep water.",
         frame(w, K * lx - 40, K * ly - 14, "lakeshore", "dim · seen", "the lake")),
        ("Where %s ends" % home["name"], "The city's density, smoothed across tile lines, decides towers, "
         "outskirts and open ground along its longest road out.",
         frame(w, K * mid[0] - 36, K * mid[1] - 13, "%s · edge" % home["name"], "bright · seen",
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
