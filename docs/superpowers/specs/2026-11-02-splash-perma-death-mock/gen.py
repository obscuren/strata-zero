"""Splash and perma death, round 1: the NAME step with the perma death pick, and three
splash screens that replace the city's generation screen.

    SHOTS=dir/ python3 -I gen.py round1.html

SHOTS holds mk_name_120x40.svg and mk_name_100x30.svg (the NAME step with VEX's build),
from a throwaway shot test.

NAME · the handle, then PERMA DEATH as a second field: ON · OFF, the pick in colour.
A · Marquee: a tall sliced logo, the welcome, the toggles, the banner.
B · Cutaway: the title's logo; the welcome on the left, the city's strata on the right.
C · Channel: the welcome as the city's own civic broadcast, read in its voice."""

import os
import random
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-31-talent-board-mock"))
from cells import Grid, fold  # noqa: E402
from cellhtml import CSS, cell as quad_cell  # noqa: E402

S = os.environ.get("SHOTS", "shots/")

BG = "#04060a"
TEXT = "#cdd4de"
DIM = "#a0aab9"
FAINT = "#78828f"
FAINTER = "#5c6673"
WHITE = "#ffffff"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
AMBER = "#ffc850"
RED = "#ff2828"
RED_TEXT = "#ff6070"
RED_BG = "#1c0508"
RULE = "#283c4b"

HANDLE = "VEX"
CITY = "Hollowside"


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v))) for v in t)


def mix(a, b, f):
    a, b = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(a[k] + (b[k] - a[k]) * f for k in range(3)))


def scale(c, f):
    return rgbhex(tuple(v * f for v in hexrgb(c)))


def wrap(text, width):
    out, line = [], ""
    for word in text.split():
        if line and len(line) + 1 + len(word) > width:
            out.append(line)
            line = word
        else:
            line = word if not line else line + " " + word
    if line:
        out.append(line)
    return out


def load(name, w, h):
    return Grid.from_svg(os.path.join(S, f"{name}.svg"), w, h)


def centre(g, y, s, fg, bold=False, bg=None):
    g.put((g.w - len(s)) // 2, y, s, fg, bg=bg, bold=bold)


def keys(g, x, y, pairs):
    """`[key]verb` pairs in the HUD's footer style."""
    for k, verb in pairs:
        g.put(x, y, "[", FAINT)
        g.put(x + 1, y, k, CYAN, bold=True)
        g.put(x + 1 + len(k), y, "]", FAINT)
        g.put(x + 2 + len(k), y, verb, DIM)
        x += 2 + len(k) + len(verb) + 2
    return x


def box(g, x, y, w, h, fg, title=None, heavy=False, bg=None, title_fg=None):
    tl, tr, bl, br, hz, vt = ("╔", "╗", "╚", "╝", "═", "║") if heavy else ("╭", "╮", "╰", "╯", "─", "│")
    if bg:
        g.fill(x, y, w, h, bg)
    g.put(x, y, tl + hz * (w - 2) + tr, fg, bg=bg)
    for j in range(1, h - 1):
        g.put(x, y + j, vt, fg, bg=bg)
        g.put(x + w - 1, y + j, vt, fg, bg=bg)
    g.put(x, y + h - 1, bl + hz * (w - 2) + br, fg, bg=bg)
    if title:
        g.put(x + 2, y, f" {title} ", title_fg or fg, bg=bg, bold=True)


# ---- the title's logo, the game's block font

GLYPHS = {
    "A": ["▄▀▀▄", "█  █", "█▀▀█", "█  █", "▀  ▀"],
    "E": ["█▀▀▀", "█   ", "█▀▀ ", "█   ", "▀▀▀▀"],
    "O": ["▄▀▀▄", "█  █", "█  █", "█  █", "▀▀▀▀"],
    "R": ["█▀▀▄", "█  █", "█▀▀▄", "█  █", "▀  ▀"],
    "S": ["▄▀▀▀", "█   ", "▀▀▀▄", "   █", "▀▀▀ "],
    "T": ["▀▀█▀▀", "  █  ", "  █  ", "  █  ", "  ▀  "],
    "Z": ["▀▀▀▀█", "   █ ", "  █  ", " █   ", "█▀▀▀▀"],
    " ": ["    "] * 5,
}


def small_logo(g, y, text="STRATA ZERO", colour=MAGENTA):
    w = len(text) * 6
    x0 = (g.w - w) // 2
    for i, c in enumerate(text):
        f = 0.55 if i % 7 == 3 else 1.0
        for r, row in enumerate(GLYPHS[c]):
            fg = mix(scale(colour, f), WHITE, 0.5) if r == 1 else scale(colour, f)
            for col, ch in enumerate(row):
                if ch != " ":
                    g.put(x0 + i * 6 + col, y + r, ch, fg)


# ---- the splash's own logo: 8 x 12 pixels a letter, cut across like strata

BIG = {
    "S": [".#######", "########", "##......", "##......", "##......", "#######.",
          ".#######", "......##", "......##", "......##", "########", "#######."],
    "T": ["########", "########", "...##...", "...##...", "...##...", "...##...",
          "...##...", "...##...", "...##...", "...##...", "...##...", "...##..."],
    "R": ["#######.", "########", "##....##", "##....##", "##....##", "########",
          "#######.", "##..##..", "##...##.", "##....##", "##....##", "##....##"],
    "A": [".######.", "########", "##....##", "##....##", "##....##", "########",
          "########", "##....##", "##....##", "##....##", "##....##", "##....##"],
    "Z": ["########", "########", "......##", ".....##.", "....##..", "....##..",
          "...##...", "..##....", "..##....", ".##.....", "########", "########"],
    "E": ["########", "########", "##......", "##......", "##......", "######..",
          "######..", "##......", "##......", "##......", "########", "########"],
    "O": [".######.", "########", "##....##", "##....##", "##....##", "##....##",
          "##....##", "##....##", "##....##", "##....##", "########", ".######."],
}
# Pixel rows with a cut after them: the logo reads as layers.
CUTS = (3, 7)


def big_logo_pixels(text="STRATA ZERO"):
    rows = []
    for r in range(12):
        line = []
        for i, c in enumerate(text):
            if i:
                line.append(".")
            line.extend(BIG[c][r] if c != " " else "...")
        rows.append(line)
        if r in CUTS:
            rows.append(["."] * len(line))
    return rows


def big_logo(g, y):
    """The sliced logo: magenta at the top running to cyan, a highlight on each slab's
    top edge. Returns the rows it took."""
    px = big_logo_pixels()
    h = len(px)
    if h % 2:
        px.append(["."] * len(px[0]))
        h += 1
    w = len(px[0])
    colours = []
    for r in range(h):
        c = mix(MAGENTA, CYAN, r / (h - 1))
        if r == 0 or (r - 1) in CUTS:
            c = mix(c, WHITE, 0.45)
        colours.append(c)
    p = [[colours[r] if px[r][x] == "#" else None for x in range(w)] for r in range(h)]
    cells = fold(p, BG)
    g.paste(cells, (g.w - w) // 2, y)
    return h // 2


# ---- what every splash says

WELCOME = [
    "The city is built five deep. The Heights look down on the Skyways, the Skyways on "
    "the Street, the Street on the Undercity. Through every wall of it runs the Net, and "
    "the ICE that keeps it.",
    "Everyone here has a job, a bed and a bar, and the clock runs whether you watch or "
    "not. You have a flat in a megablock, a few hundred in your pocket and nobody's "
    "number. Change that.",
]


def tutorial(g, x, y, off=False):
    g.put(x, y, "[", FAINT)
    g.put(x + 1, y, "✓" if off else " ", CYAN, bold=True)
    g.put(x + 2, y, "]", FAINT)
    g.put(x + 4, y, "DISABLE TUTORIAL", TEXT if off else DIM, bold=off)


def banner(g, y, pd=True, x=None, w=None):
    """The perma death banner along the bottom: red while it is on, grey once off."""
    w = w or g.w - 8
    x = (g.w - w) // 2 if x is None else x
    if pd:
        box(g, x, y, w, 4, RED, title="▲ PERMA DEATH IS ON", heavy=True, bg=RED_BG, title_fg=RED)
        g.put(x + 3, y + 1, f"One life. If {HANDLE} flatlines, the save is wiped and the run is over.",
              TEXT, bg=RED_BG)
        g.put(x + 3, y + 2, "This is your last chance to turn it off.", RED_TEXT, bg=RED_BG, bold=True)
        hint_x = x + w - 3 - len("[p] turn it off")
        keys(g, hint_x, y + 2, [("p", "turn it off")])
        for i in range(hint_x, x + w - 3):
            g.c[y + 2][i][2] = RED_BG
    else:
        box(g, x, y, w, 4, FAINTER, title="PERMA DEATH IS OFF", title_fg=DIM)
        g.put(x + 3, y + 1, "A flatline sends you back to your last save.", DIM)
        g.put(x + 3, y + 2, "Turn it on now, or play this run without it.", FAINT)
        keys(g, x + w - 3 - len("[p] turn it on"), y + 2, [("p", "turn it on")])


def footer(g, ready=True):
    keys(g, 2, g.h - 2, [("space", "JACK IN"), ("t", "tutorial"), ("p", "perma death"), ("esc", "back")])
    s = f"{CITY} · {HANDLE}"
    g.put(g.w - 2 - len(s), g.h - 2, s, FAINTER)


# ---- NAME

def name_step(w, h, focus="handle", pd=True):
    g = load(f"mk_name_{w}x{h}", w, h)
    x = 2  # the left pane's edge
    left_w = 43 if w >= 120 else 33
    y = 17 if h >= 40 else 15
    g.put(x, y, "perma death", DIM)
    y += 2
    if focus == "pd":
        g.put(x, 11, "  ", CYAN)
        g.c[11][x + 2 + len(HANDLE)][0] = " "  # no cursor off the field
        g.put(x, y, "▸ ", CYAN)
    on_fg = RED if pd else FAINT
    off_fg = FAINT if pd else CYAN
    g.put(x + 2, y, "ON", on_fg, bold=pd)
    g.put(x + 4, y, " · ", FAINT)
    g.put(x + 7, y, "OFF", off_fg, bold=not pd)
    if pd:
        words = f"One life. If {HANDLE} flatlines, the save goes too and the run is over."
    else:
        words = "A flatline sends you back to your last save."
    for i, l in enumerate(wrap(words, left_w)):
        g.put(x, y + 2 + i, l, RED_TEXT if pd else DIM)
    # The sheet carries it next to the name.
    name_y = 10 if h >= 40 else 9
    if pd:
        tag = "PERMA DEATH"
        g.put(w - 6 - len(tag), name_y, tag, RED, bold=True)
    # The footer for the field you are on.
    g.fill(0, h - 2, w - 16, 1, BG)
    if focus == "pd":
        keys(g, 2, h - 2, [("←→", "on/off"), ("↑", "handle"), ("enter", "next"), ("esc", "back")])
    else:
        keys(g, 2, h - 2, [("type", "name"), ("↓", "perma death"), ("enter", "next"), ("esc", "back")])
    return g


# ---- A · Marquee

def marquee(w, h, pd=True, tut_off=False):
    g = Grid(w, h, BG)
    tall = h >= 36
    y = 4 if tall else 1
    if tall:
        y += big_logo(g, y) + 2
    else:
        small_logo(g, y)
        y += 6
    centre(g, y, "h i g h   t e c h   ·   l o w   l i f e", CYAN)
    y += 3 if tall else 2
    centre(g, y, f"WELCOME TO {CITY.upper()}, {HANDLE}.", WHITE, bold=True)
    y += 2
    tw = 72 if w >= 120 else 64
    for p in WELCOME:
        for l in wrap(p, tw):
            g.put((w - tw) // 2, y, l, TEXT)
            y += 1
        y += 1
    y += 1 if tall else 0
    tutorial(g, (w - tw) // 2, y, tut_off)
    jack = "[space] JACK IN"
    jx = (w + tw) // 2 - len(jack)
    g.put(jx, y, "[", FAINT)
    g.put(jx + 1, y, "space", CYAN, bold=True)
    g.put(jx + 6, y, "]", FAINT)
    g.put(jx + 8, y, "JACK IN", MAGENTA, bold=True)
    banner(g, h - 7, pd)
    footer(g)
    return g


# ---- B · Cutaway

def cutaway_art(w, h, seed=7):
    """The city cut open, top to bottom: Heights, Skyways, Street, Undercity, the Net.
    Pixels, two to a cell; returns the cells and each band's first cell row."""
    rnd = random.Random(seed)
    ph = h * 2
    # Bands in pixel rows.
    heights, skyways, street, under, net = 0, int(ph * 0.30), int(ph * 0.50), int(ph * 0.70), int(ph * 0.86)
    # Sky: near black at the top, the Street's magenta haze rising into it.
    p = []
    for y in range(ph):
        if y < under:
            f = y / under
            p.append([mix("#05060d", "#3a1240", f ** 2.2)] * w)
        elif y < net:
            p.append(["#08080c"] * w)
        else:
            p.append(["#02040a"] * w)
    # Towers, back row first (dim, tall), then the front row (lit, shorter).
    for row, (bodies, tops, lit) in enumerate([
        (["#141a2a", "#171e30"], (heights + 3, skyways - 2), 0.10),
        (["#1f2740", "#242d48", "#1b2236"], (skyways - 6, street - 2), 0.18),
    ]):
        x = rnd.randrange(0, 3)
        while x < w:
            tw = rnd.choice([3, 4, 5, 5, 6, 7]) if row else rnd.choice([4, 5, 6])
            top = rnd.randrange(*tops)
            body = rnd.choice(bodies)
            rim = mix(body, "#7a88b0", 0.25)
            for yy in range(top, under):
                for xx in range(x, min(w, x + tw)):
                    p[yy][xx] = rim if xx == x else body
            # Windows in columns, a few lit.
            for yy in range(top + 2, under - 1, 2):
                for xx in range(x + 2, min(w, x + tw) - 1, 2):
                    if rnd.random() < lit:
                        p[yy][xx] = rnd.choice(["#ffd27a", "#9fd8ff", "#ffe9b0", "#6a7ca8"])
            # A mast and a red beacon on the tall ones.
            if top < skyways and row == 0:
                mid = x + tw // 2
                if mid < w and top >= 3:
                    p[top - 1][mid] = mix(body, "#7a88b0", 0.3)
                    p[top - 2][mid] = "#ff3040"
            # Neon down the face at street level.
            if row == 1 and tw >= 4 and rnd.random() < 0.6:
                sx = x + tw - 2
                c = rnd.choice([MAGENTA, CYAN, AMBER, "#ff4f5e"])
                sy = rnd.randrange(street, max(street + 1, under - 4))
                for k in range(3):
                    if sx < w:
                        p[sy + k][sx] = c
            x += tw + rnd.choice([1, 1, 2, 3])
    # Skyways: two bridges across, pods riding them.
    for by in (skyways + 2, skyways + 7):
        for xx in range(w):
            p[by][xx] = "#3b8fa8"
        for _ in range(max(1, w // 14)):
            px = rnd.randrange(w - 1)
            p[by - 1][px] = p[by - 1][px + 1] = MAGENTA
    # The Street: the road glowing, cars' lights on it.
    for xx in range(w):
        p[under - 1][xx] = "#5a2a5e"
    for _ in range(max(1, w // 7)):
        px = rnd.randrange(w - 1)
        p[under - 1][px] = rnd.choice(["#ffe9b0", "#ff3040"])
    # The Undercity: pillars down from the street, pipes, a few lamps.
    for xx in range(0, w, 7):
        for py in range(under, net):
            p[py][xx] = "#14161e"
    for py in (under + 3, under + 6):
        for xx in range(w):
            if (xx // 11) % 3 != 2:
                p[py][xx] = "#2a2418"
    for _ in range(max(1, w // 8)):
        px, py = rnd.randrange(w), rnd.randrange(under + 1, net - 1)
        p[py][px] = "#d09030"
    # The Net: a cyan lattice in the dark, nodes lit where it crosses.
    for py in range(net + 1, ph):
        for xx in range(w):
            on_row, on_col = (py - net) % 3 == 0, xx % 6 == 0
            if on_row or on_col:
                p[py][xx] = scale(CYAN, 0.45 if on_row and on_col else 0.2)
    for _ in range(max(1, w // 10)):
        px, py = rnd.randrange(0, w, 6), net + 3 * rnd.randrange(1, max(2, (ph - net) // 3))
        if py < ph:
            p[py][px] = CYAN
    g = Grid(w, h, BG)
    for j in range(h):
        for i in range(w):
            t, b = p[2 * j][i], p[2 * j + 1][i]
            g.c[j][i] = [" ", t, t, False] if t == b else ["▀", t, b, False]
    bands = [("HEIGHTS", heights // 2), ("SKYWAYS", skyways // 2), ("STREET", street // 2),
             ("UNDERCITY", under // 2), ("NET", net // 2)]
    return g, bands


def cutaway(w, h, pd=True, tut_off=False):
    g = Grid(w, h, BG)
    tall = h >= 36
    y = 2 if tall else 1
    small_logo(g, y)
    y += 6
    centre(g, y, "h i g h   t e c h   ·   l o w   l i f e", CYAN)
    y += 3 if tall else 2
    tx, tw = 4, (50 if tall else 48)
    cut = tx + tw + 13
    art_w = w - cut - 4
    art_h = h - y - (9 if tall else 8)
    art, bands = cutaway_art(art_w, art_h)
    g.paste(art, cut + 1, y)
    for j in range(art_h):
        g.put(cut, y + j, "│", RULE)
    # Each stratum's name in the gutter, a faint tick to the cut.
    for name, row in bands:
        g.put(cut - 2 - len(name), y + row, name + " ─", FAINTER)
    # The welcome on the left.
    ty = y
    title = f"WELCOME TO {CITY.upper()}, {HANDLE}."
    g.put(tx, ty, title, WHITE, bold=True)
    ty += 2
    for p in WELCOME:
        for l in wrap(p, tw):
            g.put(tx, ty, l, TEXT)
            ty += 1
        ty += 1
    ty = y + art_h - 1
    tutorial(g, tx, ty, tut_off)
    jx = tx + tw - len("[space] JACK IN")
    g.put(jx, ty, "[", FAINT)
    g.put(jx + 1, ty, "space", CYAN, bold=True)
    g.put(jx + 6, ty, "]", FAINT)
    g.put(jx + 8, ty, "JACK IN", MAGENTA, bold=True)
    banner(g, h - 7, pd)
    footer(g)
    return g


# ---- C · Channel

CHANNEL = [
    (f"Welcome to {CITY}, resident {HANDLE}.", WHITE, True),
    ("", TEXT, False),
    ("Your unit is assigned. Your balance is ¤ 320. Your record is clean, for now.", TEXT, False),
    ("", TEXT, False),
    ("Residents of the Street are reminded that the Skyways are not for loitering", TEXT, False),
    ("and the Heights are by invitation. The Undercity is not the city's concern.", TEXT, False),
    ("Net access is a privilege. ICE is lethal by design.", TEXT, False),
    ("", TEXT, False),
    ("Everyone here has a job, a bed and a bar. Find yours.", TEXT, False),
    ("", TEXT, False),
    ("Have a productive night.", CYAN, True),
]


def channel(w, h, pd=True, tut_off=False):
    g = Grid(w, h, BG)
    tall = h >= 36
    y = 2 if tall else 1
    small_logo(g, y)
    y += 6
    centre(g, y, "h i g h   t e c h   ·   l o w   l i f e", CYAN)
    y += 3 if tall else 2
    bw = 86 if w >= 120 else 84
    bx = (w - bw) // 2
    lines = CHANNEL
    bh = len(lines) + 4
    if not tall:
        lines = [l for i, l in enumerate(CHANNEL) if not (l[0] == "" and i in (1, 7, 9))]
        bh = len(lines) + 3
    box(g, bx, y, bw, bh, scale(CYAN, 0.6), title=f"◆ {CITY.upper()} CIVIC CHANNEL", title_fg=CYAN)
    stamp = "WELCOME PACKET · 23:41"
    g.put(bx + bw - 3 - len(stamp), y, f" {stamp} ", FAINT)
    ly = y + 2 if tall else y + 1
    for s, fg, bold in lines:
        g.put(bx + 3, ly, s, fg, bold=bold)
        ly += 1
    # The cursor where the feed stopped typing.
    g.put(bx + 3 + len(lines[-1][0]), ly - 1, "▮", MAGENTA)
    y += bh + (2 if tall else 1)
    tutorial(g, bx + 1, y, tut_off)
    jack = "[space] JACK IN"
    jx = bx + bw - 1 - len(jack)
    g.put(jx, y, "[", FAINT)
    g.put(jx + 1, y, "space", CYAN, bold=True)
    g.put(jx + 6, y, "]", FAINT)
    g.put(jx + 8, y, "JACK IN", MAGENTA, bold=True)
    banner(g, h - 7, pd)
    footer(g)
    return g


CONCEPTS = [
    ("A", "Marquee", marquee,
     "A logo of its own for the splash: STRATA ZERO twice the title's height, cut into "
     "three slabs like the strata, magenta running down to cyan. Under it the welcome "
     "with the city's name and your handle, the tutorial box and JACK IN, then the "
     "banner. Quiet and centred, a title card."),
    ("B", "Cutaway", cutaway,
     "The title's logo, then the city cut open on the right: the Heights' spires with "
     "red lights, the skyway bridges, the Street's neon, the Undercity's pipes and the "
     "Net's lattice at the bottom, each stratum named on the cut. The welcome reads on "
     "the left. The picture changes with nothing; it's art, drawn once."),
    ("C", "Channel", channel,
     "The welcome is the city talking: your new city's civic channel greets you as a "
     "resident, with your real balance, in a voice that doesn't care about you. It "
     "types itself out like the HOLO feed. The most in-world of the three."),
]


def cell(ch, fg, bg, bold):
    """Half blocks as a fill and an inset band: no gradient edges to seam when zoomed."""
    halves = {"▀": (fg, bg), "▄": (bg, fg), "█": (fg, fg)}
    if ch in halves:
        top, bottom = halves[ch]
        if top == bottom:
            return '<i style="background:%s"></i>' % top
        return '<i style="background:%s;box-shadow:inset 0 10.2px 0 0 %s"></i>' % (bottom, top)
    return quad_cell(ch, fg, bg, bold)


def grid_html(g):
    """cellhtml's grid, with each run of blank cells on one background as one box."""
    out = ['<div class="tm">']
    for row in g.c:
        out.append('<div class="r">')
        i = 0
        while i < len(row):
            ch, fg, bg, bold = row[i]
            if ch == " ":
                j = i
                while j < len(row) and row[j][0] == " " and row[j][2] == bg:
                    j += 1
                n = j - i
                out.append('<i style="flex:0 0 %.1fpx;background:%s"></i>' % (9.6 * n + 0.4, bg))
                i = j
                continue
            out.append(cell(ch, fg, bg, bold))
            i += 1
        out.append("</div>")
    out.append("</div>")
    return "".join(out)


def show(g, z):
    """A frame as the window paints it, at zoom `z` of 9.6 x 20 px cells."""
    return f"<div style='zoom:{z}'>" + grid_html(g) + "</div>"


def page():
    out = [
        "<!doctype html><meta charset='utf-8'><title>Splash and perma death · round 1</title><style>",
        CSS,
        # Blocks overlap their neighbours by a hair, so a zoomed frame shows no seams.
        ".tm .r>i{flex:0 0 10px;margin-right:-0.4px;height:20.4px}",
        "body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;"
        "margin:24px auto;max-width:1240px;padding:0 16px}"
        "h1{font-size:22px;margin:0 0 4px}h2{font-size:18px;margin:40px 0 4px;color:#fff}"
        "h3{font-size:15px;margin:18px 0 6px;color:#cdd4de}"
        "p{max-width:860px;color:#a0aab9}.grid{display:grid;grid-template-columns:1fr 1fr;"
        "gap:18px}.cap{font-size:13px;color:#a0aab9;margin:6px 0 0}"
        ".fr{overflow:hidden}@media(max-width:900px){.grid{grid-template-columns:1fr}}"
        "code{color:#00e5ff}",
        "</style>",
        "<h1>Splash and perma death · round 1</h1>",
        "<p>NEW GAME after this change: the chooser, the wizard (perma death set on NAME), "
        "the sheet, <b>JACK IN</b>, and then the splash in place of the generation screen. "
        "The city builds in the ~35 ms between JACK IN and the splash, so the splash already "
        "knows its name. On the splash <code>space</code> jacks in, <code>t</code> ticks "
        "DISABLE TUTORIAL, <code>p</code> flips perma death a last time, "
        "<code>esc</code> goes back to the sheet. All frames at 120×40 unless marked.</p>",
        "<h2>1 · NAME, with perma death</h2>",
        "<p>Perma death is a second field under the handle, picked like the title's NET · "
        "STREET: the pick in colour, the other grey. <code>↓</code> moves to it, "
        "<code>←→</code> picks, <code>↑</code> goes back to typing (space can't flip it: "
        "a handle has spaces). It starts on ON; while it's on, the sheet says PERMA DEATH "
        "beside the name.</p><div class='grid'>",
        "<div class='fr'>" + show(name_step(120, 40), 0.51) + "<div class='cap'>typing the handle · perma death on (the default)</div></div>",
        "<div class='fr'>" + show(name_step(120, 40, focus="pd", pd=False), 0.51) + "<div class='cap'>on the perma death field, turned off</div></div>",
        "</div>",
        "<h2>2 · the splash, three ways</h2>",
        "<p>Each one: STRATA ZERO, a welcome, <b>[ ] DISABLE TUTORIAL</b> (off), JACK IN, "
        "and the red perma death banner along the bottom; it goes grey once perma death "
        "is off. The welcome text is the same in A and B; C says it in the city's voice.</p>",
    ]
    for key, name, fn, words in CONCEPTS:
        out.append(f"<h2>{key} · {name}</h2><p>{words}</p>")
        out.append("<div class='fr'>" + show(fn(120, 40), 1.0) + "</div>")
        out.append("<div class='grid' style='margin-top:14px'>")
        out.append("<div class='fr'>" + show(fn(120, 40, pd=False, tut_off=True), 0.51)
                   + "<div class='cap'>perma death off, tutorial disabled</div></div>")
        out.append("<div class='fr'>" + show(fn(100, 30), 0.612)
                   + "<div class='cap'>at the minimum terminal, 100×30</div></div>")
        out.append("</div>")
    return "\n".join(out)


if __name__ == "__main__":
    open(sys.argv[1] if len(sys.argv) > 1 else "round1.html", "w").write(page())
