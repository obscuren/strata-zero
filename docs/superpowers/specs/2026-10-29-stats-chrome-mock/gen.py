"""STATS tab mockups: the stats pane on the left, the chrome ledger on the right.

    python3 gen.py SCRATCH m2.json && python3 page_v2.py m2.json ledger.html

The frame, header and tab bar come from real shots of the deck on STATS at 100x30 and
120x40 (SCRATCH/shots/tmp_stats_100x30.svg and _120x40.svg, taken with the shot
harness's SHOT_DIR); the body is drawn here. ledger.html is the approved ledger.
concepts.html is the first round: three concepts (A doll, B ledger, C x-ray; B was
chosen) and the chrome catalogue, built by page.py from that round's generator, whose
A and C drawings are still below.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS, fold  # noqa: E402

S = sys.argv[1]

CYAN = "#00e5ff"
LIT = "#00c8e0"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#5c6673"
TEXT = "#cdd4de"
PANEL = "#04060a"
RULE = "#283c4b"
BAR = "#003c46"
GOOD = "#50e678"
AMBER = "#ffc850"
STAT = {"BOD": "#78ff78", "REF": CYAN, "TEC": "#e6b43c", "COL": MAGENTA, "NET": "#6e96ff"}

W, H = 100, 30
L0, L1 = 2, 45      # left pane x range, half-open
DIV = 46            # the rule between the panes
R0, R1 = 48, 98     # right pane
TOP, BOT = 5, 27    # body rows, half-open


def size(w, h):
    """The deck's body at w x h: the 120x40 header is taller, the right pane wider."""
    global W, H, R1, TOP, BOT
    W, H = w, h
    R1 = w - 2
    TOP = 5 if h < 40 else 10
    BOT = h - 3


def base():
    g = Grid.from_svg(S + "/shots/tmp_stats_%dx%d.svg" % (W, H), W, H)
    for y in range(TOP, H - 1):
        for x in range(1, W - 1):
            g.c[y][x] = [" ", TEXT, PANEL, False]
    for y in range(TOP, BOT):
        g.put(DIV, y, "║", RULE)
    return g


def hints(g, text):
    g.put(2, H - 2, text, DIM)


def put_spans(g, x, y, spans, right=None, bg=None):
    for text, col, *rest in spans:
        g.put(x, y, text, col, bg=bg, bold=bool(rest and rest[0]), right=right)
        x += len(text)
    return x


def section(g, x0, x1, y, title, tail=None, lit=True):
    """TITLE ──────── tail"""
    g.put(x0, y, title, CYAN if lit else DIM, bold=True)
    tail_w = sum(len(t) for t, *_ in tail) + 1 if tail else 0
    for x in range(x0 + len(title) + 1, x1 - tail_w):
        g.put(x, y, "─", RULE)
    if tail:
        put_spans(g, x1 - tail_w + 1, y, tail)


def tile(g, x, y, w, label, value, mod=None, sel=False, focused=True,
         label_col=DIM, val_col=TEXT, mod_col=GOOD):
    """A rack-style tile: the label in the top border, the value inside, a modifier
    in the bottom border. Selected and focused it is lit like the GEAR rack's."""
    lit = sel and focused
    edge = CYAN if lit else (DIM if sel else RULE)
    g.put(x, y, "┌", edge)
    g.put(x + 1, y, " ", edge)
    g.put(x + 2, y, label, CYAN if lit else label_col, bold=True)
    for i in range(x + 2 + len(label), x + w - 1):
        g.put(i, y, " " if i == x + 2 + len(label) else "─", edge)
    g.put(x + w - 1, y, "┐", edge)
    g.put(x, y + 1, "│", edge)
    inner = w - 2
    if lit:
        g.bg(x + 1, y + 1, inner, BAR)
    v = str(value)
    vx = x + 1 + (inner - len(v)) // 2
    g.put(vx, y + 1, v, val_col, bold=True)
    g.put(x + w - 1, y + 1, "│", edge)
    g.put(x, y + 2, "└", edge)
    for i in range(x + 1, x + w - 1):
        g.put(i, y + 2, "─", edge)
    if mod:
        g.put(x + 2, y + 2, " %s " % mod, mod_col, bold=lit)
    g.put(x + w - 1, y + 2, "┘", edge)


# ---- the left pane

MAIN = [("BOD", 5, "+1"), ("REF", 7, "+1"), ("TEC", 5, None), ("COL", 5, None), ("NET", 8, "+2")]
SECOND = [("HP", 80, None), ("FOC", 51, None), ("ARM", 3, "+3"), ("SGT", 13, "+1"), ("CAR", 28, "+4"),
          ("MOV", 5, "+1")]
RESIST = [("KIN", "15", GOOD), ("THM", "0", TEXT), ("SHK", "10", GOOD), ("EMP", "-18", MAGENTA),
          ("TOX", "0", TEXT), ("FBK", "0", TEXT)]

READOUT = {
    "BOD": [
        [("BODY ", STAT["BOD"], True), ("4", TEXT, True), (" · 5 with chrome", DIM), ("        next 3 pts", AMBER)],
        [("What the meat can take, and how much of it", TEXT)],
        [("there is. Raises ", TEXT), ("HP", CYAN), (", ", TEXT), ("carry", CYAN), (" and your ", TEXT), ("load", CYAN), (".", TEXT)],
        [],
        [("feeds ", DIM), ("◇ Athletics ◇ Freerun ◇ Second wind", TEXT)],
        [("chrome ", DIM), ("+1 Piston arm", GOOD)],
    ],
    "EMP": [
        [("EMP ", MAGENTA, True), ("-18", MAGENTA, True), ("  electromagnetic pulse", DIM)],
        [("Pulses that cook electronics. Meat shrugs", TEXT)],
        [("them off; chrome does not. Every piece you", TEXT)],
        [("carry costs ", TEXT), ("-2", MAGENTA), (", and a hit can knock it out.", TEXT)],
        [],
        [("chrome ", DIM), ("-18 · nine pieces", MAGENTA), ("   gear ", DIM), ("0", TEXT)],
    ],
}


def left(g, sel=None, focused=False):
    """The three blocks of tiles, then the readout of the selected one."""
    x0 = L0
    section(g, x0, L1, TOP, "STATS", [("2", TEXT, True), (" points", DIM)], lit=focused)
    for i, (k, v, mod) in enumerate(MAIN):
        tile(g, x0 + i * 8, TOP + 1, 8, k, v, mod, sel=sel == k, focused=focused, label_col=STAT[k])
    section(g, x0, L1, TOP + 5, "SECONDARY", [("from gear & chrome", FAINT)], lit=focused)
    for i, (k, v, mod) in enumerate(SECOND):
        tile(g, x0 + i * 7, TOP + 6, 7, k, v, mod, sel=sel == k, focused=focused,
             val_col=GOOD if mod else TEXT)
    section(g, x0, L1, TOP + 10, "RESISTANCES", lit=focused)
    for i, (k, v, col) in enumerate(RESIST):
        tile(g, x0 + i * 7, TOP + 11, 7, k, v, sel=sel == k, focused=focused, val_col=col)
    for x in range(x0, L1):
        g.put(x, TOP + 15, "─", RULE)
    lines = READOUT.get(sel or "BOD")
    for j, spans in enumerate(lines):
        put_spans(g, x0, TOP + 16 + j, spans, right=L1)


# ---- the figure, front view: 23 px wide, 34 tall, two pixels to a cell

def R(*segs, w=23):
    r = ["."] * w
    for c, s in segs:
        for i, ch in enumerate(s):
            r[c + i] = ch
    return "".join(r)


FIG = [
    R((10, "mmm")),
    R((9, "hhhhh")),
    R((8, "hhhhhhh")),
    R((7, "r"), (8, "heehddh"), (15, "r")),
    R((8, "hhhhhhp")),
    R((1, "y"), (8, "jjjjjjj"), (21, "y")),
    R((0, "x"), (2, "x"), (9, "jjjjj"), (20, "x"), (22, "x")),
    R((0, "x"), (3, "x"), (10, "nnn"), (19, "x"), (22, "x")),
    R((0, "x"), (4, "x"), (5, "aa"), (7, "ttttttttt"), (16, "bb"), (18, "x"), (22, "x")),
    R((0, "x"), (4, "aa"), (7, "ttttttttt"), (17, "bb"), (22, "x")),
    R((0, "x"), (4, "aa"), (7, "tttoottt"), (15, "t"), (17, "bb"), (22, "x")),
    R((0, "x"), (4, "aa"), (7, "tttoottt"), (15, "t"), (17, "bb"), (22, "x")),
    R((0, "x"), (4, "aa"), (8, "ttttttt"), (17, "bb"), (22, "x")),
    R((1, "y"), (3, "kk"), (8, "ttttttt"), (18, "bb"), (21, "y")),
    R((3, "ss"), (8, "ttttttt"), (18, "bb")),
    R((3, "ss"), (8, "ttttttt"), (18, "bb")),
    R((3, "ss"), (7, "ttttttttt"), (18, "bb")),
    R((2, "qqq"), (7, "ttttttttt"), (18, "fff")),
    R((2, "q"), (4, "q"), (7, "llll"), (12, "llll"), (18, "f"), (20, "f")),
    R((7, "llll"), (12, "llll")),
    R((7, "llll"), (12, "llll")),
    R((7, "llll"), (12, "llll")),
    R((7, "llll"), (12, "llll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((8, "lll"), (12, "lll")),
    R((7, "llll"), (12, "llll")),
    R((7, "llll"), (12, "llll")),
    R(),
    R(),
]
assert all(len(r) == 23 for r in FIG), [len(r) for r in FIG]
assert len(FIG) % 2 == 0

# The x-ray's bones, over the meat: skull, spine, ribs, pelvis, the flesh arm's and
# the legs' long bones. The chrome arm has none.
BONES = [
    R(),
    R((10, "BBB")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "BBBBB")),
    R((10, "BBB")),
    R(),
    R((11, "B")),
    R((8, "BBBBBBB"), (11, "B")),
    R((11, "B"), (17, "B")),
    R((8, "BB"), (11, "B"), (13, "BB"), (17, "B")),
    R((11, "B"), (17, "B")),
    R((9, "B"), (11, "B"), (13, "B"), (17, "B")),
    R((11, "B"), (18, "B")),
    R((11, "B"), (18, "B")),
    R((11, "B"), (18, "B")),
    R((8, "BBBBBBB"), (18, "B")),
    R((8, "B"), (14, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R((9, "B"), (13, "B")),
    R(),
    R(),
    R(),
]
assert len(BONES) == len(FIG)

# Which site each pixel belongs to.
SITE_OF = {"h": "NEURAL", "p": "NEURAL", "m": "NEURAL", "d": "NEURAL", "e": "EYES", "r": "EARS", "j": "JAW", "n": "SPINE", "t": "SKIN", "o": "ORGANS",
           "a": "L ARM", "k": "L ARM", "s": "L ARM", "q": "L ARM", "b": "R ARM", "f": "R ARM", "l": "LEGS",
           "x": "BACK", "y": "BACK"}


def figure(palette, sel=None, glow=None, bones=None):
    """The figure as cells: each code coloured by `palette`, the selected site's pixels
    by `glow` when given, and `bones` drawn over the meat for the x-ray."""
    p = []
    for j, row in enumerate(FIG):
        out = []
        for i, ch in enumerate(row):
            if ch == ".":
                out.append(None)
                continue
            if glow and sel and SITE_OF.get(ch) == sel and ch in glow:
                out.append(glow[ch])
            elif bones and BONES[j][i] == "B" and ch in "hrtlbfjn":
                out.append(bones)
            else:
                out.append(palette.get(ch))
        p.append(out)
    return fold(p, PANEL)


DOLL = {"m": MAGENTA, "d": "#2a2226", "p": CYAN, "h": "#6a5a58", "r": "#6a5a58", "n": "#5a4c4a", "t": "#3c4a5c", "o": "#ff6a3c",
        "e": "#ff2a5a", "j": "#a8b4c0",
        "a": "#a8b4c0", "k": CYAN, "s": "#2a3440", "q": "#7ff3ff",
        "b": "#6a5a58", "f": "#6a5a58", "l": "#34404e", "x": "#7a8696", "y": CYAN}
DOLL_GLOW = {"a": "#e8f6ff", "k": "#ffffff", "s": "#3a6a7a", "q": "#c8fbff"}

XRAY = {"m": "#0c2a38", "d": "#0a2230", "p": CYAN, "h": "#12384a", "r": "#12384a", "n": "#2f7a90", "t": "#0c2a38", "o": "#ffb43c",
        "e": "#ff3c6e", "j": "#d8f4ff",
        "a": "#d8f4ff", "k": CYAN, "s": "#1a4656", "q": "#7ff3ff",
        "b": "#12384a", "f": "#12384a", "l": "#0e3040", "x": "#9adcf0", "y": CYAN}
XRAY_GLOW = {"a": "#ffffff", "k": MAGENTA, "s": "#5a2a6a", "q": "#ffd0f4"}


LOAD, LOAD_MAX = 20, 24


def chrome_header(g, x0=None, x1=None, focused=True, add=0):
    """CHROME ──── LOAD ▰▰▱ 20/24; a piece you're about to fit shows its load in amber."""
    have, new = LOAD // 2, -(-(LOAD + add) // 2)
    tail = [("LOAD ", DIM), ("▰" * have, CYAN if focused else DIM), ("▰" * (new - have), AMBER),
            ("▱" * (LOAD_MAX // 2 - new), FAINT)]
    tail += [(" %d" % (LOAD + add), AMBER if add else TEXT, True), ("/%d" % LOAD_MAX, DIM)]
    section(g, R0 if x0 is None else x0, R1 if x1 is None else x1, TOP, "CHROME", tail, lit=focused)


# Every site, what it holds (None for meat), and the leader's row and side in A.
SITES = [
    ("NEURAL", "Neural port"),
    ("EYES", "Target lens"),
    ("EARS", None),
    ("JAW", "Bulldog jaw"),
    ("SPINE", "Redline spine"),
    ("BACK", "Spider rig"),
    ("ORGANS", "Adrenal pump"),
    ("SKIN", "Subdermal mesh"),
    ("L ARM", "Piston arm"),
    ("R ARM", None),
    ("LEGS", None),
]
HELD = dict(SITES)


# ---- A: the doll, callouts both sides, the card under it

def extent(g, y, x0, x1):
    xs = [x for x in range(x0, x1) if g.c[y][x][0] != " " or g.c[y][x][2] != PANEL]
    return (min(xs), max(xs)) if xs else None


def callouts(g, fx, fy, calls, sel, x0=R0, x1=R1):
    for site, row, side in calls:
        y = fy + row
        held = HELD[site]
        on = site == sel
        name = held if held else "· " + site
        col = CYAN if on else (TEXT if held else FAINT)
        lo, hi = extent(g, y, fx, fx + 23)
        line, dot = (CYAN, "●") if on else (RULE, "·")
        if side == "L":
            g.put(x0, y, name, col, bold=on, bg=BAR if on else None)
            for x in range(x0 + len(name) + 1, lo - 1):
                g.put(x, y, "─" if on else "┈", line)
            g.put(lo - 1, y, dot, line)
        else:
            lx = x1 - len(name)
            g.put(hi + 1, y, dot, line)
            for x in range(hi + 2, lx - 1):
                g.put(x, y, "─" if on else "┈", line)
            g.put(lx, y, name, col, bold=on, bg=BAR if on else None)


def concept_a():
    g = base()
    left(g, sel="BOD", focused=False)
    chrome_header(g)
    sel = "L ARM"
    fig = figure(DOLL, sel=sel, glow=DOLL_GLOW)
    fx, fy = 62, TOP + 1
    g.paste(fig, fx, fy, skip_bg=PANEL)
    callouts(g, fx, fy, [
        ("EYES", 1, "L"), ("JAW", 2, "L"), ("BACK", 4, "L"), ("L ARM", 6, "L"), ("LEGS", 12, "L"),
        ("EARS", 1, "R"), ("NEURAL", 2, "R"), ("SPINE", 3, "R"), ("ORGANS", 5, "R"), ("R ARM", 6, "R"),
        ("SKIN", 8, "R"),
    ], sel)
    cy = TOP + 18
    for x in range(R0, R1):
        g.put(x, cy, "─", RULE)
    put_spans(g, R0, cy + 1, [("◈ PISTON ARM", CYAN, True), ("  L ARM · load 3 · sockets ", DIM),
                              ("■", CYAN), ("□", DIM)], right=R1)
    put_spans(g, R0, cy + 2, [("BODY +1", GOOD), (" · ", DIM), ("Shove", CYAN), ("   hand ", FAINT),
                              ("Arc hand", TEXT), (" · ", DIM), ("Jolt", CYAN), (" · ", DIM), ("SHK +10", GOOD)], right=R1)
    put_spans(g, R0, cy + 3, [("A piston in a sleeve, bolted on at the elbow.", TEXT)], right=R1)
    hints(g, "←↑↓→ select · space open the site · Q/E tabs · tab close")
    return g


# ---- B: the ledger and the card with art, as the SKILLS tab draws it

def piston_arm_art():
    """The Piston arm from the side: shoulder plate, upper arm, the lit elbow, the
    piston over the forearm, the arc hand. 30 x 12 pixels."""
    rows = [
        R(w=30),
        R((0, "MMMM"), (13, "JJJ"), w=30),
        R((0, "MmmM"), (12, "JJJJJ"), (17, "PPPP"), w=30),
        R((0, "MmmM"), (4, "UUUUUUUU"), (12, "JJcJJ"), (17, "PPPPpppppp"), (28, "q"), w=30),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ"), (17, "UUUUUUUU"), (25, "HHH"), (28, "H"), w=30),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ"), (17, "FFFFFFFF"), (25, "HHHH"), (29, "H"), w=30),
        R((0, "MmmM"), (4, "dddddddd"), (12, "JJcJJ"), (17, "FFFFFFFF"), (25, "HHHH"), w=30),
        R((0, "MmmM"), (12, "JJJJJ"), (17, "ffffffff"), (25, "HHH"), (28, "H"), w=30),
        R((0, "MMMM"), (13, "JJJ"), (26, "H"), (29, "H"), w=30),
        R((1, "MM"), (26, "H"), (29, "q"), w=30),
        R((27, "q"), w=30),
        R(w=30),
    ]
    pal = {"M": "#4a5666", "m": "#2a3440", "U": "#e8f2fa", "S": "#a8b4c0", "d": "#5a6676", "J": "#6a7686",
           "c": CYAN, "P": "#8a96a6", "p": "#d8e2ec", "F": "#a8b4c0", "f": "#5a6676", "H": "#c8d4e0",
           "q": "#7ff3ff"}
    p = [[pal.get(ch) if ch != "." else None for ch in r] for r in rows]
    return fold(p, PANEL)


GROUPS = [("head", ["NEURAL", "EYES", "EARS", "JAW"]), ("body", ["SPINE", "BACK", "ORGANS", "SKIN"]),
          ("limbs", ["L ARM", "R ARM", "LEGS"])]


# Sockets a chrome limb brings, and what is in them.
SOCKETS = {"L ARM": [("hand", "Arc hand"), ("forearm", None)]}

# What the mock's bag holds for each empty place.
FITS = {"R ARM": ["Grip arm", "Surgeon arm", "Wrecker arm"],
        "L ARM/forearm": ["Grapple line", "Monowire spool", "Holdout pistol"]}

PIECES = {
    "Grip arm": (3, [("BODY +1", GOOD), (" · ", DIM), ("carry +12", GOOD)],
                 "Hydraulic forearm with a clamp hand. Built for cargo, fine for doors."),
    "Surgeon arm": (3, [("TECH +2", GOOD)], "Twelve joints a finger. Locks and wiring open like books."),
    "Wrecker arm": (4, [("BODY +2", GOOD), (" · punches through doors", DIM)],
                    "Demolition arm, filed down to street legal. Mostly."),
    "Grapple line": (1, [("grants ", DIM), ("Grapple", CYAN), (" · pull 6 tiles", DIM)],
                     "A hooked line in the wrist. Ceilings are now a suggestion."),
    "Monowire spool": (2, [("grants ", DIM), ("Whip", CYAN), (" · reach 2", DIM)],
                       "A wire too thin to see. Count your fingers after."),
    "Holdout pistol": (1, [("a sidearm scanners miss", DIM)], "Comes out of the wrist with a sound like a cough."),
}

NAMES = {"R ARM": ("RIGHT ARM", [("meat", DIM), (" · nothing fitted", FAINT)]),
         "L ARM/forearm": ("FOREARM", [("a socket on the ", DIM), ("Piston arm", TEXT)])}


def wrap(text, width):
    lines, cur = [], ""
    for w in text.split():
        if cur and len(cur) + 1 + len(w) > width:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w).strip()
    return lines + [cur] if cur else lines


def held_at(key):
    site, _, sock = key.partition("/")
    return dict(SOCKETS[site])[sock] if sock else HELD[site]


def ledger(g, sel, x1, focused=True, labels=False):
    """The sites in groups, a chrome limb's sockets folded under it. Wide, every row
    names its site."""
    rows = []
    for group, sites in GROUPS:
        rows.append(("group", group))
        for site in sites:
            rows.append(("site", site))
            rows += [("socket", site + "/" + s) for s, _ in SOCKETS.get(site, [])]
    px = R0 + 2 + (8 if labels else 0)
    y = TOP + 1
    for i, (kind, key) in enumerate(rows):
        if kind == "group":
            g.put(R0 + 1, y, "── %s ──" % key, FAINT)
            y += 1
            continue
        on = key == sel
        if on and focused:
            g.bg(R0, y, x1 - R0, BAR)
        if on:
            g.put(R0, y, "▸", LIT if focused else DIM, bold=True)
        held = held_at(key)
        x = px
        if kind == "socket":
            last = i + 1 == len(rows) or rows[i + 1][0] != "socket"
            g.put(px + 1, y, "└ " if last else "├ ", RULE)
            x = px + 3
            empty = "□ " + key.split("/")[1]
        else:
            if labels:
                g.put(R0 + 2, y, key, FAINT)
            empty = "· meat" if labels else "· " + key.lower()
        if held:
            col = (CYAN if focused else TEXT) if on else (TEXT if kind == "site" else DIM)
            g.put(x, y, held, col, bold=on)
        else:
            g.put(x, y, empty, LIT if on and focused else FAINT, bold=on and focused)
        y += 1
    for yy in range(TOP + 1, BOT):
        g.put(x1, yy, "│", RULE)


def piece_card(g, cx, x1):
    g.paste(piston_arm_art(), cx, TOP + 1, skip_bg=PANEL)
    lines = [
        [("PISTON ARM", CYAN, True)],
        [("L ARM · load 3 · ", DIM), ("EMP -2", MAGENTA)],
        [("BODY +1", GOOD), (" · grants ", DIM), ("Shove", CYAN)],
        [],
        [("■ hand    ", CYAN), ("Arc hand", TEXT, True)],
        [("          ", DIM), ("Jolt", CYAN), (" · ", DIM), ("SHK +10", GOOD)],
        [("□ forearm ", DIM), ("empty", FAINT)],
        [("          grapple, launcher…", FAINT)],
        [],
    ] + [[(t, TEXT)] for t in wrap("A piston in a sleeve, bolted on at the elbow. Good for arguments that have "
                                  "gone physical.", x1 - cx)]
    for j, spans in enumerate(lines):
        put_spans(g, cx, TOP + 8 + j, spans, right=x1)


def fit_card(g, key, pick, cx, x1):
    """An empty site or socket: what in your bag fits it, the pick's numbers, what
    it does to your load."""
    title, sub = NAMES[key]
    y = TOP + 1
    g.put(cx, y, title, CYAN, bold=True)
    put_spans(g, cx, y + 1, sub, right=x1)
    y += 3
    section(g, cx, x1, y, "FITS", [("from your bag", FAINT)])
    y += 1
    for name in FITS[key]:
        load = PIECES[name][0]
        on = name == pick
        if on:
            g.bg(cx, y, x1 - cx, BAR)
        g.put(cx, y, "▸" if on else " ", LIT, bold=on)
        g.put(cx + 2, y, name, CYAN if on else TEXT, bold=on)
        g.put(x1 - 3, y, "L%d" % load, LIT if on else DIM, bold=on)
        y += 1
    load, does, line = PIECES[pick]
    y += 1
    put_spans(g, cx, y, does, right=x1)
    put_spans(g, cx, y + 1, [("load %d → " % LOAD, DIM), ("%d" % (LOAD + load), AMBER, True),
                             ("/%d · " % LOAD_MAX, DIM), ("EMP -2", MAGENTA)], right=x1)
    y += 3
    for t in wrap(line, x1 - cx):
        g.put(cx, y, t, TEXT)
        y += 1


def concept_b(sel="L ARM", pick=None, left_sel="BOD", left_focus=False):
    g = base()
    left(g, sel=left_sel, focused=left_focus)
    wide = W >= 120
    lx1 = R0 + (27 if wide else 18)
    chrome_header(g, focused=not left_focus, add=PIECES[pick][0] if pick else 0)
    ledger(g, sel, lx1, focused=not left_focus and not pick, labels=wide)
    if pick:
        fit_card(g, sel, pick, lx1 + 2, R1)
        hints(g, "↑↓ choose · space install · esc back · Q/E tabs · tab close")
    else:
        piece_card(g, lx1 + 2, R1)
        if left_focus:
            hints(g, "←↑↓→ select · space buy a point · → over to the chrome · Q/E tabs · tab close")
        else:
            hints(g, "↑↓ select · space pull it out · ← stats · Q/E tabs · tab close")
    return g


# ---- C: the x-ray, the cursor on the body, the bay beside it

def concept_c():
    g = base()
    left(g, sel="BOD", focused=False)
    chrome_header(g)
    sel = "L ARM"
    fx, fy = R0 + 1, TOP + 2
    for yy in range(TOP + 1, BOT):
        for xx in range(R0, R0 + 25):
            g.put(xx, yy, "·" if (xx + 2 * yy) % 6 == 0 else " ", "#0e2a36")
    fig = figure(XRAY, sel=sel, glow=XRAY_GLOW, bones="#3c8aa0")
    g.paste(fig, fx, fy, skip_bg=PANEL)
    # The scan bar, passing the elbow.
    scan = fy + 6
    for xx in range(R0, R0 + 25):
        if g.c[scan][xx][0] in " ·" and g.c[scan][xx][2] == PANEL:
            g.put(xx, scan, "─", "#1f6a7c")
    # Brackets round the selected site.
    bx0, by0, bx1, by1 = fx + 2, fy + 3, fx + 7, fy + 10
    for (x, y, ch) in [(bx0, by0, "┌"), (bx1, by0, "┐"), (bx0, by1, "└"), (bx1, by1, "┘")]:
        g.put(x, y, ch, MAGENTA, bold=True)
    g.put(R0, BOT - 1, "▲▼◀▶ the body · ", FAINT)
    g.put(R0 + 16, BOT - 1, "9/11", DIM)
    # The bay.
    bx = R0 + 26
    for yy in range(TOP + 1, BOT):
        g.put(bx - 1, yy, "│", RULE)
    lines = [
        [("◂ ", DIM), ("L ARM", MAGENTA, True), (" ▸", DIM)],
        [],
        [("PISTON ARM", CYAN, True), ("  load 3", DIM)],
        [("replaces the arm", FAINT)],
        [("BODY +1", GOOD), (" · ", DIM), ("Shove", CYAN)],
        [],
        [("SOCKETS", DIM, True)],
        [("■ hand    ", CYAN), ("Arc hand", TEXT, True)],
        [("  ", DIM), ("Jolt", CYAN), (" · ", DIM), ("SHK +10", GOOD)],
        [("□ forearm ", DIM), ("empty", FAINT)],
        [("  takes a grapple,", FAINT)],
        [("  launcher or holdout", FAINT)],
        [],
        [("A piston in a sleeve,", TEXT)],
        [("bolted on at the elbow.", TEXT)],
        [("Good for arguments that", TEXT)],
        [("have gone physical.", TEXT)],
    ]
    for j, spans in enumerate(lines):
        put_spans(g, bx + 1, TOP + 1 + j, spans, right=R1)
    hints(g, "←↑↓→ move over the body · space open the socket · Q/E tabs · tab close")
    return g


def at(w, h, fn):
    def go():
        size(w, h)
        try:
            return fn()
        finally:
            size(100, 30)
    return go


SCREENS = [
    ("limb", "A CHROME LIMB, OPEN", "The cursor on the Piston arm. Its sockets fold under it in the list; the card leads "
     "with its drawing, then what it gives, what sits in each socket, and the line.",
     lambda: concept_b()),
    ("site", "AN EMPTY SITE: WHAT FITS", "Space on an empty site lists the chrome in your bag that fits it. The pick's "
     "numbers sit under the list, and the LOAD gauge shows in amber what it would take.",
     lambda: concept_b(sel="R ARM", pick="Grip arm")),
    ("socket", "AN EMPTY SOCKET", "The same for a socket on a limb: the forearm takes a grapple, a wire, a holdout gun.",
     lambda: concept_b(sel="L ARM/forearm", pick="Grapple line")),
    ("left", "THE LEFT PANE, FOCUSED", "← crosses to the stats. The chrome stays drawn, dimmed; the readout explains "
     "the tile you're on. Here, EMP: the price of going chrome.",
     lambda: concept_b(left_sel="EMP", left_focus=True)),
    ("wide", "AT 120×40", "Wider, the list names every site beside what's in it, and the card gets the room.",
     at(120, 40, lambda: concept_b())),
]


if __name__ == "__main__":
    import json
    out = sys.argv[2]
    which = sys.argv[3:]
    page = [(key, title, blurb, fn().html()) for key, title, blurb, fn in SCREENS if not which or key in which]
    json.dump({"screens": page, "css": CSS}, open(out, "w"))
