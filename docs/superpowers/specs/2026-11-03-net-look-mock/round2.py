"""Net mockups, round 2, in B: inside a construct with no light rule, and a lane ride.

    python3 -I round2.py <shots dir> <out.html>

The shots dir holds construct_now.svg, construct_all_visible.svg and
construct_dump.json from the throwaway tmp_construct_look shot test (Kirosoft
Systems, security 255, grey ICE at the gate, black ICE at the vault door), and the
files gen.py reads.

1 · inside: the construct's body dark red (its security), rooms carved out of it and
    lit by what they hold, the vault amber, the spine a beam; every room and every
    piece of ICE seen from the gate, the spine each piece watches tinted.
2 · a lane ride: Shift+k on the lane by Kiroflux Mart, half way to the junction."""

import html
import json
import os
import sys

S, OUT = sys.argv[1], sys.argv[2]
sys.argv = [sys.argv[0], S, os.devnull]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen  # noqa: E402
from gen import (AMBER, CYAN, DIM, DOUBLE, LIGHT, MAGENTA, PANEL, RED, RULE, SIGNAL, TEXT,  # noqa: E402
                 WHITE, mix, scale, term)

K = json.load(open(os.path.join(S, "construct_dump.json")))
KW, KH = K["w"], K["h"]
KR = K["rows"]
OFF = (36, 10)  # cell = tile + OFF
YOU_IN = (4, 8)
VOID = "#010204"
GREY = "#c8d0dc"


def kt(x, y):
    return KR[y][x] if 0 <= x < KW and 0 <= y < KH else " "


def room_of(x, y):
    for i, (rx, ry, rw, rh) in enumerate(K["rooms"]):
        if rx <= x < rx + rw and ry <= y < ry + rh:
            return i
    return None


def open_tile(x, y):
    return kt(x, y) not in "W "


def edge(x, y):
    """A wall tile that faces a room, the spine or a door."""
    return kt(x, y) == "W" and any(open_tile(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1))


def border(x, y):
    return x in (0, KW - 1) or y in (0, KH - 1)


def watched():
    """Spine tiles each standing piece sees: its row or column, within its sight."""
    out = {}
    for g in K["guards"]:
        reach = 8 if g["grade"] == "black" else 6 if g["grade"] == "grey" else 4
        for dx in range(-reach, reach + 1):
            x = g["x"] + dx
            if kt(x, g["y"]) in "LGI" and (x, g["y"]) != (g["x"], g["y"]):
                # a clear line: nothing solid between
                lo, hi = sorted((x, g["x"]))
                if all(kt(i, g["y"]) != "W" for i in range(lo + 1, hi)):
                    out.setdefault((x, g["y"]), []).append(g["grade"])
    return out


def inside():
    g = gen.frame("construct_all_visible.svg")
    ox, oy = OFF
    # The void around the construct.
    for cy in range(5, 35):
        for cx in range(0, 79):
            x, y = cx - ox, cy - oy
            if 0 <= x < KW and 0 <= y < KH:
                continue
            lattice = x % 4 == 0 and y % 2 == 0
            g.c[cy][cx] = ["·" if lattice else " ", "#0d2a24", VOID, False]
    body = RED
    watch = watched()
    for y in range(KH):
        for x in range(KW):
            c = g.c[y + oy][x + ox]
            t = kt(x, y)
            r = room_of(x, y)
            if t == "W":
                if border(x, y):
                    m = gen.mask(x, y, lambda a, b: kt(a, b) in "WG" and border(a, b))
                    c[:] = [DOUBLE[m], body, mix(VOID, body, 0.10), False]
                elif edge(x, y):
                    m = gen.mask(x, y, lambda a, b: edge(a, b) or (kt(a, b) == "W" and border(a, b)))
                    c[:] = [LIGHT[m], scale(body, 0.75), mix(VOID, body, 0.10), False]
                else:
                    c[:] = ["░", scale(body, 0.5), mix(VOID, body, 0.07), False]
                continue
            if t == "L" or t == "I" and r is None:
                c[:] = [DOUBLE[gen.mask(x, y, lambda a, b: kt(a, b) in "LGI")], mix(SIGNAL, WHITE, 0.25),
                        mix(VOID, SIGNAL, 0.2), False]
            elif t == "G":
                c[:] = ["▯", WHITE, mix(VOID, SIGNAL, 0.25), True]
            else:
                vault = r == len(K["rooms"]) - 1 and any("V" in KR[yy] for yy in range(KH))
                tone = AMBER if vault else CYAN
                c[:] = [" ", TEXT, mix(VOID, tone, 0.16 if vault else 0.10), False]
                glyph = {"V": "⌗", "S": "◆", "T": "◇", "D": "▪", "C": "▪", "U": "▪", "N": "▪"}.get(t)
                if glyph:
                    c[0], c[1], c[3] = glyph, AMBER if t == "V" else WHITE, t in "VS"
            if (x, y) in watch:
                grades = watch[(x, y)]
                tint = RED if "black" in grades else MAGENTA
                c[2] = mix(c[2], tint, 0.45 if tint == RED else 0.3)
    for gd in K["guards"]:
        c = g.c[gd["y"] + oy][gd["x"] + ox]
        col = RED if gd["grade"] == "black" else MAGENTA
        c[0], c[1], c[3] = "◈", col, True
        c[2] = mix(VOID, col, 0.35)
    # Watch ends: a tick where each piece's sight stops.
    c = g.c[YOU_IN[1] + oy][YOU_IN[0] + ox]
    c[0], c[1], c[3] = "@", SIGNAL, True
    side_inside(g)
    return g


def side_inside(g):
    g.fill(80, 5, 40, 30, PANEL)
    g.put(81, 5, "KIROSOFT SYSTEMS", RED, bold=True)
    g.put(98, 5, "· corp tower", DIM)
    g.put(81, 6, "SEC", DIM)
    g.put(85, 6, "255", WHITE, bold=True)
    g.put(89, 6, "black ICE · vault deep", TEXT)
    g.put(81, 7, "◈", GREY, bold=True)
    g.put(83, 7, "grey ICE · gate · asleep · 6/6", MAGENTA)
    g.put(81, 8, "◈", RED, bold=True)
    g.put(83, 8, "black ICE · vault · asleep · 6/6", MAGENTA)
    g.put(81, 9, "TRACE", CYAN)
    g.put(87, 9, "▱" * 10 + " 0", CYAN)
    g.put(80, 10, "─" * 40, RULE)
    g.put(81, 11, "no job here", DIM)
    g.put(80, 12, "─" * 40, RULE)
    # The plan, a tile a half-block pixel, centred.
    w, h = 40, 10
    x0, y0 = 80, 13
    px0 = (w - KW) // 2
    py0 = (2 * h - KH) // 2
    watch = watched()

    def col(i, j):
        x, y = i - px0, j - py0
        if (x, y) == YOU_IN:
            return WHITE
        t = kt(x, y)
        if t == " ":
            return "#05070b"
        if t == "I":
            gd = next(gg for gg in K["guards"] if (gg["x"], gg["y"]) == (x, y))
            return RED if gd["grade"] == "black" else GREY
        if t == "W":
            return "#3a0a12" if not edge(x, y) else "#7a1626"
        if (x, y) in watch:
            return "#a02040" if "black" in watch[(x, y)] else "#6a7684"
        if t in "LG":
            return scale(SIGNAL, 0.8)
        if t == "V" or room_of(x, y) == len(K["rooms"]) - 1:
            return AMBER if t == "V" else scale(AMBER, 0.45)
        return "#e6f0f5" if t in "STDCUN" else scale(CYAN, 0.3)

    for j in range(h):
        for i in range(w):
            g.c[y0 + j][x0 + i] = ["▀", col(i, 2 * j), col(i, 2 * j + 1), False]
    g.put(81, 23, "Kirosoft Systems · construct", AMBER)
    g.put(80, 24, "─" * 40, RULE)
    g.put(81, 25, "22:04", DIM)
    g.put(87, 25, "in · Kirosoft Systems", TEXT)


# ---------------------------------------------------------------- the ride

RIDE_FROM = (199, 61)
RIDE_TO = (199, 53)
RIDE_AT = (199, 57)


def ride(g):
    x = RIDE_FROM[0]
    for y in range(RIDE_TO[1], RIDE_FROM[1] + 1):
        cx, cy = gen.to_cell(x, y)
        c = g.c[cy][cx]
        if y < RIDE_AT[1]:
            # Ahead: the stretch the ride will take, lit.
            c[1], c[3] = WHITE, True
            c[2] = mix(c[2], SIGNAL, 0.18)
        elif y > RIDE_AT[1]:
            # Behind: the streak, fading.
            f = max(0.0, 1 - (y - RIDE_AT[1] - 1) * 0.28)
            c[0], c[1], c[3] = "┃", mix(SIGNAL, WHITE, 0.8 * f), True
            c[2] = mix(c[2], WHITE, 0.5 * f)
    cx, cy = gen.to_cell(*RIDE_TO)
    c = g.c[cy][cx]
    c[1], c[3] = "#ffffff", True
    c[2] = mix(c[2], SIGNAL, 0.55)
    g.put(cx + 1, cy - 1, "◂ junction", WHITE, bg=None)


def lane_ride():
    gen.YOU = RIDE_AT
    g = gen.draw_b(ride)
    g.fill(80, 25, 40, 2, PANEL)
    g.put(81, 25, "22:01", DIM)
    g.put(87, 25, "riding · 8 tiles to the junction", TEXT)
    return g


# ---------------------------------------------------------------- page

def fig(g, title, lines):
    return ('<div class="opt"><div class="hd">%s</div>%s<ul>%s</ul></div>'
            % (html.escape(title), term.render(gen.shade(g), 10.5),
               "".join("<li>%s</li>" % html.escape(s) for s in lines)))


if __name__ == "__main__":
    now = gen.frame("construct_now.svg")
    one = inside()
    two = lane_ride()
    for k, g in (("inside", one), ("ride", two)):
        open(os.path.join(os.path.dirname(os.path.abspath(OUT)), "frame_%s.html" % k), "w").write(
            "<style>%s</style>%s" % (term.CSS, term.render(gen.shade(g), 14)))
    css = term.CSS + """
    .opt{border:1px solid #1d2733;border-radius:8px;padding:12px;margin:0 0 22px;background:#0b0f15}
    .opt .hd{font:600 16px system-ui;margin:0 0 8px;color:#e6edf3}
    .opt ul{margin:10px 0 0 18px;padding:0;color:#b8c2cc;font:13.5px/1.5 system-ui}
    """
    page = ("<style>%s</style><h2>The Net · round 2 · in B</h2>" % css
            + '<p class="subtitle">Inside a construct with no light rule, and riding a lane. Same frame and data as '
              "the game (seed 8).</p>"
            + fig(now, "Now · inside Kirosoft Systems from the gate, light rule on", [
                "You see the spine and nothing else; the wall mass is a grid of crossings.",
            ])
            + fig(one, "1 · Inside, in B · every room and every piece of ICE in view", [
                "The construct's body takes its security: dark red here (black ICE), green for guarded, and so on; "
                "rooms are carved out of it and lit by what they hold, cyan; the vault amber.",
                "You're in the first room, beside its door. The grey ICE at the gate and the black ICE at the vault "
                "door both stand on the spine; the stretch of spine each one watches is tinted (grey, red).",
                "With nothing hidden, the run becomes reading the patrols and picking when to cross the spine.",
                "The card reads the construct and its ICE; the minimap is the construct's plan.",
            ])
            + fig(two, "2 · A lane ride · Shift+direction", [
                "Shift+k on the lane by Kiroflux Mart: you run the lane to the next junction (or gate) in one go.",
                "Half way: the streak behind you, the stretch ahead lit, the stop marked.",
                "A ride stops at a junction, a gate, a node on the lane or the lane's end; the open grid stays a walk.",
            ]))
    open(OUT, "w").write(page)
    print("ok", OUT)
