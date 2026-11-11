"""Renders the site mocks as one phone-friendly page: each site's levels in the game's
glyphs and colours, each build's route through them, the links between floors.

    python3 -I gen.py <indoor_hotel shot dir> <out.html>
"""

import html
import json
import os
import sys
from collections import deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hotel  # noqa: E402
import metro  # noqa: E402

SHOTS = sys.argv[1]
OUT = sys.argv[2]

HEAVY = "━┃━┗┃┃┏┣━┛━┻┓┫┳╋"
LIGHT = "─│─└││┌├─┘─┴┐┤┬┼"

# Floor light by room kind: the dot's colour and the tile's ground (indoor lighting).
FLOOR = {
    "lobby": ("#2a364a", "#1a222e"),
    "corr": ("#26363a", "#141d1f"),
    "warm": ("#4a3a30", "#221915"),
    "bar": ("#3a2a46", "#241a2c"),
    "service": ("#263632", "#121a18"),
    "secure": ("#43282c", "#1b1114"),
    "park": ("#3b3221", "#18140c"),
    "yard": ("#262a30", "#0f1113"),
    "roof": ("#2a2e33", "#121518"),
    "pave": ("#2c3138", "#0e1013"),
    "road": ("#15171b", "#0b0c0e"),
    "drop": ("#000000", "#000000"),
    "plaza": ("#2c3138", "#0e1013"),
    "alley": ("#22262b", "#0a0c0e"),
    "hall": ("#2c3c4c", "#162029"),
    "subway": ("#2e383e", "#13191d"),
    "concourse": ("#23403c", "#0f1f1d"),
    "platform": ("#3d3a2e", "#16150e"),
    "track": ("#303a30", "#0d100e"),
    "power": ("#41391f", "#17130a"),
    "old": ("#3d3528", "#13100b"),
    "sump": ("#1e3a24", "#0a1c0c"),
    "sewer": ("#2a3a2a", "#0c130c"),
    "spine": ("#0d100e", "#0d100e"),
    "keel": ("#0d100e", "#0d100e"),
    None: ("#263632", "#121a18"),
}
WALL = ("#6e87a0", "#0a0b10")
VOID = ("#0c0f12", "#020304")
RAIL = "#8c7d5f"
LINE_COLOUR = {"spine": "#e0913a", "keel": "#2ab3a6"}

GLYPH = {
    "+": ("▯", "#dcebf5"), "K": ("▯", "#ff9a3c"), "G": ("▯", "#8aa0b4"),
    "L": ("≡", "#dcebf5"), "S": ("≡", "#ff9a3c"), "^": ("⇅", "#dcebf5"),
    "H": ("╫", "#dcebf5"), "v": ("☰", "#aab0ba"), "A": (" ", "#000000"),
    "c": ("◉", "#ff3c46"), "T": ("◆", "#ff6a2a"), "g": ("@", "#ff4d4d"),
    "x": ("■", "#d9a441"), "X": ("◈", "#ffe066"),
    "b": ("▤", "#84a2c0"), "d": ("▭", "#84a2c0"), "o": ("○", "#906ca8"),
    "s": ("·", "#906ca8"), "n": ("═", "#ddae34"), "$": ("═", "#ff5cc8"),
    "l": ("▮", "#8e6858"), "m": ("⚙", "#9c7260"), "t": ("▣", "#00e6d2"),
    "k": ("▮", "#00b3a4"), "h": ("▬", "#7f93b0"), "p": ('"', "#3fa45a"),
    "*": ("●", "#fff0c8"), "a": ("▩", "#7d8791"), "q": ("█", "#33506e"),
    "P": ("■", "#4c5d6f"), "Z": ("▆", "#3d5a3d"), "%": ("%", "#5a6470"),
    "~": ("~", "#2a5a78"), "u": ("≈", "#2aa8d8"), "j": ("▒", "#123c44"),
    "y": ("╪", RAIL), "Y": ("╫", RAIL), "J": ("┿", "#ff9a3c"),
    "E": ("↑", "#dcebf5"), "F": ("↓", "#dcebf5"), "V": ("▣", "#ff5cc8"),
    "i": ("@", "#8a94a6"), "W": ("≈", "#469646"), "Q": ("▒", "#5b5048"),
    "z": ("●", "#ff8a2a"),
}
BG_OVERRIDE = {
    "t": "#222224", "V": "#222224", "u": "#062030", "j": "#06181b", "A": "#000000",
    "E": "#1d2633", "F": "#1d2633", "W": "#0a1c0c",
}
BOLD = set("gXcTJ")

WALLISH = set("#=+KvGJ")
FENCEISH = set("fG")


def noise(x, y, salt):
    h = (x * 73856093) ^ (y * 19349663) ^ (salt * 83492791)
    return h & 0xFFFF


def shade(c, t):
    r, g, b = (int(c[i:i + 2], 16) for i in (1, 3, 5))
    return "#" + "".join(f"{max(0, min(255, round(v * t))):02x}" for v in (r, g, b))


def cells(lv, salt):
    out = []
    g = lv.g
    W, H = lv.w, lv.h
    for y in range(H):
        row = []
        for x in range(W):
            c = g[y][x]
            kind = lv.kind[y][x]
            fdot, fbg = FLOOR.get(kind, FLOOR[None])

            def at(dx, dy):
                xx, yy = x + dx, y + dy
                return g[yy][xx] if 0 <= xx < W and 0 <= yy < H else " "

            if c == " ":
                ch = "▓░▒ "[noise(x, y, salt) % 4]
                row.append([ch, VOID[0], VOID[1]])
            elif c in "#=":
                mask = (at(0, -1) in WALLISH) * 1 + (at(1, 0) in WALLISH) * 2 + (at(0, 1) in WALLISH) * 4 + (at(-1, 0) in WALLISH) * 8
                ch = "▫" if c == "=" else HEAVY[mask]
                fg = "#00acbf" if c == "=" else WALL[0]
                row.append([ch, fg, WALL[1]])
            elif c == "f":
                mask = (at(0, -1) in FENCEISH) * 1 + (at(1, 0) in FENCEISH) * 2 + (at(0, 1) in FENCEISH) * 4 + (at(-1, 0) in FENCEISH) * 8
                row.append([LIGHT[mask], "#4c5d6f", FLOOR["yard"][1]])
            elif c == "r":
                across = at(-1, 0) == "r" or at(1, 0) == "r"
                row.append(["─" if across else "│", WALL[0], fbg])
            elif c == ":":
                down = at(0, -1) == ":" or at(0, 1) == ":"
                row.append(["┆" if down else "┄", "#c9a227", fbg])
            elif c in "COD":
                line = LINE_COLOUR[kind]
                if c == "C":
                    row.append(["█", line, FLOOR["track"][1]])
                elif c == "O":
                    row.append(["▪", "#fff1d6", line])
                else:
                    row.append(["▯", "#fff1d6", shade(line, 0.45)])
            elif c in ".Rw":
                if c == "." or c == "R" and noise(x, y, salt + 7) % 3 == 0 or c == "w" and noise(x, y, salt + 9) % 2 == 0:
                    row.append(["·", fdot, fbg])
                else:
                    row.append([" ", fdot, fbg])
            elif c == "_":
                row.append([" ", fdot, fbg])
            else:
                ch, fg = GLYPH[c]
                bg = BG_OVERRIDE.get(c, fbg)
                row.append([ch, fg, bg] + ([1] if c in BOLD else []))
        out.append(row)
    return out


# ---- routes: waypoints joined by the shortest walk on each level

BASE = set(".+KG^LSHRwc~%*_yYJ:EFDW")


def passable(c, crawl):
    return c in BASE or (crawl and c == "v")


def walk(lv, a, b, crawl):
    """Shortest 4-way walk from a to b (or to beside b when b is solid)."""
    g = lv.g
    W, H = lv.w, lv.h
    goals = {b}
    if not passable(g[b[1]][b[0]], crawl):
        goals = {(b[0] + dx, b[1] + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))}
        goals = {p for p in goals if 0 <= p[0] < W and 0 <= p[1] < H and passable(g[p[1]][p[0]], crawl)}
    prev = {a: None}
    q = deque([a])
    while q:
        p = q.popleft()
        if p in goals:
            path = []
            while p is not None:
                path.append(p)
                p = prev[p]
            return path[::-1]
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (p[0] + dx, p[1] + dy)
            if 0 <= n[0] < W and 0 <= n[1] < H and n not in prev and passable(g[n[1]][n[0]], crawl):
                prev[n] = p
                q.append(n)
    raise SystemExit(f"no walk on {lv.key} from {a} to {b} ({g[a[1]][a[0]]!r} to {g[b[1]][b[0]]!r})")


def route_tiles(mod, r):
    by_key = {lv.key: lv for lv in mod.LEVELS}
    tiles = {lv.key: [] for lv in mod.LEVELS}
    marks = []
    n = 0
    prev = None
    for step in r["steps"]:
        k, x, y, note = step[:4]
        lv = by_key[k]
        if len(step) > 4 and step[4] == "ride":
            # A train: a straight run from where you boarded, no walk.
            (_, px, py) = prev
            sx = (x > px) - (x < px)
            sy = (y > py) - (y < py)
            p = (px, py)
            while p != (x, y):
                p = (p[0] + sx, p[1] + sy)
                tiles[k].append(p)
            end = (x, y)
        elif prev and prev[0] == k:
            path = walk(lv, (prev[1], prev[2]), (x, y), r["crawl"])
            tiles[k].extend(path)
            end = path[-1]
        else:
            end = (x, y)
            tiles[k].append(end)
        if note:
            n += 1
            marks.append(dict(level=k, x=end[0], y=end[1], n=n, note=note))
        prev = (k, end[0], end[1])
    return {k: sorted(set(v)) for k, v in tiles.items()}, marks


# ---- colour palette shared by every map on the page

PAL = []
PAL_IX = {}


def pal(c):
    if c not in PAL_IX:
        PAL_IX[c] = len(PAL)
        PAL.append(c)
    return PAL_IX[c]


def encode(grid):
    return dict(
        rows=["".join(c[0] for c in row) for row in grid],
        fg=[[pal(c[1]) for c in row] for row in grid],
        bg=[[pal(c[2]) for c in row] for row in grid],
        bold=["".join("1" if len(c) > 3 else "0" for c in row) for row in grid],
    )


# ---- legend, devices, links

LEGEND = [
    ("#", "┃", WALL[0], "wall"), ("=", "▫", "#00acbf", "window"), ("+", "▯", "#dcebf5", "door"),
    ("K", "▯", "#ff9a3c", "card door, on the Net"), ("G", "▯", "#8aa0b4", "gate"),
    ("^", "⇅", "#dcebf5", "stair or ramp"), ("L", "≡", "#dcebf5", "lift"),
    ("S", "≡", "#ff9a3c", "staff lift, needs a card"), ("H", "╫", "#dcebf5", "ladder"),
    ("E", "↑", "#dcebf5", "escalator (new)"), ("J", "┿", "#ff9a3c", "turnstile, on the Net (new)"),
    ("v", "☰", "#aab0ba", "grille, a crawl in"), ("r", "─", WALL[0], "railing"),
    (":", "┄", "#c9a227", "platform edge (new)"), ("y", "╪", RAIL, "track (╫ north-south)"),
    ("C", "█", LINE_COLOUR["spine"], "train (new)"),
    ("g", "@", "#ff4d4d", "guard"), ("i", "@", "#8a94a6", "passenger"),
    ("c", "◉", "#ff3c46", "camera"), ("T", "◆", "#ff6a2a", "turret (new)"),
    ("x", "■", "#d9a441", "container (new)"), ("X", "◈", "#ffe066", "objective (new)"),
    ("t", "▣", "#00e6d2", "terminal"), ("V", "▣", "#ff5cc8", "ad screen, on the Net"),
    ("k", "▮", "#00b3a4", "rack"), ("a", "▩", "#7d8791", "AC unit or transformer"),
    ("m", "⚙", "#9c7260", "machine"), ("u", "≈", "#2aa8d8", "pool"), ("W", "≈", "#469646", "sewage"),
    ("j", "▒", "#2a6e78", "skylight over the drop"), ("Q", "▒", "#5b5048", "rubble"),
    ("z", "●", "#ff8a2a", "fire barrel"),
]


def legend_items(mod):
    used = {c for lv in mod.LEVELS for row in lv.g for c in row}
    return "".join(
        f'<li><span class="g" style="color:{fg}">{html.escape(ch)}</span>{html.escape(t)}</li>'
        for key, ch, fg, t in LEGEND if key in used
    )


def devices(mod):
    def count(c, zone=None, not_zone=None):
        return sum(
            1 for lv in mod.LEVELS for y, row in enumerate(lv.g) for x, ch in enumerate(row)
            if ch == c and (zone is None or lv.zone[y][x] == zone)
            and (not_zone is None or lv.zone[y][x] != not_zone)
        )
    items = [
        ("◉", "#ff3c46", count("c"), "cameras"),
        ("◆", "#ff6a2a", count("T"), "turrets"),
        ("▯", "#ff9a3c", count("K"), "card doors"),
        ("┿", "#ff9a3c", count("J"), "turnstiles"),
        ("■", "#d9a441", count("x"), "containers"),
        ("@", "#ff4d4d", count("g", not_zone="gang"), "guards"),
        ("@", "#7bd14a", count("g", zone="gang"), "gangers"),
    ]
    return "".join(
        f'<li><span class="g" style="color:{fg}">{ch}</span><b>{n}</b> {html.escape(t)}</li>'
        for ch, fg, n, t in items if n
    )


def links_table(mod):
    labels = {lv.key: lv.label for lv in mod.LEVELS}
    head = "".join(f"<th>{html.escape(labels[k])}</th>" for k in mod.ORDER)
    rows = []
    for ch, fg, name, serves, note in mod.LINKS:
        cells_ = "".join(
            f'<td class="{"on" if k in serves else ""}">'
            + (f'<span style="color:{fg}">{ch}</span>' if k in serves else "·")
            + "</td>"
            for k in mod.ORDER
        )
        small = f"<small>{html.escape(note)}</small>" if note else ""
        rows.append(f"<tr><th scope=row>{html.escape(name)}{small}</th>{cells_}</tr>")
    return f'<table class="links"><thead><tr><th></th>{head}</tr></thead><tbody>{"".join(rows)}</tbody></table>'


def plain_list(items):
    return "".join(f"<li><b>{html.escape(a)}</b>{html.escape(b)}</li>" for a, b in items)


def line_map():
    """The network round Cinder Cross: the Spine east-west, the Keel north-south."""
    sp, kl = LINE_COLOUR["spine"], LINE_COLOUR["keel"]
    xs = [44, 108, 170, 232, 296]
    ys = [30, 98, 166]
    out = [
        '<svg class="linemap" viewBox="0 0 340 196" role="img" '
        'aria-label="The Spine line runs east to west through five stations; the Keel line runs north to south through three; they cross at Cinder Cross.">',
        f'<line x1="{xs[0]}" y1="98" x2="{xs[-1]}" y2="98" stroke="{sp}" stroke-width="5" stroke-linecap="round"/>',
        f'<line x1="170" y1="{ys[0]}" x2="170" y2="{ys[-1]}" stroke="{kl}" stroke-width="5" stroke-linecap="round"/>',
        f'<line x1="170" y1="8" x2="170" y2="{ys[0]}" stroke="{kl}" stroke-width="5" stroke-dasharray="3 4"/>',
        f'<line x1="170" y1="{ys[-1]}" x2="170" y2="188" stroke="{kl}" stroke-width="5" stroke-dasharray="3 4"/>',
        f'<line x1="{xs[-1]}" y1="98" x2="336" y2="98" stroke="{sp}" stroke-width="5" stroke-dasharray="3 4"/>',
        f'<line x1="4" y1="98" x2="{xs[0]}" y2="98" stroke="{sp}" stroke-width="5" stroke-dasharray="3 4"/>',
    ]
    for i, (x, name) in enumerate(zip(xs, metro.SPINE)):
        if name == metro.TITLE:
            continue
        out.append(f'<circle cx="{x}" cy="98" r="5" fill="#0a0b10" stroke="{sp}" stroke-width="3"/>')
        ty = 84 if i in (1, 4) else 118
        out.append(f'<text x="{x}" y="{ty}" text-anchor="middle">{html.escape(name)}</text>')
    for y, name in zip(ys, metro.KEEL):
        if name == metro.TITLE:
            continue
        out.append(f'<circle cx="170" cy="{y}" r="5" fill="#0a0b10" stroke="{kl}" stroke-width="3"/>')
        out.append(f'<text x="182" y="{y + 4}">{html.escape(name)}</text>')
    out.append('<circle cx="170" cy="98" r="9" fill="#0a0b10" stroke="#ffffff" stroke-width="3"/>')
    out.append(f'<text x="160" y="120" text-anchor="end" class="here">{html.escape(metro.TITLE)}</text>')
    out.append(f'<text x="4" y="190" fill="{sp}">SPINE · −2</text>')
    out.append(f'<text x="336" y="16" text-anchor="end" fill="{kl}">KEEL · −3</text>')
    out.append("</svg>")
    return "".join(out)


# ---- the sites

SITES = [
    dict(key="hotel", mod=hotel, mock="mock 1", layout="grid", today=True,
         empty="Pick SOLO, NETRUNNER, TECHIE or FIXER above. Each one gets in, reaches the safe and gets out its own way."),
    dict(key="metro", mod=metro, mock="mock 2", layout="stack", today=False,
         empty="Pick SOLO, NETRUNNER, TECHIE or FIXER above. Each one gets in, reaches Line Control and gets out its own way."),
]


def today_strip():
    rows = {}
    for line in open(os.path.join(SHOTS, "indoor_hotel.cells")):
        x, y, ch, fg, bg, bold = line.rstrip("\n").split("\t")
        rows[(int(x), int(y))] = (ch or " ", fg or "#cdd4de", bg or "#0a0b10")
    out = []
    for y in range(5, 35):
        row = []
        for x in range(28, 50):
            ch, fg, bg = rows.get((x, y), (" ", "#cdd4de", "#0a0b10"))
            if ch == "@":
                ch, fg = "·", "#2a364a"
            row.append([ch, fg, bg])
        out.append(row)
    return out


DATA = dict(order=[s["key"] for s in SITES], sites={})
for s in SITES:
    mod = s["mod"]
    by_key = {lv.key: lv for lv in mod.LEVELS}
    zone_codes = [z[0] for z in mod.ZONES]
    levels = []
    for i, k in enumerate(mod.ORDER):
        lv = by_key[k]
        used = [y for y in range(lv.h) if any(c != " " for c in lv.g[y])]
        y0, y1 = max(0, used[0] - 1), min(lv.h - 1, used[-1] + 1)
        enc = encode(cells(lv, i + 3 + len(s["key"]))[y0:y1 + 1])
        enc.update(key=k, label=lv.label, blurb=lv.blurb, y0=y0)
        enc["zone"] = [
            "".join(
                str(zone_codes.index(lv.zone[y][x]))
                if lv.g[y][x] not in "#=f " and lv.zone[y][x] in zone_codes else "."
                for x in range(lv.w)
            )
            for y in range(y0, y1 + 1)
        ]
        levels.append(enc)
    routes = []
    for r in mod.ROUTES:
        tiles, marks = route_tiles(mod, r)
        routes.append(dict(key=r["key"], name=r["name"], colour=r["colour"], line=r["line"], tiles=tiles, marks=marks))
        print(s["key"], r["name"], {k: len(v) for k, v in tiles.items()}, len(marks), "steps")
    DATA["sites"][s["key"]] = dict(
        w=mod.LEVELS[0].w, h=mod.LEVELS[0].h, levels=levels, routes=routes,
        zones=[z[1] for z in mod.ZONES],
        today=encode(today_strip()) if s["today"] else None,
    )
DATA["pal"] = PAL


def level_html(site, lv):
    k = site["key"]
    return (
        f'<article class="level" aria-label="{html.escape(lv.label)}">'
        f'<h2>{html.escape(lv.label)}<small>{html.escape(lv.blurb)}</small></h2>'
        f'<pre class="map" id="{k}-map-{lv.key}" data-cols="{lv.w}"></pre></article>'
    )


def site_html(site):
    mod = site["mod"]
    k = site["key"]
    by_key = {lv.key: lv for lv in mod.LEVELS}
    buttons = "".join(
        f'<button type="button" data-route="{r["key"]}" aria-pressed="false" style="--c:{r["colour"]}">{r["name"]}</button>'
        for r in mod.ROUTES
    )
    inputs = "".join(f"<li><span>{html.escape(a)}</span>{html.escape(b)}</li>" for a, b in mod.INPUTS)
    zones = "".join(f'<span style="--z:{c}">{html.escape(t)}</span>' for _, c, t in mod.ZONES)
    n = len(mod.ORDER)
    extra_aside = ""
    below = ""
    if k == "metro":
        extra_aside = (
            '<div class="card"><h3>On the line</h3>' + line_map()
            + '<p class="foot">The station stands where the city\'s Undercity rail (the Spine) meets a '
            "second line. Names are placeholders.</p></div>"
        )
        below = (
            '<section class="notes">'
            f'<div class="card"><h3>What makes it a mission</h3><ul class="plain traits">{plain_list(mod.TRAITS)}</ul></div>'
            f'<div class="card"><h3>What it sets up for the metro</h3><ul class="plain traits">{plain_list(mod.SYSTEM)}</ul></div>'
            "</section>"
            '<p class="foot">Drawn by hand to show what the generator should aim for at large size; nothing here '
            "is generated yet. Glyphs and colours are the game's own; the escalator, turnstile, platform edge, "
            "train, turret ◆, container ■ and objective ◈ are new and only proposed. The game draws track as "
            "╬ and ╫ in turn; here it runs ╪ east-west and ╫ north-south.</p>"
        )
    else:
        below = (
            '<section class="compare" aria-label="Today against the mock">'
            '<div class="today"><h3 class="eyebrow" style="margin:0;color:var(--dim)">A hotel today</h3>'
            f'<pre class="map" id="{k}-today" aria-label="A hotel floor as the game draws it today"></pre>'
            '<p class="foot">The Tessgen Inn, 2F, from the game. Every floor is this strip, stacked.</p></div>'
            f'<div class="card"><h3>What makes it a mission</h3><ul class="plain traits">{plain_list(mod.TRAITS)}</ul></div>'
            "</section>"
            '<p class="foot">Drawn by hand to show what the generator should aim for; nothing here is generated '
            "yet. Glyphs and colours are the game's own. The turret ◆, container ■ and objective ◈ are new and "
            "only proposed.</p>"
        )
    return f"""
<section class="site" id="site-{k}" data-site="{k}" hidden>
  <header>
    <div class="eyebrow">Site generator · {site["mock"]} · {html.escape(mod.SIZE)}</div>
    <h1>{html.escape(mod.TITLE)}</h1>
    <p class="lede">{html.escape(mod.LEDE)} Pick a build to see its route.</p>
    <ul class="inputs">{inputs}</ul>
  </header>
  <nav class="bar" aria-label="Routes and overlays">
    <button type="button" data-route="" aria-pressed="true">NO ROUTE</button>
    {buttons}
    <span class="sep" aria-hidden="true"></span>
    <button type="button" data-toggle="zones" aria-pressed="false">ZONES</button>
    <button type="button" data-toggle="zoom" aria-pressed="false">ZOOM</button>
  </nav>
  <div class="main">
    <section class="levels {site["layout"]}" aria-label="The {n} levels, top first">
      {"".join(level_html(site, by_key[key]) for key in mod.ORDER)}
    </section>
    <aside>
      <div class="card">
        <h3 class="route-title">Route</h3>
        <p class="empty route-empty">{html.escape(site["empty"])}</p>
        <p class="routeline" hidden></p>
        <ol class="steps" hidden></ol>
      </div>
      <div class="card zone-card" hidden><h3>Zones</h3><div class="zones">{zones}</div></div>
      {extra_aside}
      <div class="card"><h3>Between the levels</h3>{links_table(mod)}</div>
      <div class="card"><h3>On the site's Net, and guarding it</h3><ul class="plain devices">{devices(mod)}</ul></div>
      <div class="card"><h3>Key</h3><ul class="plain legend">{legend_items(mod)}</ul></div>
    </aside>
  </div>
  {below}
</section>"""


PAGE = r"""<title>Site Generator Mocks</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Fira+Code:wght@400;600;700&family=IBM+Plex+Sans:wght@400;600&display=swap">
<style>
/* One terminal-dark column on a phone; maps beside a sticky notes column on a wide
   screen. Single dark theme on purpose: it's the game's screen. */
:root {
  --bg: #07080b;
  --panel: #0a0b10;
  --line: #1b2130;
  --text: #cdd4de;
  --dim: #7d8796;
  --accent: #00e5ff;
  --mono: "Fira Code", "SFMono-Regular", Menlo, Consolas, monospace;
  --sans: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  color-scheme: dark;
}
* { box-sizing: border-box; }
body {
  background: var(--bg);
  color: var(--text);
  font: 15px/1.5 var(--sans);
  margin: 0;
  padding-left: max(16px, env(safe-area-inset-left, 0px));
  padding-right: max(16px, env(safe-area-inset-right, 0px));
  padding-block: 16px 48px;
}
body::before {
  content: ""; position: fixed; inset: 0 0 auto; height: env(safe-area-inset-top, 0px);
  z-index: 10; background: var(--bg);
}
.wrap { max-width: 1320px; margin: 0 auto; display: grid; gap: 18px; grid-template-columns: minmax(0, 1fr); }
[hidden] { display: none !important; }
.site { display: grid; gap: 20px; grid-template-columns: minmax(0, 1fr); }

.tabs { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.tabs button {
  text-align: left; cursor: pointer; color: var(--text); background: var(--panel);
  border: 1px solid var(--line); padding: 10px 12px; display: grid; gap: 3px; min-width: 0;
}
.tabs button b { font: 700 12px/1 var(--mono); letter-spacing: .14em; color: var(--accent); }
.tabs button span { font: 600 15px/1.2 var(--mono); overflow-wrap: anywhere; }
.tabs button small { font: 12px/1.3 var(--sans); color: var(--dim); }
.tabs button[aria-pressed="true"] { border-color: var(--accent); background: #0c1820; }
.tabs button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

header { display: grid; gap: 10px; }
.eyebrow { font: 600 11px/1 var(--mono); letter-spacing: .14em; color: var(--accent); text-transform: uppercase; }
h1 { font: 700 clamp(26px, 6vw, 40px)/1.05 var(--mono); margin: 0; letter-spacing: -.01em; text-wrap: balance; }
.lede { margin: 0; color: var(--dim); max-width: 66ch; }
.inputs { display: flex; flex-wrap: wrap; gap: 6px; margin: 4px 0 0; padding: 0; list-style: none; }
.inputs li { font: 12px/1 var(--mono); border: 1px solid var(--line); background: var(--panel); padding: 7px 9px; }
.inputs li span { color: var(--dim); margin-right: 6px; letter-spacing: .08em; }

.bar {
  position: sticky; top: env(safe-area-inset-top, 0px); z-index: 5;
  background: color-mix(in srgb, var(--bg) 92%, transparent);
  backdrop-filter: blur(6px);
  padding-block: 10px; border-bottom: 1px solid var(--line);
  display: flex; flex-wrap: wrap; gap: 6px; align-items: center;
}
.bar button {
  font: 600 12px/1 var(--mono); letter-spacing: .06em;
  color: var(--text); background: var(--panel); border: 1px solid var(--line);
  padding: 9px 11px; cursor: pointer;
}
.bar button[aria-pressed="true"] { color: #07080b; background: var(--c, var(--accent)); border-color: var(--c, var(--accent)); }
.bar button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.bar .sep { width: 1px; height: 22px; background: var(--line); margin: 0 4px; }

.main { display: grid; gap: 20px; grid-template-columns: minmax(0, 1fr); }
@media (min-width: 1000px) { .main { grid-template-columns: minmax(0, 1fr) 340px; align-items: start; } }
.levels { display: grid; gap: 14px; grid-template-columns: minmax(0, 1fr); }
@media (min-width: 720px) { .levels.grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
.level { background: var(--panel); border: 1px solid var(--line); padding: 10px; display: grid; gap: 8px; min-width: 0; }
.level h2 { margin: 0; display: flex; gap: 10px; align-items: baseline; font: 700 15px/1.2 var(--mono); }
.level h2 small { font: 400 12px/1.3 var(--sans); color: var(--dim); }
.map { margin: 0; font-family: var(--mono); line-height: 1.22; white-space: pre; overflow: hidden; font-variant-ligatures: none; }
.map.zoomed { overflow-x: auto; -webkit-overflow-scrolling: touch; overscroll-behavior-x: contain; }
.map span.b { font-weight: 700; }
.map span.mk { font-weight: 700; color: #07080b !important; }

aside { display: grid; gap: 16px; min-width: 0; }
@media (min-width: 1000px) { aside { position: sticky; top: calc(env(safe-area-inset-top, 0px) + 64px); max-height: calc(100vh - 80px); overflow: auto; } }
.card { background: var(--panel); border: 1px solid var(--line); padding: 14px; display: grid; gap: 10px; min-width: 0; }
.card h3 { margin: 0; font: 600 11px/1 var(--mono); letter-spacing: .14em; text-transform: uppercase; color: var(--dim); }
.routeline { margin: 0; }
ol.steps { margin: 0; padding: 0; list-style: none; display: grid; gap: 8px; }
ol.steps li { display: grid; grid-template-columns: 22px 30px minmax(0, 1fr); gap: 8px; align-items: baseline; font-size: 14px; }
ol.steps .n { font: 700 12px/20px var(--mono); text-align: center; color: #07080b; background: var(--c); }
ol.steps .lv { font: 600 11px/20px var(--mono); color: var(--dim); }
.empty { color: var(--dim); margin: 0; }

ul.plain { margin: 0; padding: 0; list-style: none; display: grid; gap: 6px; }
ul.legend { grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); font-size: 13px; }
ul.legend li, ul.devices li { display: flex; gap: 8px; align-items: baseline; }
.g { font: 700 15px/1 var(--mono); width: 14px; display: inline-block; text-align: center; flex: none; }
ul.devices { grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); font-size: 13px; }
ul.devices b { font: 700 14px var(--mono); }
.zones { display: flex; flex-wrap: wrap; gap: 6px 14px; font-size: 13px; }
.zones span::before { content: ""; display: inline-block; width: 10px; height: 10px; margin-right: 6px; background: var(--z); }

table.links { border-collapse: collapse; width: 100%; font-size: 13px; }
table.links th, table.links td { padding: 5px 3px; border-bottom: 1px solid var(--line); }
table.links thead th { font: 600 11px var(--mono); color: var(--dim); text-align: center; }
table.links tbody th { text-align: left; font-weight: 600; }
table.links tbody th small { display: block; font-weight: 400; color: var(--dim); }
table.links td { text-align: center; font: 700 15px var(--mono); color: #2a313d; }
table.links td.on { background: #10141c; }

svg.linemap { width: 100%; height: auto; display: block; }
svg.linemap text { font: 10px var(--mono); fill: var(--text); }
svg.linemap text.here { font-weight: 700; fill: #ffffff; }

.compare { display: grid; gap: 14px; grid-template-columns: minmax(0, 1fr); align-items: start; }
@media (min-width: 720px) { .compare { grid-template-columns: 220px minmax(0, 1fr); } }
.notes { display: grid; gap: 14px; grid-template-columns: minmax(0, 1fr); align-items: start; }
@media (min-width: 720px) { .notes { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
.today { background: var(--panel); border: 1px solid var(--line); padding: 10px; display: grid; gap: 8px; min-width: 0; }
.today .map { max-width: 220px; }
ul.traits { display: grid; gap: 10px; }
ul.traits li { display: grid; gap: 2px; }
ul.traits b { font: 600 13px var(--mono); color: var(--accent); }
.foot { color: var(--dim); font-size: 13px; margin: 0; max-width: 72ch; }
</style>

<div class="wrap">
  <nav class="tabs" aria-label="Sites">__TABS__</nav>
  __SITES__
</div>

<script>
const DATA = __DATA__;
const PAL = DATA.pal;
const grids = {};
const state = {};
DATA.order.forEach(k => { state[k] = { route: null, zones: false, zoom: false }; });
let current = null;

function hex(c) { return [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16)); }
function mix(a, b, t) {
  const x = hex(a), y = hex(b);
  return "#" + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, "0")).join("");
}
function mark(n) { return n > 9 ? String.fromCharCode(55 + n) : String(n); }

function draw(pre, enc) {
  const frag = document.createDocumentFragment();
  const grid = [];
  enc.rows.forEach((row, y) => {
    const line = [];
    for (let x = 0; x < row.length; x++) {
      const s = document.createElement("span");
      const base = { ch: row[x], fg: PAL[enc.fg[y][x]], bg: PAL[enc.bg[y][x]], bold: enc.bold[y][x] === "1" };
      s.textContent = base.ch;
      s.style.color = base.fg;
      s.style.background = base.bg;
      if (base.bold) s.className = "b";
      s._base = base;
      frag.appendChild(s);
      line.push(s);
    }
    frag.appendChild(document.createTextNode("\n"));
    grid.push(line);
  });
  pre.appendChild(frag);
  return grid;
}

function build(k) {
  if (grids[k]) return;
  const site = DATA.sites[k];
  grids[k] = {};
  site.levels.forEach(lv => { grids[k][lv.key] = draw(document.getElementById(`${k}-map-${lv.key}`), lv); });
  if (site.today) {
    const pre = document.getElementById(`${k}-today`);
    draw(pre, site.today);
    pre.dataset.cols = site.today.rows[0].length;
  }
}

let ratio = 0;
function fit() {
  if (!current) return;
  if (!ratio) {
    const probe = document.createElement("span");
    probe.style.cssText = "position:absolute;visibility:hidden;font:100px var(--mono);white-space:pre";
    probe.textContent = "0".repeat(10);
    document.body.appendChild(probe);
    ratio = probe.getBoundingClientRect().width / 1000;
    probe.remove();
  }
  const zoom = state[current].zoom;
  document.querySelectorAll(`#site-${current} pre.map`).forEach(pre => {
    const today = pre.id.endsWith("-today");
    const cols = Number(pre.dataset.cols);
    const width = pre.parentElement.clientWidth - (today ? 0 : 20);
    let size = Math.min(22, Math.floor((Math.min(width, today ? 200 : width) / (cols * ratio)) * 100) / 100);
    const zoomed = zoom && !today && size < 13;
    if (zoomed) size = 13;
    pre.classList.toggle("zoomed", zoomed);
    pre.style.fontSize = size + "px";
  });
}

function paint() {
  const k = current, site = DATA.sites[k], st = state[k];
  const sec = document.getElementById(`site-${k}`);
  const r = site.routes.find(r => r.key === st.route);
  site.levels.forEach(lv => {
    const grid = grids[k][lv.key];
    for (let y = 0; y < lv.rows.length; y++) for (let x = 0; x < site.w; x++) {
      const s = grid[y][x], b = s._base;
      s.textContent = b.ch; s.style.color = b.fg; s.style.background = b.bg;
      s.className = b.bold ? "b" : "";
      const z = lv.zone[y][x];
      if (st.zones && z !== ".") s.style.background = mix(b.bg, site.zones[Number(z)], 0.32);
    }
    if (!r) return;
    (r.tiles[lv.key] || []).forEach(([x, y]) => {
      const s = grid[y - lv.y0][x], b = s._base;
      s.style.background = mix(b.bg, r.colour, 0.42);
      if (b.ch === "·" || b.ch === " ") { s.textContent = "·"; s.style.color = r.colour; }
    });
    r.marks.filter(m => m.level === lv.key).forEach(m => {
      const s = grid[m.y - lv.y0][m.x];
      s.textContent = mark(m.n);
      s.style.background = r.colour;
      s.className = "mk";
    });
  });
  const steps = sec.querySelector("ol.steps");
  const line = sec.querySelector(".routeline");
  sec.querySelector(".route-empty").hidden = !!r;
  line.hidden = steps.hidden = !r;
  sec.querySelector(".route-title").textContent = r ? r.name : "Route";
  steps.innerHTML = "";
  if (r) {
    line.textContent = r.line;
    const names = Object.fromEntries(site.levels.map(l => [l.key, l.label]));
    r.marks.forEach(m => {
      const li = document.createElement("li");
      li.style.setProperty("--c", r.colour);
      li.innerHTML = `<span class="n">${mark(m.n)}</span><span class="lv"></span><span></span>`;
      li.children[1].textContent = names[m.level];
      li.lastChild.textContent = m.note;
      steps.appendChild(li);
    });
  }
  sec.querySelector(".zone-card").hidden = !st.zones;
  sec.querySelectorAll("button[data-route]").forEach(b => b.setAttribute("aria-pressed", String((b.dataset.route || null) === st.route)));
  sec.querySelector('button[data-toggle="zones"]').setAttribute("aria-pressed", String(st.zones));
  sec.querySelector('button[data-toggle="zoom"]').setAttribute("aria-pressed", String(st.zoom));
}

function remember() {
  const st = state[current];
  const hash = "#" + current + (st.route ? "-" + st.route : "");
  try { history.replaceState(null, "", hash); } catch (e) {}
}

function show(k, route) {
  current = k;
  DATA.order.forEach(o => { document.getElementById(`site-${o}`).hidden = o !== k; });
  document.querySelectorAll(".tabs button").forEach(b => b.setAttribute("aria-pressed", String(b.dataset.site === k)));
  build(k);
  if (route !== undefined) state[k].route = DATA.sites[k].routes.some(r => r.key === route) ? route : null;
  fit();
  paint();
}

document.querySelectorAll(".tabs button").forEach(btn => {
  btn.addEventListener("click", () => { show(btn.dataset.site); remember(); scrollTo({ top: 0 }); });
});
document.querySelectorAll(".site").forEach(sec => {
  const k = sec.dataset.site;
  sec.querySelectorAll("button[data-route]").forEach(btn => {
    btn.addEventListener("click", () => { state[k].route = btn.dataset.route || null; paint(); remember(); });
  });
  sec.querySelector('button[data-toggle="zones"]').addEventListener("click", () => { state[k].zones = !state[k].zones; paint(); });
  sec.querySelector('button[data-toggle="zoom"]').addEventListener("click", () => { state[k].zoom = !state[k].zoom; fit(); paint(); });
});

function fromHash() {
  const h = location.hash.slice(1);
  const [k, r] = h.split("-");
  if (DATA.order.includes(k)) return show(k, r || null);
  // Links from the first version: #solo, #net, #tech, #fixer meant the hotel.
  if (DATA.sites.hotel.routes.some(x => x.key === h)) return show("hotel", h);
  show(DATA.order[0], null);
}
fromHash();
addEventListener("hashchange", fromHash);
addEventListener("resize", fit);
if (document.fonts && document.fonts.ready) document.fonts.ready.then(fit);
</script>
"""


def tab_html(site):
    mod = site["mod"]
    n = len(mod.ORDER)
    return (
        f'<button type="button" data-site="{site["key"]}" aria-pressed="false">'
        f'<b>{html.escape(mod.TAB)}</b><span>{html.escape(mod.TITLE)}</span>'
        f"<small>{html.escape(mod.SIZE)} · {n} levels</small></button>"
    )


page = (
    PAGE.replace("__TABS__", "".join(tab_html(s) for s in SITES))
    .replace("__SITES__", "".join(site_html(s) for s in SITES))
    .replace("__DATA__", json.dumps(DATA, separators=(",", ":"), ensure_ascii=False))
)
open(OUT, "w").write(page)
print("wrote", OUT, len(page.encode()) // 1024, "KB,", len(PAL), "colours")
