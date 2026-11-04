"""Overworld mockups, round 3: the picks from rounds 1 and 2 on one page, plus the deck's
MAP zoomed out a step further than today, to the world, drawn as round 1's survey chart.

    python3 -I ow3.py <out.html>

1 · the overworld on foot (round 1's A) and jacked in (round 1's D).
2 · one level down at Kilo-9: the sparse ground with a little relief (T), and its Net
    with the fading grid (round 2's B).
3 · where Saintburg ends, grown, on the street and in the Net.
4 · the deck's MAP: zoom 3 as it is (the whole city, the game's own frame from the
    throwaway tmp_overworld_deck_map shot test) and zoom 4, the world."""

import html
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "round3.html")
sys.argv = [sys.argv[0], os.devnull]
sys.path.insert(0, HERE)
import ow  # noqa: E402
import ow2  # noqa: E402
from ow import Grid, mix, term  # noqa: E402

# Seed 8's city, so the real frames and the world agree on its name.
ow.CITIES[0]["name"] = "Saintburg"
HOME = "Saintburg"

DECK = Grid.from_svg(os.path.join(HERE, "shots", "deck_map_fit_120x40.svg"), 120, 40)


def overworld_frames():
    near = ow.near_list()
    foot = ("%d,%d · the world" % ow.YOU, "↵ down")
    a = ow.frame(["Dust Flats · Route 9", "open ground · clear night",
                  "Route 9 · 2 tiles out of %s" % HOME], near, foot)
    ow.draw_atlas(a)
    d = ow.frame(["Dust Flats · Net", "dead grid · 2 nodes in reach",
                  "Route 9 trunk · %s to Ashgate" % HOME], near, foot,
                 mode="jacked in · overworld")
    ow.draw_net(d)
    return a, d


# ---- the deck's MAP at zoom 4: the world as a survey chart

AX, AY, AW, AH = 1, 11, 81, 26  # the MAP's map area in the deck
KINDS = [("◉", "#e6f0f5"), ("⌂", ow.AMBER), ("╪", ow.RED), ("⬡", ow.SIGNAL),
         ("△", "#ff8040"), ("×", ow.RED), ("▣", "#a890d8"), ("▦", "#7ab0e0"),
         ("◎", ow.CYAN), ("◈", ow.MAGENTA), ("◆", "#c8a050")]


def deck_world():
    g = Grid(120, 40)
    g.c = [[list(c) for c in row] for row in DECK.c]
    home = ow.CITIES[0]["c"]
    you = (home[0] + 5, home[1] - 1)
    # point the chart at the deck's map area, centred on you
    ow.MX, ow.MY, ow.MW, ow.MH = AX, AY, AW, AH
    ow.VX, ow.VY = you[0] - AW // 2, you[1] - AH
    ow.ROAD_E = ow.edges([p for _, p in ow.ROUTES], ow.cell_of)
    ow.RIVER_E = ow.edges([ow.RIVER_PATH], ow.cell_of)
    ow.POI_AT = {ow.cell_of(p[3]): p for p in ow.POIS}
    ow.YOU_CELL = ow.cell_of(you)
    g.fill(AX, AY, AW, AH, ow.BG)
    ow.draw_chart(g)
    # the cursor on Ashgate, and its card
    ash = next(c for c in ow.CITIES if c["name"] == "Ashgate")
    cx, cy = ow.cell_of(ash["c"])
    cx, cy = AX + cx, AY + cy
    for dx, ch in ((-1, "["), (1, "]")):
        if AX <= cx + dx < AX + AW:
            g.c[cy][cx + dx][0], g.c[cy][cx + dx][1], g.c[cy][cx + dx][3] = ch, ow.CYAN, True
    km = math.hypot(ash["c"][0] - you[0], ash["c"][1] - you[1]) * 20 / 1000
    lines = [("◉ Ashgate", "#e6f0f5", True), ("city · 4 districts", "#c8d0dc", False),
             ("up Route 9 · %.1f km" % km, "#c8d0dc", False),
             ("⌖ t track", ow.CYAN, False)]
    w = 26
    bx, by = cx - w - 3, cy + 2
    border = DECK.c[16][34][1]
    card_bg = DECK.c[17][40][2]
    g.fill(bx, by, w + 2, len(lines) + 2, card_bg)
    g.put(bx, by, "┌" + "─" * w + "┐", border, bg=card_bg)
    for i, (text, col, bold) in enumerate(lines):
        g.put(bx, by + 1 + i, "│", border, bg=card_bg)
        g.put(bx + 1, by + 1 + i, text.ljust(w), col, bg=card_bg, bold=bold)
        g.put(bx + w + 1, by + 1 + i, "│", border, bg=card_bg)
    g.put(bx, by + 1 + len(lines), "└" + "─" * w + "┘", border, bg=card_bg)
    # the title
    title_fg, dim_fg = DECK.c[10][2][1], DECK.c[10][22][1]
    g.fill(1, 10, 80, 1, DECK.c[10][1][2])
    g.put(2, 10, "WORLD", title_fg, bold=True)
    g.put(7, 10, " · zoom 4", dim_fg)
    # the places pane, listing the world
    pane_bg = DECK.c[14][90][2]
    name_fg, dist_fg = DECK.c[14][86][1], DECK.c[14][115][1]
    sel_bg = DECK.c[32][90][2]
    g.fill(83, 11, 36, 25, pane_bg)
    x = 84
    for gl, col in KINDS:
        g.put(x, 11, gl, col, bold=True)
        x += 2
    g.put(x, 11, "all", ow.CYAN, bold=True)
    g.put(83, 12, "─" * 34, DECK.c[12][84][1])
    places = [(c["name"], "◉", "#e6f0f5", c["c"], "city") for c in ow.CITIES]
    places += [(name, gl, col, p, note) for gl, col, name, p, note in ow.POIS]
    rows = []
    for name, gl, col, p, note in places:
        d = math.hypot(p[0] - you[0], p[1] - you[1]) * 20 / 1000
        rows.append((d, name, gl, col, note))
    rows.sort()
    y = 13
    for d, name, gl, col, note in rows[:22]:
        here = name == HOME
        sel = name == "Ashgate"
        bg = sel_bg if sel else pane_bg
        if sel:
            g.fill(83, y, 35, 3, sel_bg)
        g.put(84, y, gl, col, bg=bg, bold=True)
        g.put(86, y, name, "#ffffff" if sel else name_fg, bg=bg, bold=sel)
        dist = "here" if here else "%.1f km" % d
        g.put(118 - len(dist), y, dist, dist_fg, bg=bg)
        y += 1
        if sel:
            g.put(86, y, "city · up Route 9", name_fg, bg=bg)
            g.put(86, y + 1, "⌖ tracking · t stops" if False else "⌖ t track", ow.CYAN, bg=bg)
            y += 2
        if y > 34:
            break
    g.fill(83, 35, 35, 1, pane_bg)
    return g


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
    a, d = overworld_frames()
    secs = [
        ("1 · The overworld", "One cell a tile. Step onto any tile and ↵ takes you down "
         "onto its detail map; jack in and the same world shows its Net.",
         [("On foot · round 1's A", a,
           "The atlas: the ground as glyphs, highways as double lines, the river a thin "
           "line, cities as shaded blocks in their districts' colours."),
          ("Jacked in · round 1's D", d,
           "The same world in the Net: cities as node fields, the backbones down the "
           "highways with packets on them, dead grid between, a few lone nodes on spurs. "
           "↵ goes down onto that tile's Net.")]),
        ("2 · One level down", "Kilo-9 truck stop on Route 9 at 22:00.",
         [("On foot · sparse ground with a little relief", ow2.kilo_frame("T"),
           "Round 2's A with a touch of B: the rises a shade lighter with a few "
           "<code>░</code>, the wash's bed flecked the same way. Still quiet; the stop "
           "and the road carry the screen."),
          ("Jacked in · the fading grid", ow2.net_kilo("B"),
           "Round 2's B as it was: the grid shows near signal and fades to void a few "
           "tiles out.")]),
        ("3 · Where %s ends" % HOME, "",
         [("On foot", ow2.edge_frame("T"),
           "The towers stop where the city's shape says; a lot and a body shop, a fenced "
           "scrapyard, shanties round fire barrels, the last lamp, a billboard; the avenue "
           "runs on as Route 9."),
          ("Jacked in", ow2.net_edge(),
           "The district's tint and the grid fade past the last constructs; the avenue's "
           "lane runs on as Route 9's trunk; a stolen line off it to the shanties.")]),
        ("4 · The deck's MAP, one zoom further", "Today MAP's furthest zoom (3) fits the "
         "whole city. Zoom 4 shows the world as the survey chart, the cursor and the places "
         "pane working as they do on the city.",
         [("Zoom 3 · as it is", ow2.copy(DECK),
           "The game's own frame: the whole of %s." % HOME),
          ("Zoom 4 · the world", deck_world(),
           "The survey chart round you: contour dots, the coast hatched, cities as tinted "
           "ground with their names, highways with route shields, every POI and region "
           "named. The cursor sits on Ashgate with its card; PLACES lists the world's "
           "cities and POIs by distance (a tile taken as 20 m, the street's metre a tile; "
           "the real scale is still open). <code>+</code> goes back to the city.")]),
    ]
    out = ["<!doctype html><meta charset=utf-8><title>Overworld · round 3</title>",
           "<style>%s</style>" % CSS,
           "<h1>The overworld · round 3: the picks together</h1>",
           "<p>Rounds 1 and 2 as chosen: the atlas overworld and its Net, the ground one "
           "level down (sparse, a little relief), the fading grid, the city's outskirts, "
           "and the survey chart as the deck MAP's world zoom. Building waits for save "
           "the run (#2): the world, and every tile you have been down on, is saved with "
           "the rest of the run.</p>"]
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
                open(os.path.join(chk, "r3_%d.html" % n), "w").write(
                    "<!doctype html><meta charset=utf-8><style>%s body{margin:0;"
                    "background:#000}</style>%s" % (term.CSS, term.render(g, 14.0)))
    open(OUT, "w").write("\n".join(out))
    print(OUT)


if __name__ == "__main__":
    main()
