"""Story missions, round 1: the welcome dialog (three looks) and the BCI call (three
places, ringing and talking), drawn over the game's own frames.

    SHOTS=dir/ python3 -I gen.py out.html

SHOTS holds tmp_street_120x40.svg (VEX at home, 18F) and tmp_outside_120x40.svg (VEX on
the street outside a block), taken with a throwaway shot test in screens/shots.rs:
site_state(), MapScreen::walking, and for the street the player moved onto the
Street stratum beside the home block's first portal. round1.html (the concepts),
round2.html (the picks: welcome B, call 1, HOLO) and round3.html (the glitching face,
gen3.py) are the approved pages."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")
W, H = 120, 40

CYAN = "#00e5ff"
CYAN_DIM = "#005059"
MAGENTA = "#ff00c8"
MAG_DIM = "#5a0046"
DIM = "#a0aab9"
FAINT = "#5c6673"
TEXT = "#cdd4de"
PANEL = "#04060a"
BLACK = "#000000"
AMBER = "#ffc850"
RED = "#ff4f5e"
BLOOD = "#5a0c12"

# ---------------------------------------------------------------- helpers


def scale(c, f):
    r, g, b = (int(c[i:i + 2], 16) for i in (1, 3, 5))
    return "#%02x%02x%02x" % (round(r * f), round(g * f), round(b * f))


def wash(g, top=0, bottom=None, right=None):
    """The game's dim::wash: everything to 35 %, odd rows a little darker."""
    bottom = g.h if bottom is None else bottom
    for y in range(top, bottom):
        f = 0.35 * (0.8 if y % 2 else 1.0)
        for cell in g.c[y][:right]:
            cell[1] = scale(cell[1], f)
            cell[2] = scale(cell[2], f)


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


def popup_frame(g, x, y, w, h, title, color=CYAN):
    """The game's Popup frame: ┍━ title ━┑, │ sides, ┕━┙, on the panel."""
    g.fill(x, y, w, h, PANEL, " ", TEXT)
    g.put(x, y, "┍" + "━" * (w - 2) + "┑", color)
    g.put(x + 2, y, " %s " % title, color, bold=True)
    for j in range(1, h - 1):
        g.put(x, y + j, "│", color)
        g.put(x + w - 1, y + j, "│", color)
    g.put(x, y + h - 1, "┕" + "━" * (w - 2) + "┙", color)


def bracket_frame(g, x, y, w, h, color, bg=PANEL):
    """Half-block brackets at the corners, faint rule between: the BCI's own frame."""
    g.fill(x, y, w, h, bg, " ", TEXT)
    for i in range(w):
        g.put(x + i, y, "▀", scale(color, 0.35), bg)
        g.put(x + i, y + h - 1, "▄", scale(color, 0.35), bg)
    for j in range(h):
        g.put(x, y + j, "▌", scale(color, 0.35), bg)
        g.put(x + w - 1, y + j, "▐", scale(color, 0.35), bg)
    g.put(x, y, "▛▀▀", color, bg)
    g.put(x + w - 3, y, "▀▀▜", color, bg)
    g.put(x, y + h - 1, "▙▄▄", color, bg)
    g.put(x + w - 3, y + h - 1, "▄▄▟", color, bg)
    g.put(x, y + 1, "▌", color, bg)
    g.put(x + w - 1, y + 1, "▐", color, bg)
    g.put(x, y + h - 2, "▌", color, bg)
    g.put(x + w - 1, y + h - 2, "▐", color, bg)


# ---------------------------------------------------------------- the face

FACE = [
    "................",
    ".....dddddd.....",
    "...dddddddddd...",
    "..dddddmmddddd..",
    "..ddmmmmmmmmdd..",
    "..dmllllllllmd..",
    "..mlbbllllbblm..",
    "..mlXXlllleelm..",
    "..mlXXllllllm...",
    "..mllllhhllllm..",
    "...mlllhllllm...",
    "...mllmmmmllm...",
    "....mllllllm....",
    ".....mmllmm.....",
    "....dmmmmmmd....",
    "..dddmmmmmmddd..",
    ".dddddddddddddd.",
    "dddddddddddddddd",
]


def face(tint, eye):
    """The caller as a hologram: a cyan ramp, the chrome eye in `eye`, every second
    pixel row darker for scanlines, one row slipped a pixel for the jitter."""
    ramp = {
        "d": scale(tint, 0.22),
        "m": scale(tint, 0.45),
        "l": scale(tint, 0.75),
        "h": scale(tint, 1.0),
        "b": scale(tint, 0.3),
        "e": "#e8ffff",
        "X": eye,
    }
    px = []
    for j, row in enumerate(FACE):
        if j == 9:
            row = row[1:] + "."
        line = []
        for ch in row:
            c = ramp.get(ch)
            if c and j % 2 == 1 and ch not in "Xe":
                c = scale(c, 0.6)
            line.append(c)
        px.append(line)
    return px


def put_face(g, x, y, px, bg=PANEL):
    for j in range(len(px) // 2):
        for i in range(len(px[0])):
            t, b = px[2 * j][i], px[2 * j + 1][i]
            if t is None and b is None:
                g.c[y + j][x + i] = [" ", TEXT, bg, False]
            elif t is None:
                g.c[y + j][x + i] = ["▄", b, bg, False]
            elif b is None:
                g.c[y + j][x + i] = ["▀", t, bg, False]
            else:
                g.c[y + j][x + i] = ["▀", t, b, False]


# ---------------------------------------------------------------- words

WELCOME = [
    "Ey, welcome to the block, choom. Word on the wire says you got what it takes. "
    "Preem. Let's find out.",
    "Got a package needs walking across town. Nothing heavy, nothing hot. Goes to "
    "Marlo, cooks noodles at Sato's on the Strip. Into his hands. Not the counter, "
    "not his cousin. Hands.",
    "Do it clean, there's more where that came from. Don't open it. Don't get cute. "
    "Don't be late.",
]

CALL = [
    "Choom. Choom, you there? Whatever you're looking at, don't touch it.",
    "Marlo was meant to be breathing when you got there. Somebody got there first.",
    "Badges are four minutes out. Walk, don't run. Keep the package. Get home.",
]


def home():
    return Grid.from_svg(S + "tmp_street_120x40.svg", W, H)


def street():
    g = Grid.from_svg(S + "tmp_outside_120x40.svg", W, H)
    # Marlo, face down beside the stall, and what ran out of him.
    for x, y in [(40, 20), (41, 20), (42, 20), (40, 19), (41, 19), (42, 19), (43, 19), (41, 18)]:
        g.c[y][x][2] = BLOOD
        if g.c[y][x][0] in "░·":
            g.c[y][x][0] = " "
    g.c[20][41] = ["%", "#ff5a64", BLOOD, True]
    return g


# ---------------------------------------------------------------- welcome A, B, C


def welcome_popup():
    g = home()
    wash(g)
    w = 64
    body = []
    for i, p in enumerate(WELCOME):
        if i:
            body.append("")
        body += wrap(p, w - 4)
    h = len(body) + 7
    x, y = (W - w) // 2, (H - h) // 2
    popup_frame(g, x, y, w, h, "Dez · fixer")
    g.put(x + 2, y + 2, "a message · 22:00 · your flat, 18F", DIM)
    for j, line in enumerate(body):
        g.put(x + 2, y + 4 + j, line, TEXT)
    g.put(x + 2, y + h - 2, "space", CYAN, bold=True)
    g.put(x + 8, y + h - 2, "take the package", DIM)
    return g


def welcome_holo():
    g = home()
    wash(g)
    w, h = 90, 16
    x, y = (W - w) // 2, 12
    bracket_frame(g, x, y, w, h, MAGENTA)
    g.put(x + 3, y + 1, "BCI", MAGENTA, bold=True)
    g.put(x + 7, y + 1, "▸ incoming feed", DIM)
    g.put(x + w - 16, y + 1, "▂▄▆█", MAGENTA)
    g.put(x + w - 11, y + 1, "22:00", DIM)
    put_face(g, x + 3, y + 3, face(CYAN, MAGENTA))
    g.put(x + 3, y + 13, "DEZ", MAGENTA, bold=True)
    g.put(x + 7, y + 13, "· fixer", DIM)
    tx = x + 22
    body = []
    for i, p in enumerate(WELCOME):
        if i:
            body.append("")
        body += wrap(p, w - 26)
    for j, line in enumerate(body):
        g.put(tx, y + 3 + j, line, TEXT)
    g.put(tx, y + h - 3, "space", MAGENTA, bold=True)
    g.put(tx + 6, y + h - 3, "take the package", DIM)
    return g


def welcome_letterbox():
    g = home()
    wash(g, 5, 35, 79)
    # cinema bars over the map only; the HUD and sidebar stay
    g.fill(0, 5, 79, 4, BLACK)
    g.fill(0, 25, 79, 10, BLACK)
    g.put(4, 26, "DEZ", AMBER, bold=True)
    g.put(8, 26, "· fixer · on your rig", FAINT)
    body = []
    body += wrap(WELCOME[1], 70)
    for j, line in enumerate(body):
        g.put(4, 28 + j, line, TEXT)
    g.put(4 + len(body[-1]) + 1, 28 + len(body) - 1, "▌", AMBER)
    g.put(4, 33, "2/3", FAINT)
    g.put(64, 33, "space", AMBER, bold=True)
    g.put(70, 33, "next", DIM)
    return g


# ---------------------------------------------------------------- call 1: corner card


def card_ringing(g, blink=True):
    x, y, w, h = 1, 6, 36, 5
    bracket_frame(g, x, y, w, h, MAGENTA)
    g.put(x + 2, y + 1, "●" if blink else "○", MAGENTA, bold=True)
    g.put(x + 4, y + 1, "INCOMING · DEZ", MAGENTA, bold=True)
    g.put(x + w - 7, y + 1, "▂▄▆█", MAGENTA)
    g.put(x + 4, y + 2, "fixer · Velform corner", DIM)
    g.put(x + 4, y + 3, "space", MAGENTA, bold=True)
    g.put(x + 10, y + 3, "answer", DIM)
    g.put(x + 18, y + 3, "esc", MAGENTA, bold=True)
    g.put(x + 22, y + 3, "let it ring", DIM)


def card_talking(g):
    x, y, w, h = 1, 6, 62, 13
    bracket_frame(g, x, y, w, h, MAGENTA)
    g.put(x + 2, y + 1, "●", MAGENTA, bold=True)
    g.put(x + 4, y + 1, "DEZ", MAGENTA, bold=True)
    g.put(x + 8, y + 1, "· 00:07", DIM)
    g.put(x + w - 7, y + 1, "▂▄▆█", MAGENTA)
    put_face(g, x + 2, y + 2, face(CYAN, MAGENTA))
    tx = x + 20
    lines = []
    for i, p in enumerate(CALL[:2]):
        if i:
            lines.append("")
        lines += wrap("“%s”" % p, w - 23)
    for j, line in enumerate(lines):
        g.put(tx, y + 3 + j, line, TEXT)
    g.put(tx + len(lines[-1]), y + 3 + len(lines) - 1, "▌", MAGENTA)
    g.put(tx, y + h - 2, "space", MAGENTA, bold=True)
    g.put(tx + 6, y + h - 2, "next", DIM)
    g.put(tx + 12, y + h - 2, "esc", MAGENTA, bold=True)
    g.put(tx + 16, y + h - 2, "hang up", DIM)


# ---------------------------------------------------------------- call 2: the sidebar


def side_ringing(g, blink=True):
    x, w = 81, 39
    g.fill(x, 5, w, 1, MAG_DIM if blink else PANEL)
    g.put(x + 1, 5, "●", MAGENTA, MAG_DIM if blink else PANEL, bold=True)
    g.put(x + 3, 5, "DEZ CALLING", "#ffffff" if blink else MAGENTA, MAG_DIM if blink else PANEL, bold=True)
    g.put(x + 15, 5, "▂▄▆█", MAGENTA, MAG_DIM if blink else PANEL)
    g.put(x + w - 13, 5, "space answer", "#ffffff" if blink else DIM, MAG_DIM if blink else PANEL)


def side_talking(g):
    x, w = 81, 39
    g.fill(x, 5, w, 14, PANEL)
    g.fill(x, 5, w, 1, MAG_DIM)
    g.put(x + 1, 5, "●", MAGENTA, MAG_DIM, bold=True)
    g.put(x + 3, 5, "DEZ · 00:07", "#ffffff", MAG_DIM, bold=True)
    g.put(x + w - 5, 5, "▂▄▆█", MAGENTA, MAG_DIM)
    put_face(g, x + 1, 6, face(CYAN, MAGENTA))
    g.put(x + 18, 7, "fixer", DIM)
    g.put(x + 18, 8, "Velform corner", FAINT)
    g.put(x + 18, 10, "space", MAGENTA, bold=True)
    g.put(x + 24, 10, "next", DIM)
    g.put(x + 18, 11, "esc", MAGENTA, bold=True)
    g.put(x + 22, 11, "hang up", DIM)
    lines = wrap("“%s”" % CALL[1], w - 2)
    for j, line in enumerate(lines):
        g.put(x + 1, 15 + j, line, TEXT)
    g.put(x + 1 + len(lines[-1]), 15 + len(lines) - 1, "▌", MAGENTA)
    g.put(x, 18, "─" * w, "#283c4b")


# ---------------------------------------------------------------- call 3: the footer


def foot_ringing(g, blink=True):
    y = 37
    g.fill(0, y, 116, 2, PANEL)
    g.put(1, y, "●" if blink else "○", MAGENTA, bold=True)
    g.put(3, y, "DEZ", MAGENTA, bold=True)
    g.put(7, y, "is calling your BCI", DIM)
    g.put(28, y, "▂▄▆█", MAGENTA)
    g.put(1, y + 1, "space", MAGENTA, bold=True)
    g.put(7, y + 1, "answer", DIM)
    g.put(15, y + 1, "esc", MAGENTA, bold=True)
    g.put(19, y + 1, "let it ring", DIM)


def foot_talking(g):
    y = 37
    g.fill(0, y, 116, 2, PANEL)
    g.put(1, y, "●", MAGENTA, bold=True)
    g.put(3, y, "DEZ", MAGENTA, bold=True)
    lines = wrap("“%s”" % CALL[1], 100)
    for j, line in enumerate(lines):
        g.put(8, y + j, line, TEXT)
    g.put(8 + len(lines[-1]), y + len(lines) - 1, "▌", MAGENTA)
    g.put(1, y + 1, "00:07", FAINT)
    g.put(104, y + 1, "space", MAGENTA, bold=True)
    g.put(110, y + 1, "next", DIM)


# ---------------------------------------------------------------- the page


def frame(g, caption, scale_=0.82):
    return '<div class="shot"><div class="cap">%s</div>%s</div>' % (caption, g.html(scale_))


def call_pair(ring, talk):
    a = street()
    ring(a)
    b = street()
    talk(b)
    return a, b


def page():
    out = ["<style>%s" % CSS]
    out.append("""
.shot{margin:10px 0 22px}
.cap{font:13px/1.4 system-ui;color:#8a94a3;margin:0 0 6px}
.opt{border:1px solid #1d2733;border-radius:8px;padding:14px 16px;margin:0 0 26px;cursor:pointer}
.opt.selected{border-color:#00e5ff;box-shadow:0 0 0 1px #00e5ff}
.opt h3{margin:0 0 4px}
.opt p{margin:0 0 8px;color:#9aa4b2}
h2.sec{margin-top:36px}
</style>""")
    out.append("<h2>Story missions · round 1</h2>")
    out.append('<p class="subtitle">Drawn over the game\'s own frames at 120×40: VEX at home on '
               "18F for the welcome, then on the street with Marlo face down beside the stall "
               "for the call. Pick one look for each.</p>")

    out.append('<h2 class="sec">1 · The welcome, the moment the game starts</h2>')
    opts = [
        ("w-a", "A · The game's own window",
         "The popup every other window uses (the job offer, the chair), headed by the "
         "speaker. All three paragraphs at once; space closes it and the package is in "
         "your pack.", welcome_popup()),
        ("w-b", "B · A BCI feed with the caller's face",
         "Magenta brackets for anything that comes in through your head, a hologram "
         "portrait (scanlined, the chrome eye lit), the words beside it. The same frame "
         "the call uses, so the two read as one system.", welcome_holo()),
        ("w-c", "C · Letterbox",
         "Bars over the map like a film, the HUD left alone; one paragraph at a time "
         "typing out, space for the next. Most cinematic, slowest.", welcome_letterbox()),
    ]
    for key, title, text, g in opts:
        out.append('<div class="opt" data-choice="%s" onclick="toggleSelect(this)"><h3>%s</h3><p>%s</p>%s</div>'
                   % (key, title, text, frame(g, "")))

    out.append('<h2 class="sec">2 · The call, standing over the body</h2>')
    out.append('<p class="subtitle">It rings first (the dot blinks); space answers and the lines '
               "type out one by one, space for the next. The game keeps running underneath.</p>")
    calls = [
        ("c-1", "1 · A card in the map's corner",
         "Small while it rings, grows into face plus words when you answer. Like a holo "
         "call in the corner of your eye; it covers some of the map.", card_ringing, card_talking),
        ("c-2", "2 · The sidebar's head",
         "The call takes the top of the sidebar: a lit band while it rings, the face and "
         "the line under it when you answer. The map stays clear.", side_ringing, side_talking),
        ("c-3", "3 · Subtitles in the footer",
         "The footer's two empty rows carry it like film subtitles. Nothing covered, no "
         "face; the quietest of the three.", foot_ringing, foot_talking),
    ]
    for key, title, text, ring, talk in calls:
        a, b = call_pair(ring, talk)
        out.append('<div class="opt" data-choice="%s" onclick="toggleSelect(this)"><h3>%s</h3><p>%s</p>%s%s</div>'
                   % (key, title, text, frame(a, "ringing"), frame(b, "answered")))
    return "\n".join(out)


if __name__ == "__main__":
    open(sys.argv[1], "w").write(page())
