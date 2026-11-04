"""Saving the run (#2): mockups over the game's own frames (seed 8, VEX at home), from the
throwaway `save_mock_frames` shot test.

    python3 -I gen.py shots index.html

The title with CONTINUE and HALL OF FAME, LOAD GAME as a list of runs, the pause menu and
the map's saved mark, the flatline with perma death on (the run goes to the HALL OF FAME)
and off (back to the last save)."""

import copy
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")
W, H = 120, 40

BG = "#04060a"
TEXT = "#cdd4de"
DIM = "#a0aab9"
FAINT = "#646e7d"
CYAN = "#00e5ff"
PICK = "#00ccb8"
ROW = "#aab4c3"
EDGE = "#465a6e"
MAGENTA = "#ff00c8"
RED = "#ff4f6a"
PINK = "#f89393"


def load(name):
    return Grid.from_svg(os.path.join(S, "save_%s.svg" % name), W, H)


def centred(g, y, s, fg, bold=False):
    g.put((W - len(s)) // 2, y, s, fg, BG, bold=bold)


def segs(g, x, y, parts):
    """Puts `(text, colour[, bold])` pieces one after another from `x`."""
    for p in parts:
        text, fg = p[0], p[1]
        g.put(x, y, text, fg, BG, bold=len(p) > 2 and p[2])
        x += len(text)
    return x


# ---------------------------------------------------------------- title


def title():
    g = load("title")
    g.fill(40, 15, 80, 8, BG)
    rows = ["CONTINUE", "NEW GAME", "LOAD GAME", "HALL OF FAME", "QUIT"]
    for i, r in enumerate(rows):
        if i == 0:
            g.put(54, 15, "▸ " + r, PICK, BG, bold=True)
        else:
            g.put(56, 15 + i, r, ROW, BG)
    line = [("VEX", CYAN, True), (" · NETRUNNER · Saintburg · day 2 07:40 · ", FAINT),
            ("Bring it to Coz", DIM)]
    n = sum(len(p[0]) for p in line)
    segs(g, (W - n) // 2, 22, line)
    return g


# ---------------------------------------------------------------- the panel lists


def panel(name, head, bottom):
    """The files frame emptied, its box retitled `head` and closed at row `bottom`."""
    g = load("files")
    g.fill(20, 9, 80, 23, BG)
    g.put(20, 8, "╭" + "─" * 78 + "╮", EDGE, BG)
    g.put(21, 8, " ", TEXT, BG)
    g.put(22, 8, head, CYAN, BG)
    g.put(22 + len(head), 8, " ", TEXT, BG)
    for y in range(9, bottom):
        g.put(20, y, "│", EDGE, BG)
        g.put(99, y, "│", EDGE, BG)
    g.put(20, bottom, "╰" + "─" * 78 + "╯", EDGE, BG)
    return g


def header(g, y, names, widths):
    x = 23
    for name, w in zip(names, widths):
        g.put(x, y, name, FAINT, BG)
        x += w


def columns(g, y, cells, picked, widths, tag=None):
    x = 21
    g.put(x, y, "▸ " if picked else "  ", PICK, BG, bold=True)
    x += 2
    for text, w in zip(cells, widths):
        fg = PICK if picked else (TEXT if x == 23 else DIM)
        g.put(x, y, text[: w - 1], fg, BG, bold=picked and x == 23)
        x += w
    if tag:
        g.put(98 - len(tag[0]), y, tag[0], tag[1], BG, bold=True)


def rule(g, y):
    g.put(21, y, "─" * 78, "#1f2b38", BG)


def load_game():
    g = panel("load", " LOAD GAME ", 24)
    widths = [10, 11, 5, 13, 13, 18]
    header(g, 9, ["RUNNER", "BUILD", "LVL", "CITY", "CLOCK", "THE STORY"], widths)
    runs = [
        (["VEX", "NETRUNNER", "4", "Saintburg", "day 2 07:40", "Bring it to Coz"], ("PERMA", MAGENTA)),
        (["KESTREL", "SOLO", "2", "Lowport", "day 1 23:10", "See the doctor"], ("PERMA", MAGENTA)),
        (["BRAM", "TECHIE", "1", "Chromespire", "day 1 06:05", "Watch the news"], None),
    ]
    for i, (cells, tag) in enumerate(runs):
        columns(g, 10 + i, cells, i == 0, widths, tag)
    rule(g, 14)
    segs(g, 23, 16, [("VEX", CYAN, True), (" · NETRUNNER · level 4", TEXT)])
    g.put(98 - len("PERMA DEATH"), 16, "PERMA DEATH", MAGENTA, BG, bold=True)
    segs(g, 23, 17, [("Saintburg · day 2 07:40 · Velform Group Plaza 18F · at home", DIM)])
    segs(g, 23, 18, [("The ledger", TEXT), (" · Bring it to Coz · the Brass Lamp · Ironside", DIM)])
    segs(g, 23, 19, [("¤ 1,240 · 2 chrome · 6 talents · played 3h 12m · saved today 10:44", FAINT)])
    rule(g, 21)
    segs(g, 22, 22, [("↑↓ select · space continue · esc back", FAINT)])
    return g


def hall():
    g = panel("hall", " HALL OF FAME ", 25)
    widths = [10, 11, 5, 13, 10, 26]
    header(g, 9, ["RUNNER", "BUILD", "LVL", "CITY", "LASTED", "THE END"], widths)
    dead = [
        ["MOTH", "NETRUNNER", "6", "Saintburg", "3 days", "black ICE · Velform"],
        ["RIX", "NETRUNNER", "3", "Lowport", "19 hours", "black ICE · Zenbo"],
        ["NAIL", "NETRUNNER", "1", "Chromespire", "2 hours", "black ICE · Aoki"],
    ]
    for i, cells in enumerate(dead):
        columns(g, 10 + i, cells, i == 0, widths)
    rule(g, 14)
    segs(g, 23, 16, [("MOTH", CYAN, True), (" · NETRUNNER · level 6", TEXT)])
    g.put(98 - len("FLATLINED"), 16, "FLATLINED", RED, BG, bold=True)
    segs(g, 23, 17, [("black ICE · Velform Group's vault · day 4 03:12 · 2026-11-02", PINK)])
    segs(g, 23, 18, [("Saintburg · lasted 3 days · played 5h 40m", DIM)])
    segs(g, 23, 19, [("got as far as  ", FAINT), ("The ledger", TEXT), (" · Bring it to Coz", DIM)])
    segs(g, 23, 20, [("left behind    ", FAINT), ("¤ 2,340 · neural port · target lens · 9 talents", DIM)])
    rule(g, 22)
    segs(g, 22, 23, [("↑↓ select · esc back", FAINT)])
    return g


def hall_empty():
    g = panel("hall", " HALL OF FAME ", 16)
    centred(g, 11, "Nobody you ran has flatlined. Yet.", DIM)
    segs(g, 22, 14, [("esc back", FAINT)])
    return g


# ---------------------------------------------------------------- pause and the mark


def pause():
    g = load("pause")
    g.put(44, 22, "  last saved · 2 min ago", FAINT, None)
    return g


def saved_mark():
    g = load("map")
    segs(g, 14, 36, [(" :: ", FAINT), ("SAVED", PICK, True)])
    return g.crop(0, 30, W, 10)


# ---------------------------------------------------------------- the flatline

# One picked per death; the first is the one drawn.
TAUNTS = [
    "somebody's already wearing your jacket.",
    "your rent's still due.",
    "should've stayed in bed.",
    "the street forgot you before you hit the floor.",
    "the ICE didn't even slow down.",
    "they'll sell your chrome by morning.",
]


def flatline(perma):
    g = load("flatlined")
    g.fill(0, 21, W, 6, BG)
    if perma:
        centred(g, 22, TAUNTS[0], PINK)
        centred(g, 24, "any key · title", FAINT)
    else:
        centred(g, 22, "reload last save point · day 2 07:40 · at home", DIM)
        centred(g, 24, "space · load last save     esc · title", FAINT)
    return g


# ---------------------------------------------------------------- page


def fig(g, caption, px=10.4):
    return '<figure>%s<figcaption>%s</figcaption></figure>' % (term.render(g, px), caption)


CSS = term.CSS + """
body{background:#0a0d13;color:#cdd4de;font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding:24px 16px 80px}
main{max-width:1060px;margin:0 auto}
h1{font-size:22px;margin:0 0 4px;color:#fff}
h2{font-size:17px;margin:40px 0 6px;color:#00e5ff;border-bottom:1px solid #1d2a38;
 padding-bottom:6px}
p,li{max-width:820px}
.sub{color:#8a95a5;margin:0 0 18px}
.guess{color:#ffb347}
figure{margin:12px 0 18px;overflow-x:auto}
figcaption{color:#8a95a5;font-size:13px;margin-top:6px;max-width:820px}
"""


def page():
    out = ['<!doctype html><html><head><meta charset="utf-8">',
           '<meta name="viewport" content="width=device-width,initial-scale=1">',
           '<title>Saving the run</title><style>%s</style></head><body><main>' % CSS,
           '<h1>Saving the run</h1>',
           '<p class="sub">Issue #2. Drawn over the game\'s own frames. '
           '<span class="guess">Orange</span> marks my guesses.</p>',

           '<h2>A snapshot of the game, not a list of things to keep</h2>',
           '<ul>'
           '<li><b>Everything that is the game is written down as it stands.</b> You, the '
           'city\'s people where they are this minute, every floor you walked into, the Net and '
           'what you did to it (a camera on a loop, a lamp you cut), foes on you, a fuse '
           'burning, the clock, and the dice: a reload rolls what it would have rolled.</li>'
           '<li><b>New things are saved without anyone remembering to.</b> The run\'s state '
           'lives in one place in the code. Anything added there is in the save, and the game '
           'doesn\'t build if a new piece can\'t be written.</li>'
           '<li><b>Left out:</b> which screen is open and where its cursor is, your settings '
           '(they\'re yours, not the run\'s), and the game\'s own content, which comes with '
           'the build.</li>'
           '<li><b>An older save opens in a newer build.</b> Whatever the new build adds starts '
           'fresh. A save it really can\'t read says so in LOAD GAME instead of opening half '
           'a run.</li>'
           '<li><b>It saves</b> on quit, when you wake, when a story step ends, and on SAVE '
           'GAME. Quitting doesn\'t get you out of a fight: you come back in it.</li>'
           '</ul>',

           '<h2>The title</h2>',
           fig(title(), 'CONTINUE picks up your last run, its line beneath the menu. '
               'HALL OF FAME is a row of its own. LOAD GAME and HALL OF FAME are lists, '
               'below.'),

           '<h2>LOAD GAME: your runs</h2>',
           fig(load_game(), 'One row per run, newest first, with a card for the one you\'re '
               'on. <b>PERMA</b> marks a one-life run. <span class="guess">A flatlined run '
               'leaves this list for the HALL OF FAME (my reading of "can\'t be opened").</span>'),

           '<h2>HALL OF FAME</h2>',
           fig(hall(), 'The flatlined runs, kept and marked, never opened again. The card says '
               'how it ended, how long it lasted, how far the story got and what was left '
               'behind.'),
           fig(hall_empty(), 'Before anyone has flatlined.'),

           '<h2>Playing: the pause menu and the mark</h2>',
           fig(pause(), 'The rows stay. LOAD GAME opens your runs; both QUIT rows save first. '
               '<span class="guess">A line says when it last saved.</span>'),
           fig(saved_mark(), 'When the game saves, the status line says so for a few '
               'seconds.', px=12),

           '<h2>The flatline</h2>',
           fig(flatline(True), 'Perma death on: the save is kept and marked FLATLINED, and the '
               'run goes to the HALL OF FAME without a word about it. The screen just taunts. '
               '<span class="guess">One line per death, picked from a small bank: ' +
               ' · '.join('&ldquo;%s&rdquo;' % t for t in TAUNTS[1:]) + '</span>'),
           fig(flatline(False), 'Perma death off: reload the last save point. Nothing is '
               'marked.'),

           '<h2>My calls</h2>',
           '<ul>'
           '<li>One file per run, holding its city. They live in the player\'s folder (#3).</li>'
           '<li>The old <code>.city</code> files stay for dev and the command line; a '
           'player\'s LOAD GAME lists runs only.</li>'
           '<li>The courier broadcast plays on CONTINUE while the run is at the morning '
           'after.</li>'
           '<li>Your settings and the HALL OF FAME belong to you, not to a run: no run '
           'holds them.</li>'
           '</ul>',
           '</main></body></html>']
    open(OUT, "w").write("".join(out))
    print("wrote", OUT)


page()
