"""Indoor lighting, round 1: three ways to light an interior that isn't a street lamp,
relit over the game's own frames.

    python3 -I gen.py SHOTS_DIR out.html

SHOTS_DIR holds <name>.cells (x, y, symbol, fg, bg, bold for a 120x40 frame) and
<name>.tiles (a `#` line with the screen offset of tile 0,0, then x, y, terrain,
feature, light, room `index:Kind`, walkable per tile), from a throwaway shot test.

A · Ceiling fixtures: the lamp becomes the room's own fixture, drawn on the floor
    (tubes in corridors and offices, a bulb in a flat, pendants over a bar's tables,
    the surgical lamp over the chair), each throwing a pool in its own colour.
B · Lit from the walls: no fixtures in the room; strips, sconces and a neon backbar
    sit in the walls and the light comes in from the edges.
C · Room tone: no fixtures and no pools; every room takes an even cast by its kind."""

import html
import math
import os
import sys

S = sys.argv[1]
OUT = sys.argv[2]
W, H = 120, 40
MX0, MY0, MX1, MY1 = 0, 5, 79, 35
PANEL_BG = "#0a0b10"
TEXT = "#cdd4de"

# ---- colour helpers


def rgb(c):
    return [int(c[i:i + 2], 16) for i in (1, 3, 5)]


def hexc(v):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(round(x)))) for x in v)


# ---- light families: the colour each kind of fixture throws, and its glyph colour

FAM = {
    "fluo": dict(col=(185, 232, 222), glyph="#eafffa"),   # corridor tubes, cold green-white
    "led": (dict(col=(175, 205, 255), glyph="#e8f0ff")),  # office panels, cold blue-white
    "warm": dict(col=(255, 168, 88), glyph="#ffd9a0"),    # tungsten: flats, rooms, bars
    "surg": dict(col=(228, 242, 255), glyph="#ffffff"),   # the clinic's hard white
    "sodium": dict(col=(255, 150, 55), glyph="#ffc070"),  # high-bay in halls and bays
    "neon": dict(col=(255, 60, 175), glyph="#ff5cc8"),    # a bar's backbar, the stage
    "screen": dict(col=(0, 225, 210), glyph="#00e6d2"),   # a rig or terminal's glow
    "club": dict(col=(175, 95, 255), glyph="#d9a8ff"),    # a club's floor: violet wash
}

KIND_FAM = {
    "Corridor": "fluo", "Laundry": "fluo", "Lockers": "fluo", "Toilets": "fluo",
    "Storage": "fluo", "Waiting": "fluo", "Lobby": "led", "OpenPlan": "led",
    "Office": "led", "Shop": "led", "Flat": "warm", "Home": "warm", "HotelRoom": "warm",
    "Suite": "warm", "BackRoom": "warm", "Vip": "warm", "Bar": "warm", "Shrine": "warm",
    "Surgery": "surg", "Hall": "sodium", "Bays": "sodium", "Stage": "neon", "Club": "club",
    None: "fluo",
}

# Ambient per family: A and B light the room mostly from their fixtures; C only by tone.
AMB_A = dict(fluo=66, led=84, warm=40, surg=110, sodium=56, neon=44, club=46)
AMB_C = dict(fluo=150, led=168, warm=104, surg=215, sodium=128, neon=110, club=112)

# ---- loading


def load(name):
    cells = [[[" ", TEXT, PANEL_BG, False] for _ in range(W)] for _ in range(H)]
    for line in open(os.path.join(S, name + ".cells")):
        x, y, ch, fg, bg, bold = line.rstrip("\n").split("\t")
        cells[int(y)][int(x)] = [ch or " ", fg or TEXT, bg or PANEL_BG, bold == "true"]
    tiles = {}
    with open(os.path.join(S, name + ".tiles")) as f:
        head = f.readline().rstrip("\n").split("\t")
        ox, oy = int(head[1]), int(head[2])
        for line in f:
            x, y, terrain, feature, light, room, walk = line.rstrip("\n").split("\t")
            idx, kind = (None, None)
            if room != "-":
                i, k = room.split(":")
                idx, kind = int(i), k
            tiles[(int(x), int(y))] = dict(
                terrain=terrain, feature=feature, light=int(light), room=idx, kind=kind,
                walk=walk == "true")
    return cells, tiles, ox, oy


def blocks(t):
    return t is None or t["terrain"] in ("Wall", "Door", "Window")


def line(a, b):
    (x0, y0), (x1, y1) = a, b
    dx, dy = abs(x1 - x0), -abs(y1 - y0)
    sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
    err = dx + dy
    out = []
    while True:
        out.append((x0, y0))
        if (x0, y0) == (x1, y1):
            return out
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


class Scene:
    def __init__(self, name):
        self.name = name
        self.cells, self.tiles, self.ox, self.oy = load(name)
        self.orig = [[list(c) for c in row] for row in self.cells]
        # Screen cell of each tile shown on the map.
        self.screen = {}
        for y in range(MY0, MY1):
            for x in range(MX0, MX1):
                p = (x - self.ox, y - self.oy)
                c = self.cells[y][x]
                if p in self.tiles and not (c[0] == " " and c[2] == "#000000"):
                    self.screen[p] = (x, y)
        # Unlit base colours.
        self.base = {}
        for p, (x, y) in self.screen.items():
            t = self.tiles[p]
            ch, fg, bg, bold = self.cells[y][x]
            f = 0.25 + 0.75 * t["light"] / 255
            emits = t["feature"] in ("Some(Lamp)", "Some(Terminal)") or ch == "@"
            self.base[p] = (
                [v / f for v in rgb(fg)] if not emits else rgb(fg),
                [v / f for v in rgb(bg)],
                emits,
            )
        # A floor dot's base colours, to repaint tiles whose lamp goes.
        dots = [p for p, (x, y) in self.screen.items() if self.cells[y][x][0] == "·"]
        self.dot = self.base[dots[0]] if dots else ([60, 60, 60], [20, 20, 20], False)
        # Rooms: walkable tiles grouped by room; tiles in none make one more group.
        self.rooms = {}
        for p, t in self.tiles.items():
            if t["walk"] or t["feature"] != "None":
                if t["terrain"] in ("Wall", "Door", "Window"):
                    continue
                key = t["room"] if t["room"] is not None else -1
                self.rooms.setdefault(key, dict(kind=t["kind"], tiles=set()))["tiles"].add(p)
        self.player = next(p for p, (x, y) in self.screen.items() if self.cells[y][x][0] == "@")
        lamps = {p for p, t in self.tiles.items() if t["feature"] == "Some(Lamp)"}
        near = lambda p: sum((p[0] + dx, p[1] + dy) in lamps for dx in (-1, 0, 1) for dy in (-1, 0, 1))
        stage = {p for p in lamps if near(p) >= 4}
        if stage:
            for room in self.rooms.values():
                room["tiles"] -= stage
            self.rooms[-2] = dict(kind="Stage", tiles=stage)
            for p in stage:
                self.tiles[p]["kind"] = "Stage"
        if name == "indoor_club":
            for t in self.tiles.values():
                if t["kind"] == "Bar":
                    t["kind"] = "Club"
            for room in self.rooms.values():
                if room["kind"] == "Bar":
                    room["kind"] = "Club"

    def fam_of(self, p):
        t = self.tiles.get(p)
        if t is None:
            return "fluo"
        if t["terrain"] in ("Wall", "Door", "Window"):
            for d in ((0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, -1), (1, -1), (-1, 1)):
                n = self.tiles.get((p[0] + d[0], p[1] + d[1]))
                if n and n["terrain"] not in ("Wall", "Door", "Window"):
                    return KIND_FAM.get(n["kind"], "fluo")
            return "fluo"
        return KIND_FAM.get(t["kind"], "fluo")

    def visible(self, a, b):
        pts = line(a, b)
        return all(not blocks(self.tiles.get(q)) for q in pts[1:-1])


# ---- relighting


def relight(sc, amb, emitters, glyphs, wash=None, out_of_tone=False):
    """A new frame: ambient by family (`amb`), emitters [(pos, radius, strength, fam)],
    glyphs {pos: (ch, fg, bg or None, bold)} drawn after, `wash` {pos: (fam, w)} extra
    tint on a tile."""
    L = {p: [0.0, 0.0, 0.0] for p in sc.screen}
    for p in sc.screen:
        fam = sc.fam_of(p)
        a = amb[fam] if isinstance(amb, dict) else amb
        col = FAM[fam]["col"]
        L[p] = [a * c / 255 for c in col]
    for pos, r, s, fam in emitters:
        col = FAM[fam]["col"]
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                q = (pos[0] + dx, pos[1] + dy)
                if q not in L:
                    continue
                d = math.hypot(dx, dy)
                if d > r or not sc.visible(pos, q):
                    continue
                k = s * (1 - d / (r + 0.5))
                for i in range(3):
                    L[q][i] += k * col[i] / 255
    frame = [[list(c) for c in row] for row in sc.orig]
    for p, (x, y) in sc.screen.items():
        t = sc.tiles[p]
        bfg, bbg, emits = sc.base[p]
        ch = frame[y][x][0]
        if t["feature"] == "Some(Lamp)" and t["kind"] != "Stage":
            bfg, bbg, emits = sc.dot
            ch = "·"
        if t["feature"] == "Some(Lamp)" and t["kind"] == "Stage":
            bfg, bbg, emits = sc.dot
            ch = "·"
        if ch == "@":
            emits = True
        Lp = L[p]
        k = [min(1.15, v / 255) for v in Lp]
        bg = [bbg[i] * (0.22 + 0.78 * k[i]) + Lp[i] * 0.10 for i in range(3)]
        fg = bfg if emits else [bfg[i] * (0.30 + 0.70 * k[i]) + Lp[i] * 0.16 for i in range(3)]
        if wash and p in wash:
            fam, w = wash[p]
            col = FAM[fam]["col"]
            bg = [bg[i] * (1 - w) + col[i] * w * 0.35 for i in range(3)]
        frame[y][x][0] = ch
        frame[y][x][1] = hexc(fg)
        frame[y][x][2] = hexc(bg)
    for p, (ch, fg, bg, bold) in glyphs.items():
        if p not in sc.screen:
            continue
        x, y = sc.screen[p]
        frame[y][x][0] = ch
        frame[y][x][1] = fg
        if bg:
            frame[y][x][2] = bg
        frame[y][x][3] = bold
    return frame


def free(sc, p):
    t = sc.tiles.get(p)
    return t is not None and t["walk"] and t["feature"] in ("None", "Some(Lamp)") and p != sc.player


def centroid(tiles):
    xs = [p[0] for p in tiles]
    ys = [p[1] for p in tiles]
    return (sum(xs) / len(xs), sum(ys) / len(ys))


def bbox(tiles):
    xs = [p[0] for p in tiles]
    ys = [p[1] for p in tiles]
    return min(xs), min(ys), max(xs), max(ys)


def feature_tiles(sc, room, name):
    return [p for p in room["tiles"] if name in sc.tiles[p]["feature"]]


def nearest_free(sc, room, at):
    cand = [p for p in room["tiles"] if free(sc, p)]
    if not cand:
        return None
    return min(cand, key=lambda p: (p[0] - at[0]) ** 2 + (p[1] - at[1]) ** 2)


def edge_walls(sc, room):
    """Wall tiles beside the room's floor, each with the side the room lies on."""
    out = {}
    for p in room["tiles"]:
        for d in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            q = (p[0] + d[0], p[1] + d[1])
            t = sc.tiles.get(q)
            if t and t["terrain"] == "Wall" and q not in out:
                out[q] = (-d[0], -d[1])  # from the wall toward the room
    return out


def screens(sc):
    """A rig or terminal glows wherever it is."""
    em = []
    for p, t in sc.tiles.items():
        if t["feature"] == "Some(Terminal)" and p in sc.screen:
            em.append((p, 3, 90, "screen"))
    return em


def stage(sc, room):
    """The club's stage: its lamps become a neon wash."""
    em, wash = [], {}
    for p in room["tiles"]:
        fam = "neon" if p[1] % 4 < 2 else "screen"
        wash[p] = (fam, 0.45)
        if (p[0] + p[1]) % 3 == 0:
            em.append((p, 3, 60, fam))
    return em, wash


# ---- A: ceiling fixtures


def option_a(sc):
    em, gl, wash = screens(sc), {}, {}
    tubes_seen = 0
    for key, room in sc.rooms.items():
        kind = room["kind"]
        fam = KIND_FAM.get(kind, "fluo")
        if kind == "Stage":
            e, w = stage(sc, room)
            em += e
            wash.update(w)
            continue
        if fam in ("fluo", "led", "sodium", "surg"):
            # Tubes lie across the map's rows: one cell in a narrow run, two in a room.
            x0, y0, x1, y1 = bbox(room["tiles"])
            w, h = x1 - x0 + 1, y1 - y0 + 1
            long = 2 if w >= 5 else 1
            cols = [x0 + (w - long) // 2] if w <= 7 else list(range(x0 + 2, x1 - 1, 6))
            rows = [y0 + h // 2] if h <= 4 else list(range(y0 + 2, y1, 4))
            tone = "fluo" if fam == "surg" else fam
            for r in rows:
                for c in cols:
                    pts = [(c + i, r) for i in range(long)]
                    if not all(free(sc, q) for q in pts):
                        alt = [(c + i, r + 1) for i in range(long)]
                        if not all(free(sc, q) for q in alt):
                            continue
                        pts = alt
                    tubes_seen += 1
                    state = "ok"
                    if fam == "fluo" and tubes_seen % 7 == 3:
                        state = "dead"
                    elif fam == "fluo" and tubes_seen % 7 == 5:
                        state = "stutter"
                    for q in pts:
                        if state == "dead":
                            gl[q] = ("▬", "#3e4750", None, False)
                        elif state == "stutter":
                            gl[q] = ("▬", hexc([v * 0.6 for v in rgb(FAM[tone]["glyph"])]), None, False)
                            em.append((q, 3, 60 / long, tone))
                        else:
                            gl[q] = ("▬", FAM[tone]["glyph"], None, False)
                            em.append((q, 5, 200 / long, tone))
            if fam == "surg":
                for c in feature_tiles(sc, room, "Chair"):
                    em.append((c, 3, 210, "surg"))
                    x, y = sc.screen.get(c, (None, None))
                    if x is not None:
                        gl[c] = (sc.orig[y][x][0], "#ffffff", None, True)
            continue
        if kind in ("Bar", "Vip", "Club"):
            tone = "club" if kind == "Club" else "warm"
            for tb in feature_tiles(sc, room, "Table"):
                em.append((tb, 3, 150, tone))
                x, y = sc.screen.get(tb, (None, None))
                if x is not None:
                    gl[tb] = (sc.orig[y][x][0], FAM[tone]["glyph"] if tone == "club" else "#ffc777", None, False)
            for ct in feature_tiles(sc, room, "Counter"):
                em.append((ct, 2, 55, tone))
            continue
        # Warm rooms: one bulb, beside the bed if there is one.
        beds = feature_tiles(sc, room, "Bed")
        at = centroid(beds) if beds else centroid(room["tiles"])
        b = nearest_free(sc, room, at)
        if b:
            gl[b] = ("•", FAM["warm"]["glyph"], None, True)
            em.append((b, 5, 190, "warm"))
    return relight(sc, AMB_A, em, gl, wash)


# ---- B: lit from the walls

SIDE_BLOCK = {(0, 1): "▄", (0, -1): "▀", (1, 0): "▐", (-1, 0): "▌"}


def option_b(sc):
    em, gl, wash = screens(sc), {}, {}
    for key, room in sc.rooms.items():
        kind = room["kind"]
        fam = KIND_FAM.get(kind, "fluo")
        if kind == "Stage":
            e, w = stage(sc, room)
            em += e
            wash.update(w)
            continue
        walls = edge_walls(sc, room)
        if fam in ("fluo", "led", "sodium", "surg"):
            n = 0
            for q, side in sorted(walls.items()):
                if q not in sc.screen:
                    continue
                run = q[0] if side[0] == 0 else q[1]
                lit = fam == "surg" or (run // 3) % 2 == 0
                if not lit:
                    continue
                n += 1
                out = fam == "fluo" and n % 11 == 6
                x, y = sc.screen[q]
                col = FAM[fam]
                if out:
                    continue
                gl[q] = (sc.orig[y][x][0], col["glyph"], hexc([c * 0.22 for c in col["col"]]), True)
                em.append((q, 3, 105 if fam != "surg" else 130, fam))
            continue
        if kind in ("Bar", "Vip", "Club"):
            counters = feature_tiles(sc, room, "Counter")
            for q, side in walls.items():
                if q not in sc.screen:
                    continue
                near = any(abs(q[0] - c[0]) + abs(q[1] - c[1]) <= 2 for c in counters)
                x, y = sc.screen[q]
                if near:
                    gl[q] = (sc.orig[y][x][0], FAM["neon"]["glyph"], "#2c0a22", True)
                    em.append((q, 4, 110, "neon"))
                elif ((q[0] if side[0] == 0 else q[1]) % 5) == 2:
                    tone = "club" if kind == "Club" else "warm"
                    gl[q] = (sc.orig[y][x][0], FAM[tone]["glyph"], hexc([c * 0.22 for c in FAM[tone]["col"]]), True)
                    em.append((q, 3, 95, tone))
            continue
        # Warm rooms: a sconce on the wall nearest the bed (else the room's middle).
        beds = feature_tiles(sc, room, "Bed")
        at = centroid(beds) if beds else centroid(room["tiles"])
        cand = [q for q in walls if q in sc.screen]
        if cand:
            q = min(cand, key=lambda p: (p[0] - at[0]) ** 2 + (p[1] - at[1]) ** 2)
            x, y = sc.screen[q]
            gl[q] = (sc.orig[y][x][0], FAM["warm"]["glyph"], "#4a2c10", True)
            em.append((q, 5, 190, "warm"))
    return relight(sc, AMB_A, em, gl, wash)


# ---- C: room tone


def option_c(sc):
    wash = {}
    for key, room in sc.rooms.items():
        if room["kind"] == "Stage":
            for p in room["tiles"]:
                wash[p] = ("neon", 0.45)
    return relight(sc, AMB_C, [], {}, wash)


# ---- the page


def crop_box(sc):
    xs = [x for (x, y) in sc.screen.values()]
    ys = [y for (x, y) in sc.screen.values()]
    x0, x1 = max(MX0, min(xs) - 1), min(MX1 - 1, max(xs) + 1)
    y0, y1 = max(MY0, min(ys) - 1), min(MY1 - 1, max(ys) + 1)
    return x0, y0, x1, y1


def term(frame, box, fs):
    x0, y0, x1, y1 = box
    out = ['<div class="term" style="font-size:%dpx">' % fs]
    for y in range(y0, y1 + 1):
        out.append('<div class="r">')
        run, key = [], None

        def flush():
            if run:
                fg, bg, bold = key
                st = "color:%s;background:%s%s" % (fg, bg, ";font-weight:700" if bold else "")
                out.append('<span style="%s">%s</span>' % (st, html.escape("".join(run))))

        for x in range(x0, x1 + 1):
            ch, fg, bg, bold = frame[y][x]
            k = (fg, bg, bold)
            if k != key:
                flush()
                run, key = [], k
            run.append(ch)
        flush()
        out.append("</div>")
    out.append("</div>")
    return "".join(out)


SCENES = [
    ("indoor_bar", "Bar Mirasys · L", "A bar: tables, stools, the counter along the bottom."),
    ("indoor_hotel", "The Tessgen Inn · 2F", "A hotel floor: the corridor down the middle, rooms off it."),
    ("indoor_office", "Hextek Holdings Plaza · 2F", "An office floor: open plan and offices off a corridor."),
    ("indoor_clinic", "Miracorp Tattoo · L", "A ripperdoc's: waiting room, the surgery with the chair, the back room."),
    ("indoor_club", "A club · L", "A club: tables, the bar down the left, and the stage (today a block of lamps) in the corner."),
    ("indoor_flat", "Your flat · 18F", "Your flat: the bed, the rig, the counter."),
]

OPTIONS = [
    ("now", "Now", "Today's frame: the street lamp, on a six-tile grid."),
    ("a", "A · Ceiling fixtures", "The room's own fixture on the floor, a pool in its colour."),
    ("b", "B · Lit from the walls", "Strips, sconces and neon in the walls; the floor stays clear."),
    ("c", "C · Room tone", "No fixtures; each room an even cast by its kind."),
]

INTRO = """<h1>Indoor lighting · round 1</h1>
<p>Indoors today every room gets the same thing as the street: a <b>●</b> called
<i>Street lamp</i> ("Sodium light and a hum you stop hearing"), planted on a fixed six-tile grid
whatever the room, lighting a floor that's already at 150 of 255. So a flat, a bar, a corridor and a
surgery all read alike, and the lamps sit in the middle of beds and walkways. Below are three
directions, relit over the game's own frames (120×40, the map only), each next to today's frame.
They share one idea: <b>indoor light has a colour by what lights the room</b>, and rooms go darker so
the light shows.</p>
<div class="key">
<span><i style="background:#b9e8de"></i>fluorescent · corridors, laundries, waiting rooms</span>
<span><i style="background:#afcdff"></i>LED panel · offices, lobbies, shops</span>
<span><i style="background:#ffa858"></i>tungsten · flats, hotel rooms, bars, back rooms</span>
<span><i style="background:#e4f2ff"></i>surgical white · the doc's chair</span>
<span><i style="background:#ff3caf"></i>neon · a backbar, a stage</span>
<span><i style="background:#00e1d2"></i>screen glow · your rig, terminals</span>
</div>
<div class="opts">
<div><h3>A · Ceiling fixtures</h3><p>The lamp splits into the room's own fixture, drawn where it hangs:
tubes <b>▬▬</b> along corridors and offices, one bulb <b>•</b> by a flat's bed, pendants over a bar's
tables (the table glows), the surgical lamp over the chair. Each throws a smaller pool in its own
colour. The city's wear shows: a dead tube (grey) and a stuttering one (dim, it flickers in the game) along a corridor.
You can see where the light is, which a later light switch or a shot-out tube can use.</p>
<p class="look">x on a tube: <b>Fluorescent tube</b> · One end flickers. Nobody's coming to fix it.</p></div>
<div><h3>B · Lit from the walls</h3><p>Nothing hangs in the room. Light sits in the walls: LED strips in
runs along corridors and offices (lit wall segments), a warm sconce in the wall by the bed, a neon backbar behind
the counter, the surgery's walls all light. The floor stays clear and rooms read dark in the middle,
bright at the edges, which suits the genre. Wall fixtures need no floor space and never sit on a bed.</p>
<p class="look">x on a strip: <b>Light strip</b> · A cold line along the wall, one segment dead.</p></div>
<div><h3>C · Room tone</h3><p>No fixtures and no pools. Every room takes an even cast by its kind: cold
green-white corridors, blue-white offices, a warm low flat, a white surgery, a dark warm bar. The
cheapest to build and the quietest map; A or B could later add their fixtures on top of it.</p>
<p class="look">x on the floor: nothing new; the room's name says what lights it.</p></div>
</div>"""

CSS = """
.term{font-family:'Fira Code','JetBrains Mono','Menlo','DejaVu Sans Mono',monospace;line-height:1.43;
 display:inline-block;white-space:pre;letter-spacing:0;border-radius:4px;overflow:hidden;
 box-shadow:0 0 0 1px #1a2230, 0 8px 30px rgba(0,0,0,.5)}
.term .r{height:1.43em}
.term span{display:inline-block;height:1.43em;vertical-align:top}
body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;margin:24px auto;max-width:1500px;padding:0 16px}
h1{font-size:22px;margin:0 0 6px}h2{font-size:18px;margin:40px 0 2px;color:#fff}h3{margin:0 0 4px;font-size:15px;color:#fff}
p{max-width:900px;color:#a0aab9;margin:4px 0 8px}
.key{display:flex;flex-wrap:wrap;gap:6px 18px;font-size:13px;color:#a0aab9;margin:10px 0 16px}
.key i{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:6px;vertical-align:-1px}
.opts{display:grid;grid-template-columns:repeat(3,1fr);gap:18px}.opts p{font-size:14px}
.look{font-family:'Fira Code',Menlo,monospace;font-size:12.5px!important;color:#8a94a3!important}
.row{display:flex;flex-wrap:wrap;gap:16px;align-items:flex-start}.row>div{width:min-content}
.cap{font-size:13px;color:#a0aab9;margin:6px 0 0}.cap b{color:#fff}
@media(max-width:900px){.opts{grid-template-columns:1fr}}
"""


def main():
    parts = ["<!doctype html><meta charset='utf-8'><title>Indoor lighting · round 1</title>",
             "<style>", CSS, "</style>", INTRO]
    only = os.environ.get("ONLY")
    for name, title, blurb in SCENES:
        if only and only not in name:
            continue
        if not os.path.exists(os.path.join(S, name + ".cells")):
            continue
        sc = Scene(name)
        frames = dict(now=sc.orig, a=option_a(sc), b=option_b(sc), c=option_c(sc))
        box = crop_box(sc)
        w = box[2] - box[0] + 1
        fs = 20 if w < 24 else (15 if w < 40 else 12)
        fs = int(fs * float(os.environ.get("ZOOM", "1")))
        parts.append("<h2>%s</h2><p>%s</p><div class='row'>" % (html.escape(title), html.escape(blurb)))
        for key, label, cap in OPTIONS:
            parts.append("<div>%s<div class='cap'><b>%s</b> · %s</div></div>" % (
                term(frames[key], box, fs), html.escape(label), html.escape(cap)))
        parts.append("</div>")
    open(OUT, "w").write("\n".join(parts))


main()
