"""The cord, on option B: the route of a rig run drawn on the map.

    python3 -I cord.py <shots dir> <out.html>

1 · on a rig run: you landed at your backdoor on Stack 101 Terminal and walked to
    Kiroflux Mart. ROUTE zenbo ▸ stack-101-terminal; the cord runs you → the relay →
    (a hop) → your rig's port.
2 · the trace walking home: it has reached the relay; the stretch behind it is
    magenta, the hop home still clean. TRACE ▸ stack-101-terminal · 9 min."""

import heapq
import html
import os
import sys

S, OUT = sys.argv[1], sys.argv[2]
sys.argv = [sys.argv[0], S, os.devnull]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen  # noqa: E402
from gen import (BLD, LIGHT, MAGENTA, PORT, YOU, CYAN, DIM, mix, scale, ter, to_cell,  # noqa: E402
                 in_view, term)

RELAY = tuple(BLD[95]["gate"])  # Stack 101 Terminal
HOME_GREEN = "#7cff9a"
CORD = "#e6f0f5"


def walk(a, b):
    """Lanes cheap, open grid dear."""
    best, prev, q = {a: 0}, {}, [(0, a)]
    while q:
        c, p = heapq.heappop(q)
        if p == b:
            break
        if c > best.get(p, 1e9):
            continue
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (p[0] + dx, p[1] + dy)
            if ter(*n) not in "LG.:":
                continue
            nc = c + (1 if ter(*n) in "LG" else 3)
            if nc < best.get(n, 1e9):
                best[n], prev[n] = nc, p
                heapq.heappush(q, (nc, n))
    path, p = [], b
    while p in prev:
        path.append(p)
        p = prev[p]
    path.append(a)
    return path[::-1]


PATH = walk(RELAY, YOU)


def hop_line(a, b):
    (ax, ay), (bx, by) = a, b
    n = max(abs(bx - ax), abs(by - ay))
    return [(round(ax + (bx - ax) * i / n), round(ay + (by - ay) * i / n)) for i in range(1, n)]


def draw_path(g, tiles, col, strength):
    on = set(tiles)
    for x, y in tiles:
        cx, cy = to_cell(x, y)
        if not in_view(cx, cy) or cy < 6:
            continue
        c = g.c[cy][cx]
        if ter(x, y) in "LG":
            c[1], c[3] = col, True
        else:
            m = gen.mask(x, y, lambda a, b: (a, b) in on)
            c[0], c[1], c[3] = LIGHT[m], col, True
        c[2] = mix(c[2], col, strength)


def mark(g, at, ch, col, bg=None):
    cx, cy = to_cell(*at)
    c = g.c[cy][cx]
    c[0], c[1], c[3] = ch, col, True
    if bg:
        c[2] = bg


def route_row(g, trace=None):
    """The row under the top rule, as the game draws it on a rig run."""
    g.fill(0, 5, 79, 1, gen.PANEL)
    g.put(0, 5, " ROUTE  ", DIM)
    g.put(8, 5, "zenbo ▸ stack-101-terminal", CYAN)
    if trace:
        g.put(78 - len(trace), 5, trace, MAGENTA, bold=True)


def calm(g):
    for x, y in hop_line(PORT, RELAY):
        cx, cy = to_cell(x, y)
        if in_view(cx, cy) and cy >= 6 and (x, y) not in gen.NODES:
            c = g.c[cy][cx]
            c[0], c[1], c[3] = "·", scale(CYAN, 0.9), True
    draw_path(g, PATH[:-1], CORD, 0.14)
    mark(g, PORT, "▣", HOME_GREEN)
    mark(g, RELAY, "▣", CYAN)
    route_row(g)


def traced(g):
    calm(g)
    draw_path(g, PATH[1:-1], MAGENTA, 0.22)
    mark(g, RELAY, "▣", "#ffffff", mix("#010204", MAGENTA, 0.75))
    route_row(g, "TRACE ▸ stack-101-terminal · 9 min")


def log(g, lines):
    for i, (t, s, col) in enumerate(lines):
        g.fill(80, 25 + i, 40, 1, gen.PANEL)
        g.put(81, 25 + i, t, DIM)
        g.put(87, 25 + i, s, col)


one = gen.draw_b(calm)
log(one, [("22:00", "landed · stack-101-terminal", gen.TEXT), ("22:03", "on the wire · Stack 101", gen.TEXT)])
two = gen.draw_b(traced)
log(two, [("22:00", "landed · stack-101-terminal", gen.TEXT), ("22:03", "on the wire · Stack 101", gen.TEXT),
          ("22:09", "honeypot · trace running", MAGENTA)])
for k, g in (("cord1", one), ("cord2", two)):
    open(os.path.join(os.path.dirname(os.path.abspath(OUT)), "frame_%s.html" % k), "w").write(
        "<style>%s</style>%s" % (term.CSS, term.render(gen.shade(g), 14)))


def fig(g, title, lines):
    return ('<div class="opt"><div class="hd">%s</div>%s<ul>%s</ul></div>'
            % (html.escape(title), term.render(gen.shade(g), 10.5),
               "".join("<li>%s</li>" % html.escape(s) for s in lines)))


css = term.CSS + """
.opt{border:1px solid #1d2733;border-radius:8px;padding:12px;margin:0 0 22px;background:#0b0f15}
.opt .hd{font:600 16px system-ui;margin:0 0 8px;color:#e6edf3}
.opt ul{margin:10px 0 0 18px;padding:0;color:#b8c2cc;font:13.5px/1.5 system-ui}
.key{color:#b8c2cc;font:14px/1.55 system-ui;max-width:760px}
.key b{color:#e6edf3}
"""
page = ("<style>%s</style><h2>The cord · an example on B</h2>" % css
        + '<p class="key">The game already keeps the <b>route</b> of a run from your rig: '
          "<code>ROUTE zenbo ▸ …</code> in the row above the map, one name per backdoor you came in "
          "through. And when you're dumped or land on a honeypot, a <b>trace</b> walks that route home, "
          "burning each backdoor on the way, then your door, then the raid. Today both are text. "
          "The cord draws them on the map.</p>"
        + fig(one, "1 · On a rig run", [
            "You landed at your backdoor on Stack 101 Terminal (▣ cyan, a relay) and walked to Kiroflux Mart.",
            "The cord is the white line from you back to the relay; the dotted line is the hop from there to "
            "your rig's port by your block (▣ green).",
            "From a street terminal there are no relays: the cord just leads back to the terminal your body "
            "stands at.",
        ])
        + fig(two, "2 · A trace walking home", [
            "A honeypot sprang: the trace starts at the relay nearest you and walks home.",
            "Magenta is where it has been; it sits on Stack 101 Terminal now, burns that backdoor in 9 minutes, "
            "then takes the hop to your door (20 more), then the raid.",
            "Catch: a dump already jacks you out, so most traces run while you're back in your body. You'd see "
            "this picture after a honeypot, or if you jack back in while one runs.",
        ]))
open(OUT, "w").write(page)
print("ok", OUT)
