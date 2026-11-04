"""Slice 2 mockups, round 1: what a hacked node does. Drawn over the game's own frames
(seed 8): inside Kiroflux Mart's construct beside its camera, and the street round that
camera at 22:00.

    python3 -I s2.py <shots dir> <out.html>

The shots dir holds s2_inside.svg, s2_street.svg (from the throwaway tmp_slice2 shot
test) and what gen.py reads.

1 · holding a node: A a verb popup, B the run's tether stays as a dock with the verbs,
    C the hotbar turns into the node's verbs.
2 · a camera tapped, the street seen from the Net: A a feed hanging off the node on a
    wire, B the MAP pane becomes the feed, C a full cut to the camera.
3 · the lamp grid: a district's street at night, then lights out."""

import html
import os
import sys

S, OUT = sys.argv[1], sys.argv[2]
sys.argv = [sys.argv[0], S, os.devnull]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-11-03-net-look-mock"))
import gen  # noqa: E402
from gen import AMBER, CYAN, DIM, FAINT, MAGENTA, PANEL, RULE, SIGNAL, TEXT, WHITE, mix, rgb, scale, term  # noqa: E402
from cells import Grid  # noqa: E402

OFF = (34, 18)  # construct tile -> cell
CAM = (37, 19)  # the camera node's cell
YOU = (38, 19)
STREET_CAM = (42, 20)  # the camera on the street frame
GREEN = "#7cff9a"
HELD = "#00ffc8"
VOID = "#010204"


def inside():
    return gen.frame("s2_inside.svg")


def street():
    return gen.frame("s2_street.svg")


def held_node(g):
    """The held camera: the node on a signal glow, its glyph bright."""
    c = g.c[CAM[1]][CAM[0]]
    c[1], c[3] = "#ffffff", True
    c[2] = mix(c[2], HELD, 0.55)


def here_line(g, text):
    """HERE's line for the node under you, after the card; the panes below move down."""
    g.fill(80, 8, 40, 4, PANEL)
    g.put(81, 8, text, TEXT)
    g.put(80, 9, "─" * 40, RULE)
    g.put(81, 10, "no job on · find a fixer", TEXT)
    g.put(80, 11, "─" * 40, RULE)


def box(g, x, y, w, h, title, fg=CYAN, bg="#060a10"):
    g.fill(x, y, w, h, bg)
    g.put(x, y, "┌" + "─" * (w - 2) + "┐", fg, bg)
    for j in range(1, h - 1):
        g.put(x, y + j, "│", fg, bg)
        g.put(x + w - 1, y + j, "│", fg, bg)
    g.put(x, y + h - 1, "└" + "─" * (w - 2) + "┘", fg, bg)
    if title:
        g.put(x + 2, y, " " + title + " ", fg, bg, bold=True)


# ---------------------------------------------------------------- 1 · holding a node

def hold_a():
    g = inside()
    held_node(g)
    here_line(g, "◆ camera · held · 59 min")
    x, y, w = 43, 11, 34
    box(g, x, y, w, 7, "CAMERA · HELD 59 MIN", HELD)
    rows = [("t", "TAP", "see what it sees"), ("l", "LOOP", "it stops seeing you"), ("esc", "", "later")]
    for i, (k, verb, what) in enumerate(rows):
        yy = y + 2 + i
        g.put(x + 2, yy, "[%s]" % k, CYAN, "#060a10", bold=True)
        g.put(x + 8, yy, verb, WHITE, "#060a10", bold=True)
        g.put(x + 14, yy, what, DIM, "#060a10")
    g.put(81, 26, "22:06", DIM)
    g.put(87, 26, "camera cracked · held an hour", TEXT)
    return g


def hold_b():
    g = inside()
    held_node(g)
    here_line(g, "◆ camera · held · 59 min")
    # The tether, landed and kept: a wire from the node to a dock on the left.
    lx, ly = 16, 18
    g.put(lx, ly, "CAMERA", HELD, "#04120e", bold=True)
    g.put(lx + 7, ly, "HELD 59m", DIM, "#04120e")
    g.put(lx, ly + 1, "[t]", CYAN, "#04120e", bold=True)
    g.put(lx + 3, ly + 1, " tap ", WHITE, "#04120e")
    g.put(lx + 8, ly + 1, "[l]", CYAN, "#04120e", bold=True)
    g.put(lx + 11, ly + 1, " loop", WHITE, "#04120e")
    for x in range(lx + 16, CAM[0] - 1):
        g.c[ly + 1][x][0], g.c[ly + 1][x][1] = "─", HELD
    g.c[ly + 1][CAM[0] - 1][0], g.c[ly + 1][CAM[0] - 1][1] = "╯", HELD
    g.c[ly][CAM[0] - 1][0], g.c[ly][CAM[0] - 1][1] = "│", HELD
    g.put(81, 26, "22:06", DIM)
    g.put(87, 26, "camera cracked · held an hour", TEXT)
    return g


def hold_c():
    g = inside()
    held_node(g)
    here_line(g, "◆ camera · held · 59 min")
    g.fill(0, 39, 120, 1, PANEL)
    g.put(1, 39, "CAMERA", HELD, bold=True)
    g.put(8, 39, "held 59m", DIM)
    g.put(19, 39, "[1]", CYAN, bold=True)
    g.put(23, 39, "TAP ●", WHITE)
    g.put(31, 39, "[2]", CYAN, bold=True)
    g.put(35, 39, "LOOP ●", WHITE)
    g.put(45, 39, "[3] ──", FAINT)
    g.put(55, 39, "[tab] programs", DIM)
    g.put(81, 26, "22:06", DIM)
    g.put(87, 26, "camera cracked · held an hour", TEXT)
    return g


# ---------------------------------------------------------------- 2 · the feed

def cctv(c, row, people=True):
    """A street cell as a camera sees it: green phosphor by its light, every other row
    a shade darker; walkers bright."""
    ch, fg, bg, bold = c
    lf = sum(rgb(fg)) / 765
    lb = sum(rgb(bg)) / 765
    dark = 0.78 if row % 2 else 1.0
    nfg = gen.hexc((40 * lf * dark, 255 * min(1, lf * 1.3) * dark, 150 * lf * dark))
    nbg = gen.hexc((4 + 10 * lb * dark, 10 + 60 * lb * dark, 8 + 30 * lb * dark))
    if people and ch == "@":
        return ["@", "#ffffff", nbg, True]
    if fg.lower() == "#ff3c46":
        return [ch, "#ff5a64", nbg, True]
    return [ch, nfg, nbg, bold]


def crop(g, cx, cy, w, h):
    """`w` × `h` cells of the street round (cx, cy), recoloured as the camera sees them."""
    out = Grid(w, h)
    for j in range(h):
        for i in range(w):
            x, y = cx - w // 2 + i, cy - h // 2 + j
            if 0 <= x < 79 and 5 <= y < 35:
                out.c[j][i] = cctv(STREET.c[y][x], j)
            else:
                out.c[j][i] = [" ", "#000000", "#020604", False]
    return out


STREET = None


def feed_a():
    g = inside()
    held_node(g)
    here_line(g, "◆ camera · tapped · 59 min")
    w, h = 32, 13
    x0, y0 = 1, 10
    feed = crop(STREET, STREET_CAM[0], STREET_CAM[1], w, h - 1)
    g.paste(feed, x0, y0 + 1)
    g.fill(x0, y0, w, 1, "#06140c")
    g.put(x0 + 1, y0, "CAM 1 · KIROFLUX MART", GREEN, "#06140c", bold=True)
    g.put(x0 + w - 6, y0, "● REC", "#ff4050", "#06140c", bold=True)
    # The wire from the feed's edge to the node.
    for x in range(x0 + w, CAM[0]):
        g.c[CAM[1]][x][0], g.c[CAM[1]][x][1] = "─", GREEN
    return g


def feed_b():
    g = inside()
    held_node(g)
    here_line(g, "◆ camera · tapped · 59 min")
    feed = crop(STREET, STREET_CAM[0], STREET_CAM[1], 40, 12)
    g.paste(feed, 80, 12)
    g.fill(80, 24, 40, 1, PANEL)
    g.put(81, 24, "CAM 1 · Kiroflux Mart", GREEN)
    g.put(107, 24, "● live", "#ff4050", bold=True)
    return g


def feed_c():
    g = inside()
    feed = crop(STREET, STREET_CAM[0], STREET_CAM[1] + 0, 79, 30)
    g.paste(feed, 0, 5)
    # The camera's dressing: corners, the stamp, the way back.
    for (x, y, ch) in ((1, 6, "┌"), (77, 6, "┐"), (1, 33, "└"), (77, 33, "┘")):
        g.put(x, y, ch, GREEN, bold=True)
    for y in (6, 33):
        for x in range(2, 77):
            g.c[y][x] = [" ", GREEN, "#020a05", False]
    g.put(3, 6, "CAM 1 · KIROFLUX MART · STACK 101", GREEN, bold=True)
    g.put(60, 6, "22:06:14", GREEN)
    g.put(70, 6, "● REC", "#ff4050", bold=True)
    g.put(3, 33, "[esc] back to the Net", DIM)
    g.put(56, 33, "1 of 1 · [tab] next", DIM)
    here_line(g, "◆ camera · tapped · 59 min")
    return g


# ---------------------------------------------------------------- 3 · lights out

def lamps_off(g):
    """The district's street lamps out: each lamp dark, the pool round it fallen to
    the night; neon, holo and windows stay lit."""
    out = Grid(g.w, g.h)
    out.c = [[list(c) for c in row] for row in g.c]
    lamps = [(x, y) for y in range(5, 35) for x in range(0, 79) if g.c[y][x][0] == "●"]
    for y in range(5, 35):
        for x in range(0, 79):
            near = min((abs(x - lx) / 2 + abs(y - ly) for lx, ly in lamps), default=99)
            c = out.c[y][x]
            if near <= 3.5:
                f = 0.35 + 0.15 * near / 3.5
                c[2] = scale(c[2], f)
                if c[0] not in "@◉":
                    c[1] = scale(c[1], f + 0.15)
    for lx, ly in lamps:
        c = out.c[ly][lx]
        c[1], c[3] = "#3a3a34", False
    return out


def lights(name):
    g = street()
    if name == "after":
        g = lamps_off(g)
    return g.crop(14, 7, 56, 24)


# ---------------------------------------------------------------- page

def fig(g, letter, title, lines, choice, scale_px=10.5):
    rows = "".join("<li>%s</li>" % html.escape(s) for s in lines)
    return ('<div class="opt" data-choice="%s" onclick="toggleSelect(this)">'
            '<div class="hd"><span class="lt">%s</span> %s</div>%s<ul>%s</ul></div>'
            % (choice, letter, html.escape(title), term.render(gen.shade(g), scale_px), rows))


VERBS = [
    ("shape", "Programs get the node; the node gives the verbs",
     "Crack (loud) or spoof (quiet) takes a node for an hour. While you hold it, its kind "
     "offers its own verbs. A new node later means new verbs, not new programs."),
    ("camera", "Camera · tap, loop",
     "Tap: see the street round it from the Net (part 2 here), and for an hour on your "
     "minimap once you're back in your body. Loop: it stops seeing you, once cameras count as "
     "eyes in the meat."),
    ("adboard", "Ad board · hijack, decoy",
     "Hijack: your message on the building's holo ads; walkers stop and stare (cover) or a "
     "fake offer pulls a lobby empty. Decoy (in the construct): it floods its room and awake "
     "ICE goes to look."),
    ("lamp", "Lamp grid · lights out",
     "One node per district, out on the open Net (the first hack outside a construct). The "
     "district's street lamps go dark for a while; your exposure falls (part 3 here)."),
    ("terminal", "Terminal · shell",
     "Opens the building's shell from the Net: the same terminal you use on the street, on "
     "that building's host: mail, rosters, door codes."),
    ("packages", "Packages · intercept (later)",
     "Some packets on the lanes become couriers. A fixer job: catch one between two "
     "constructs, then copy it, steal it or reroute it."),
]


def page():
    global STREET
    STREET = street()
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
.pair{display:flex;gap:14px;flex-wrap:wrap}
.pair .cap{font:600 13px system-ui;color:#c8d0dc;margin:0 0 6px}
h3{margin-top:30px}
"""
    hold = [
        fig(hold_a(), "A", "A verb popup", [
            "The crack lands: a small window names the node, how long you hold it, and its verbs.",
            "One key does it; esc keeps the hold for later (HERE still says held).",
        ], "hold-a"),
        fig(hold_b(), "B", "The tether stays: a dock with the verbs", [
            "The run's live wire doesn't reel in when it lands: it stays on the node as a dock that reads "
            "HELD and the verbs, keyed by letter while you're in the construct.",
            "Same picture as the run, so a held node looks like a finished run.",
        ], "hold-b"),
        fig(hold_c(), "C", "The hotbar turns into the node's verbs", [
            "Beside a held node the PROGRAMS row becomes its verbs on the same number keys; tab flips "
            "back to your programs.",
            "Nothing new on the map; the footer you already read.",
        ], "hold-c"),
    ]
    feed = [
        fig(feed_a(), "A", "A feed hanging off the node", [
            "Tapped, the camera opens a small live window on a wire from its node: the street round "
            "it, walkers moving, in the camera's green.",
            "It stays open while you're in the construct; one per camera; you keep running.",
        ], "feed-a"),
        fig(feed_b(), "B", "The MAP pane becomes the feed", [
            "While a tap lasts, the sidebar's minimap shows the camera instead. Nothing covers the run.",
            "Small (40 by 12), but always there.",
        ], "feed-b"),
        fig(feed_c(), "C", "A full cut to the camera", [
            "The map cuts to the camera: the whole view, stamped and recording, esc back to the Net, "
            "tab to the next camera you hold.",
            "Biggest picture; the run waits while you look.",
        ], "feed-c"),
    ]
    before, after = lights("before"), lights("after")
    lamp = ('<div class="pair"><div><div class="cap">Stack 101 at 22:00</div>%s</div>'
            '<div><div class="cap">The lamp grid held · lights out</div>%s</div></div>'
            % (term.render(gen.shade(before), 10.5), term.render(gen.shade(after), 10.5)))
    ideas = "".join('<div class="idea" data-choice="%s" onclick="toggleSelect(this)"><h4>%s</h4><p>%s</p></div>'
                    % (k, html.escape(t), html.escape(d)) for k, t, d in VERBS)
    return ("<style>%s</style><h2>Slice 2 · what a hacked node does · round 1</h2>" % css
            + '<p class="subtitle">Kiroflux Mart (seed 8): its construct, and the street round its camera at '
              "22:00. Click the ones you like; tick the ideas to keep.</p>"
            + "<h3>1 · Holding a node: how its verbs show</h3>"
            + '<div class="options">' + "".join(hold) + "</div>"
            + "<h3>2 · A camera tapped: the street from the Net</h3>"
            + '<div class="options">' + "".join(feed) + "</div>"
            + "<h3>3 · The lamp grid: lights out (a rough picture)</h3>" + lamp
            + "<h3>The verbs, node by node</h3>"
            + '<div class="ideas" data-multiselect>' + ideas + "</div>")


if __name__ == "__main__":
    html_out = page()
    open(OUT, "w").write(html_out)
    if os.environ.get("FRAMES"):
        for k, f in (("hold_a", hold_a), ("hold_b", hold_b), ("hold_c", hold_c),
                     ("feed_a", feed_a), ("feed_b", feed_b), ("feed_c", feed_c)):
            open(os.path.join(os.path.dirname(os.path.abspath(OUT)), "frame_%s.html" % k), "w").write(
                "<style>%s</style>%s" % (term.CSS, term.render(gen.shade(f()), 14)))
        for k in ("before", "after"):
            open(os.path.join(os.path.dirname(os.path.abspath(OUT)), "frame_lamp_%s.html" % k), "w").write(
                "<style>%s</style>%s" % (term.CSS, term.render(gen.shade(lights(k)), 14)))
    print("ok", OUT)
