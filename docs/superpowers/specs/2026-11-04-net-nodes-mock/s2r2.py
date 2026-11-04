"""Slice 2 mockups, round 2: the danger outside a construct, and the nodes at work.

    python3 -I s2r2.py <shots dir> <out.html>

Drawn on the game's frames (seed 8) with the round-1 picks: a held node keeps a dock
(B), a tapped camera's feed hangs off it on a wire (A). You are blue.

Danger outside: A tracers follow your trail, B the hub sweeps, C sentinels patrol.
Nodes: a camera on a construct's wall from outside, the lamp grid on the open Net,
an ad hijack (the picker, then the street), camera eyes and a loop on the street, a
decoy inside, the shell from the Net."""

import heapq
import html
import os
import sys

S, OUT = sys.argv[1], sys.argv[2]
sys.argv = [sys.argv[0], S, os.devnull]
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-03-net-look-mock"))
import gen  # noqa: E402
import round2  # noqa: E402
import s2  # noqa: E402
from gen import AMBER, CYAN, DIM, FAINT, MAGENTA, PANEL, RULE, SIGNAL, TEXT, WHITE, mix, scale, term  # noqa: E402

BLUE = "#468cff"
BLUE_GLOW = "#0e2254"
RED = "#ff2848"
GREY_ICE = "#ff00c8"  # grey ICE is magenta in the game
DOCK_BG = "#04120e"
s2.STREET = s2.street()


def you_blue(g, at):
    cx, cy = gen.to_cell(*at)
    g.c[cy][cx] = ["@", BLUE, BLUE_GLOW, True]


def footer_trace(g, place, bar, heat, who, extra="", colour=CYAN):
    g.fill(0, 36, 120, 1, PANEL)
    g.put(1, 36, "⚡", CYAN)
    g.put(4, 36, place, TEXT)
    g.put(46, 36, "TRACE", colour)
    g.put(52, 36, "▰" * bar + "▱" * (10 - bar), colour)
    g.put(63, 36, "%d" % heat, colour, bold=heat >= 60)
    g.put(67, 36, "· " + who, colour)
    if extra:
        g.put(69 + len(who) + 1, 36, extra, MAGENTA, bold=True)
    g.put(117, 36, "[?]", DIM)


def card(g, lines):
    """HERE's top rows on the Net: (text, colour, bold) per row, then the rule."""
    g.fill(80, 5, 40, len(lines) + 1, PANEL)
    for i, row in enumerate(lines):
        x = 81
        for text, colour, bold in row:
            g.put(x, 5 + i, text, colour, bold=bold)
            x += len(text)
    g.put(80, 5 + len(lines), "─" * 40, RULE)


def log(g, rows):
    g.fill(80, 25, 40, 4, PANEL)
    for i, (t, text, colour) in enumerate(rows):
        g.put(81, 25 + i, t, DIM)
        g.put(87, 25 + i, text, colour)


def dock(g, node, rows, label_at, wire=SIGNAL):
    """A held node's dock: two rows of label at `label_at` (cells), a wire to `node`
    (cells) along the label's second row, then up or down to the node."""
    lx, ly = label_at
    for i, row in enumerate(rows):
        x = lx
        for text, colour, bold in row:
            g.put(x, ly + i, text, colour, DOCK_BG, bold=bold)
            x += len(text)
    width = max(sum(len(t) for t, _, _ in row) for row in rows)
    nx, ny = node
    wy = ly + 1
    x0, x1 = (lx + width, nx) if nx > lx else (nx + 1, lx - 1)
    for x in range(x0, x1):
        g.c[wy][x][0], g.c[wy][x][1] = "─", wire
    if ny != wy:
        step = 1 if ny > wy else -1
        for y in range(wy + step, ny, step):
            g.c[y][nx][0], g.c[y][nx][1] = "│", wire
        g.c[wy][nx][0], g.c[wy][nx][1] = ("╮" if step > 0 else "╯") if nx > lx else ("╭" if step > 0 else "╰"), wire
    c = g.c[ny][nx]
    c[1], c[3] = "#ffffff", True
    c[2] = mix(c[2], wire, 0.55)


def walk(a, b):
    best, prev, q = {a: 0}, {}, [(0, a)]
    while q:
        c, p = heapq.heappop(q)
        if p == b:
            break
        if c > best.get(p, 1e9):
            continue
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (p[0] + dx, p[1] + dy)
            if gen.ter(*n) not in "LG.:":
                continue
            nc = c + (1 if gen.ter(*n) in "LG" else 3)
            if nc < best.get(n, 1e9):
                best[n], prev[n] = nc, p
                heapq.heappush(q, (nc, n))
    path, p = [], b
    while p in prev:
        path.append(p)
        p = prev[p]
    path.append(a)
    return path[::-1]


def open_net(you, extra=None, ox=150, oy=46):
    """Option B's open Net round `you`, `extra` drawn over it before you."""
    gen.OX, gen.OY = ox, oy
    gen.YOU = you
    g = gen.draw_b(extra or (lambda g: None))
    you_blue(g, you)
    gen.OX, gen.OY = 150, 46
    return g


# ---------------------------------------------------------------- danger outside

HACKED = (204, 62)  # where you stood when you cracked Kiroflux Mart's camera
FLED = (199, 53)    # where you are now, up the lane
TRAIL = walk(HACKED, FLED)


def trail(g, upto, colour, f):
    """Your trail from the tracer to you: its lanes tinted and drawn in its colour."""
    for x, y in TRAIL[upto:-1]:
        cx, cy = gen.to_cell(x, y)
        c = g.c[cy][cx]
        c[2] = mix(c[2], colour, f)
        c[1] = mix(c[1], colour, 0.7)
        if gen.ter(x, y) not in "LG":
            c[0] = "·"


def danger_a():
    at = len(TRAIL) * 2 // 5

    def extra(g):
        trail(g, at, GREY_ICE, 0.32)
        cx, cy = gen.to_cell(*TRAIL[at])
        g.c[cy][cx] = ["◈", GREY_ICE, mix("#010204", GREY_ICE, 0.4), True]
        g.put(cx + 2, cy, "tracer", GREY_ICE, bold=True)

    g = open_net(FLED, extra)
    card(g, [[("KIROFLUX MART", AMBER, True), (" · shop ↓", DIM, False)],
             [("SEC ", DIM, False), ("114", WHITE, True), (" · guarded", TEXT, False)],
             [("◈ tracer", GREY_ICE, True), (" · on your trail · 9 tiles", TEXT, False)]])
    footer_trace(g, "Stack 101 · grid", 6, 64, "Kiroflux Mart ", "ALARM")
    log(g, [("22:08", "camera cracked · held an hour", TEXT),
            ("22:09", "Kiroflux Mart's alarm · tracer out", MAGENTA)])
    return g


def danger_b():
    hub = (200, 66)
    radius = 11.0

    def extra(g):
        for cy in range(5, 35):
            for cx in range(0, 79):
                x, y = gen.OX + cx, gen.OY + cy
                r = ((x - hub[0]) ** 2 + ((y - hub[1]) * 2) ** 2) ** 0.5
                for R, f in ((radius, 0.55), (radius - 1.0, 0.22)):
                    if abs(r - R) < 0.5:
                        c = g.c[cy][cx]
                        c[2] = mix(c[2], GREY_ICE, f)
                        if c[0] in " ·":
                            c[0], c[1] = "·", GREY_ICE

    g = open_net(FLED, extra)
    card(g, [[("STACK 101", "#ffbe00", True), (" · district", DIM, False)],
             [("SEC ", DIM, False), ("117", WHITE, True), (" · sweeping", MAGENTA, False)],
             [("sweep", GREY_ICE, True), (" · reaches you in 2 beats", TEXT, False)]])
    footer_trace(g, "Stack 101 · grid", 6, 64, "Stack 101 ", "SWEEP")
    log(g, [("22:08", "lamp grid cracked · Stack 101", TEXT),
            ("22:09", "Stack 101 sweeps its grid", MAGENTA)])
    return g


def danger_c():
    pieces = [((199, 57), "grey", GREY_ICE), ((193, 53), "white", "#ebf0fa")]

    def extra(g):
        for (px, py), _, colour in pieces:
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                for d in range(1, 7):
                    x, y = px + dx * d, py + dy * d
                    if gen.ter(x, y) not in "LG":
                        break
                    cx, cy = gen.to_cell(x, y)
                    c = g.c[cy][cx]
                    c[2] = mix(c[2], colour, 0.26)
            cx, cy = gen.to_cell(px, py)
            g.c[cy][cx] = ["◈", colour, mix("#010204", colour, 0.35), True]
        # The grey one has seen you: Signal ghost marks its next step.
        cx, cy = gen.to_cell(199, 56)
        g.c[cy][cx][0], g.c[cy][cx][1], g.c[cy][cx][3] = "·", GREY_ICE, True

    g = open_net((199, 54), extra)
    card(g, [[("STACK 101", "#ffbe00", True), (" · district", DIM, False)],
             [("◈ grey sentinel", GREY_ICE, True), (" · hunting · 3", TEXT, False)],
             [("◈ white sentinel", "#ebf0fa", True), (" · on its beat · 6", TEXT, False)]])
    footer_trace(g, "Stack 101 · grid", 3, 30, "Stack 101 ", "HUNTED")
    log(g, [("22:10", "a sentinel has you · Stack 101", MAGENTA)])
    return g


# ---------------------------------------------------------------- the nodes at work

def outside_camera():
    cam = (204, 61)

    def extra(g):
        ncx, ncy = gen.to_cell(*cam)
        w, h = 32, 13
        x0, y0 = 1, 9
        feed = s2.crop(s2.STREET, s2.STREET_CAM[0], s2.STREET_CAM[1], w, h - 1)
        g.paste(feed, x0, y0 + 1)
        g.fill(x0, y0, w, 1, "#06140c")
        g.put(x0 + 1, y0, "CAM 1 · KIROFLUX MART", "#7cff9a", "#06140c", bold=True)
        g.put(x0 + w - 6, y0, "● REC", "#ff4050", "#06140c", bold=True)
        g.put(x0 + 1, y0 + h, "HELD 59m · [l] loop · [esc] close", DIM, PANEL)
        wy = y0 + 6
        for x in range(x0 + w, ncx):
            g.c[wy][x][0], g.c[wy][x][1] = "─", "#7cff9a"
        for y in range(wy + 1, ncy):
            g.c[y][ncx][0], g.c[y][ncx][1] = "│", "#7cff9a"
        g.c[wy][ncx][0], g.c[wy][ncx][1] = "╮", "#7cff9a"
        c = g.c[ncy][ncx]
        c[1], c[3] = "#ffffff", True
        c[2] = mix(c[2], "#7cff9a", 0.55)

    g = open_net(HACKED, extra)
    card(g, [[("KIROFLUX MART", AMBER, True), (" · shop ↑", DIM, False)],
             [("SEC ", DIM, False), ("114", WHITE, True), (" · guarded", TEXT, False)],
             [("◆ camera", TEXT, False), (" · on its wall · tapped", DIM, False)]])
    footer_trace(g, "Stack 101 · grid", 2, 18, "Kiroflux Mart")
    log(g, [("22:06", "camera cracked from the grid", TEXT), ("22:06", "tap · the street, live", TEXT)])
    return g


LAMP = (184, 34)


def lamp_grid():
    def extra(g):
        nx, ny = gen.to_cell(*LAMP)
        g.c[ny][nx] = ["▪", "#ffffff", mix(g.c[ny][nx][2], SIGNAL, 0.55), True]
        dock(g, (nx, ny), [[("LAMP GRID", SIGNAL, True), (" · STACK 101 · HELD 29m", DIM, False)],
                           [("[o]", CYAN, True), (" lights out  ", WHITE, False), ("[esc]", CYAN, True),
                            (" later", DIM, False)]], (nx - 37, ny - 5))

    g = open_net((183, 35), extra, ox=145, oy=14)
    card(g, [[("LAMP GRID", SIGNAL, True), (" · Stack 101 ↗", DIM, False)],
             [("SEC ", DIM, False), ("117", WHITE, True), (" · district", TEXT, False)],
             [("LAMPS ", DIM, False), ("214 lit", TEXT, False), (" · 22:12 night", DIM, False)]])
    footer_trace(g, "Stack 101 · grid", 1, 9, "Stack 101 (district)")
    log(g, [("22:11", "lamp grid cracked · Stack 101", TEXT)])
    return g


AD = (204, 57)
LINES = [("1", "KIROFLUX SELLS YOUR FACE", "smear"),
         ("2", "FREE NOODLES · BACK DOOR · NOW", "lure"),
         ("3", "VEX WAS HERE", "tag")]


def hijack_pick():
    def extra(g):
        nx, ny = gen.to_cell(*AD)
        c = g.c[ny][nx]
        c[1], c[3] = "#ffffff", True
        c[2] = mix(c[2], MAGENTA, 0.6)
        x, y, w = 40, 7, 38
        s2.box(g, x, y, w, 7, "AD BOARDS · WHAT DO THEY SAY", MAGENTA)
        for i, (k, line, kind) in enumerate(LINES):
            g.put(x + 2, y + 2 + i, "[%s]" % k, CYAN, "#060a10", bold=True)
            g.put(x + 6, y + 2 + i, line, WHITE, "#060a10", bold=True)
            g.put(x + w - 2 - len(kind), y + 2 + i, kind, DIM, "#060a10")
        for yy in range(y + 7, ny):
            g.c[yy][nx][0], g.c[yy][nx][1] = "│", MAGENTA

    g = open_net((204, 56), extra)
    card(g, [[("KIROFLUX MART", AMBER, True), (" · shop ↓", DIM, False)],
             [("SEC ", DIM, False), ("114", WHITE, True), (" · guarded", TEXT, False)],
             [("◆ ad board", TEXT, False), (" · held · 3 on the street", DIM, False)]])
    footer_trace(g, "Stack 101 · grid", 2, 21, "Kiroflux Mart")
    return g


def street_ads():
    """Kiroflux Mart's three holo ads cut to your line, a crowd stopped under them."""
    g = s2.street()
    ads = [(38, 16), (42, 16), (46, 20)]
    for x, y in ads:
        g.c[y][x] = ["▣", MAGENTA, mix(g.c[y][x][2], MAGENTA, 0.35), True]
    # The bubble over the middle ad, as the news's cut-ins are drawn.
    x, y, w = 30, 9, 30
    s2.box(g, x, y, w, 4, "", MAGENTA, "#05070b")
    g.put(x + 2, y, " VEX ", MAGENTA, "#05070b", bold=True)
    g.put(x + 2, y + 1, "▸ KIROFLUX SELLS YOUR FACE", WHITE, "#05070b")
    g.put(x + 2, y + 2, "  every ad on the block", DIM, "#05070b")
    for (cx, cy) in ((40, 15), (43, 15), (45, 15), (39, 14), (36, 21), (44, 21)):
        g.c[cy][cx] = ["@", "#c8d0dc", g.c[cy][cx][2], False]
    g.put(81, 2, "bright · unseen".ljust(38), TEXT)
    g.fill(80, 5, 40, 1, PANEL)
    g.put(81, 5, "▣ holo ads · yours · 58 min", MAGENTA)
    log(g, [("22:07", "Kiroflux Mart's ads are yours", TEXT),
            ("22:07", "six stop to stare", DIM)])
    return g


def cone(g, at, colour, f, dotted=False):
    cx, cy = at
    for d in range(1, 8):
        for dx in range(-d, d + 1):
            x, y = cx + dx, cy + d
            if 0 <= x < 79 and 5 <= y < 35:
                c = g.c[y][x]
                if dotted and (x + y) % 2:
                    continue
                c[2] = mix(c[2], colour, f)


def street_eyes():
    g = s2.street()
    cone(g, (42, 20), RED, 0.22)
    g.put(81, 2, "bright · 1 eye · on camera".ljust(38), TEXT)
    g.fill(80, 5, 40, 1, PANEL)
    g.put(81, 5, "◉ Kiroflux Mart's camera has you", RED)
    log(g, [("22:08", "on camera · Kiroflux Mart", RED), ("", "its trace +5 while it sees you", DIM)])
    return g


def street_looped():
    g = s2.street()
    cone(g, (42, 20), "#8a93a3", 0.12, dotted=True)
    g.put(44, 19, "LOOPED 41m", DIM, bold=True)
    g.put(81, 2, "bright · unseen".ljust(38), TEXT)
    g.fill(80, 5, 40, 1, PANEL)
    g.put(81, 5, "◉ Kiroflux Mart's camera · looped", DIM)
    log(g, [("22:07", "loop · Kiroflux Mart's camera", TEXT)])
    return g


def decoy():
    g = round2.inside()
    ox, oy = round2.OFF
    ad = (4, 6)
    piece = (2, 10)
    you = (10, 13)
    # The ad board floods its room.
    ax, ay = ad[0] + ox, ad[1] + oy
    for y in range(4, 9):
        for x in range(2, 7):
            if round2.kt(x, y) != "W":
                c = g.c[y + oy][x + ox]
                c[2] = mix(c[2], MAGENTA, 0.18)
    g.c[ay][ax] = ["▪", "#ffffff", mix(g.c[ay][ax][2], MAGENTA, 0.6), True]
    # The grey piece goes to look: its next steps marked.
    for x, y in ((3, 10), (4, 10)):
        c = g.c[y + oy][x + ox]
        c[0], c[1], c[3] = "·", GREY_ICE, True
    # You: out of the first room, in the second, the spine clear behind the piece.
    old = round2.YOU_IN
    c = g.c[old[1] + oy][old[0] + ox]
    c[0], c[1] = " ", WHITE
    g.c[you[1] + oy][you[0] + ox] = ["@", BLUE, BLUE_GLOW, True]
    dock(g, (ax, ay), [[("AD BOARD", MAGENTA, True), (" · DECOY · 2 beats", DIM, False)],
                       [("ICE goes to look", WHITE, False)]], (ax - 30, ay - 3), wire=MAGENTA)
    g.fill(80, 7, 40, 2, PANEL)
    g.put(81, 7, "◈", "#c8d0dc", bold=True)
    g.put(83, 7, "grey ICE · gate · to the decoy", MAGENTA)
    g.put(81, 8, "◈", RED, bold=True)
    g.put(83, 8, "black ICE · vault · asleep", MAGENTA)
    return g


def shell():
    return gen.frame("s2_shell.svg")


# ---------------------------------------------------------------- page

def fig(g, letter, title, lines, choice):
    rows = "".join("<li>%s</li>" % html.escape(s) for s in lines)
    lt = '<span class="lt">%s</span> ' % letter if letter else ""
    return ('<div class="opt" data-choice="%s" onclick="toggleSelect(this)">'
            '<div class="hd">%s%s</div>%s<ul>%s</ul></div>'
            % (choice, lt, html.escape(title), term.render(gen.shade(g), 10.5), rows))


TRACE_RULES = [
    "Every hack heats the trace of the node's subnet. A node on Kiroflux Mart's wall heats Kiroflux Mart's "
    "trace, from outside or in, so tapping its camera from the grid warms the construct you go into next. "
    "The lamp grid, street terminals and the hub heat a district trace.",
    "Outside, every turn of a run is louder than inside: you're on the open grid, not behind a gate.",
    "The alarm line (60) is where the outside bites back: A, B or C below. The dump line (100) jacks you out "
    "as inside, and on a rig run starts the trace home.",
    "Heat cools when you stop: a subnet you leave alone cools back below the alarm in a few minutes.",
]


def page():
    css = term.CSS + """
.opt{border:1px solid #1d2733;border-radius:8px;padding:12px;margin:0 0 22px;cursor:pointer;background:#0b0f15}
.opt.selected{border-color:#00ffc8;box-shadow:0 0 0 1px #00ffc8}
.opt .hd{font:600 16px system-ui;margin:0 0 8px;color:#e6edf3}
.opt .lt{display:inline-block;background:#00ffc8;color:#02110c;border-radius:4px;padding:0 7px;margin-right:6px}
.opt ul{margin:10px 0 0 18px;padding:0;color:#b8c2cc;font:13.5px/1.5 system-ui}
.rules{color:#c8d0dc;font:14px/1.55 system-ui;max-width:900px}
.rules li{margin:0 0 6px}
h3{margin-top:30px}
"""
    danger = [
        fig(danger_a(), "A", "Tracers follow your trail", [
            "Past the alarm line the subnet sends a tracer out of its gate (a building) or hub (a district): "
            "a piece that doesn't need to see you; it runs your trail, the way you went since the hack.",
            "It rides lanes as you do. If it reaches you it latches: you're dumped, and on a rig run the trace "
            "walks home along your cord.",
            "Ways out: jack out in time, outrun it, lose it through a hub (the hub scrambles a trail), mask your "
            "trail cold, or fight it (it's ICE: crack, attack).",
        ], "danger-a"),
        fig(danger_b(), "B", "The hub sweeps the district", [
            "Past the alarm line the district's hub pulses a ring out across its grid every few beats.",
            "A ring that crosses you adds heat; at the dump line you're out. Inside a construct the rings pass "
            "you by.",
            "No piece to fight; you read the rings and get out of their way.",
        ], "danger-b"),
        fig(danger_c(), "C", "Sentinels patrol the lanes", [
            "Districts keep ICE on their lanes: more and darker the higher their security (corp cores black). "
            "They patrol and see along lanes, as ICE sees along a spine.",
            "Asleep they're just risk on the way; the alarm wakes them and sends them after you. Seen, it's a "
            "fight on the open grid, on turns, as inside.",
            "Danger even before you hack anything, in the hard districts.",
        ], "danger-c"),
    ]
    nodes = [
        fig(outside_camera(), "", "A camera on the wall, from outside", [
            "Beside Kiroflux Mart's wall on the open grid: crack its camera there, tap it, and the feed hangs off "
            "the node as inside. The heat goes on Kiroflux Mart's trace.",
        ], "outside"),
        fig(lamp_grid(), "", "The lamp grid, out on the open Net", [
            "Stack 101's lamp grid sits on a lane out in the district. Crack it, then lights out: the "
            "district's street lamps go dark (round 1's picture). Its heat is the district's.",
        ], "lamp"),
        fig(hijack_pick(), "", "Hijacking the ad boards: what they say", [
            "Hijack asks what the ads say: a smear (a job may want it), a lure (pulls the walkers to it) or "
            "your tag. Lines in the city's own voice, per kind of building.",
        ], "pick"),
        fig(street_ads(), "", "…and on the street", [
            "Every holo ad of the building cuts to your line, as the news cuts in; people nearby stop and "
            "stare at it instead of at you.",
        ], "ads"),
        fig(street_eyes(), "", "Camera eyes in the meat", [
            "A live camera sees a cone in front of it. In its cone you count as an eye on the header, and "
            "while it sees you, that building's trace rises: its construct is hotter when you go in.",
        ], "eyes"),
        fig(street_looped(), "", "…looped", [
            "Looped from the Net, it shows the cone dotted and grey for the hour; walk through.",
        ], "loop"),
        fig(decoy(), "", "A decoy inside", [
            "Inside a construct an ad board can decoy: it floods its room and awake ICE goes to look (Signal "
            "ghost shows its steps). Here the gate's grey ICE leaves the spine while you cross to the vault.",
        ], "decoy"),
        fig(shell(), "", "The building's shell, from the Net", [
            "The terminal node opens the shell you know on the building's host (this is the game's own "
            "screen, on Kiroflux Mart): mail, rosters, door codes.",
        ], "shell"),
    ]
    rules = "".join("<li>%s</li>" % html.escape(r) for r in TRACE_RULES)
    return ("<style>%s</style><h2>Slice 2 · round 2</h2>" % css
            + '<p class="subtitle">With round 1\'s picks: a held node keeps a dock (B), a tapped camera\'s feed '
              "hangs off it (A). You are blue now. Click what you like.</p>"
            + "<h3>Trace outside a construct</h3><ul class=\"rules\">" + rules + "</ul>"
            + "<h3>What hits back outside, past the alarm</h3>"
            + '<div class="options">' + "".join(danger) + "</div>"
            + "<h3>The nodes at work</h3>"
            + '<div class="options">' + "".join(nodes) + "</div>")


if __name__ == "__main__":
    open(OUT, "w").write(page())
    if os.environ.get("FRAMES"):
        for k, f in (("d_a", danger_a), ("d_b", danger_b), ("d_c", danger_c), ("n_out", outside_camera),
                     ("n_lamp", lamp_grid), ("n_pick", hijack_pick), ("n_ads", street_ads),
                     ("n_eyes", street_eyes), ("n_loop", street_looped), ("n_decoy", decoy)):
            open(os.path.join(os.path.dirname(os.path.abspath(OUT)), "frame_%s.html" % k), "w").write(
                "<style>%s</style>%s" % (term.CSS, term.render(gen.shade(f()), 14)))
    print("ok", OUT)
