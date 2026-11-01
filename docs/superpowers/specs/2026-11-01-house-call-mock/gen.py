"""House call, round 1: three ways to play the cinematic at your door, each as four
storyboard frames over the game's own frames.

    SHOTS=dir/ python3 -I gen.py out.html

SHOTS holds hc_walkway.svg, hc_keypad.svg and hc_flat.svg (120x40), from a throwaway
shot test: the walkway outside the home flat on 18F with the player at the door, the
keypad over it, and the flat seen whole with the player a step inside.

A · Film: the HUD goes; black bars, the map framed in the middle, subtitles below.
B · Bubbles: the HUD stays, washed; lines float over the speakers on the map.
C · Script: the side panel turns into the scene's screenplay, line by line."""

import copy
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")
W, H = 120, 40
MAP_W = 79  # the map's columns, 0..78; the rule is 79 and the side panel 80..119
MAP_Y0, MAP_Y1 = 5, 35

BLACK = "#000000"
PANEL = "#04060a"
TEXT = "#cdd4de"
DIM = "#a0aab9"
FAINT = "#5c6673"
WHITE = "#ffffff"
AMBER = "#ffc850"
MAGENTA = "#ff00c8"
RED = "#ff4f5e"
CYAN = "#00e5ff"

WHO = {
    "VEX": WHITE,
    "A VOICE": DIM,
    "CASTELLAN": AMBER,
    "SEREN": MAGENTA,
}

# The four beats: what you see, then who speaks.
BEATS = [
    {
        "title": "1 · the code",
        "stage": "You key your code. The pad buzzes red.",
        "heading": "18F WALKWAY · NIGHT",
        "who": "VEX", "line": "WTF…?",
    },
    {
        "title": "2 · the door",
        "stage": "The lock clicks from inside. The door slides open on a dark flat.",
        "who": "A VOICE", "line": "Come in, Vex.",
    },
    {
        "title": "3 · the couch",
        "stage": "You step in. A man sits on your couch, waiting.",
        "heading": "YOUR FLAT · DARK",
        "who": "CASTELLAN", "line": "Sit down. It's your couch, but sit anyway.",
    },
    {
        "title": "4 · the rig",
        "stage": "From the bedroom, a woman, the rig's cable still at her neck.",
        "who": "SEREN", "line": "Your fixer's line leaks. I was on it the whole walk home.",
    },
]

# Cells in the frames (the player's cell is the camera's middle, 39,20).
PLAYER = (39, 20)
DOOR_OUT = (39, 21)   # the front door, below you on the walkway
DOOR_IN = (39, 19)    # the same door, above you inside
STEP_IN = (39, 22)    # two steps into the living room
CASTELLAN = (39, 26)  # on the couch, facing the door
SEREN_AT = (35, 23)   # just through from the hall
SEREN_WALK = [(29, 25), (30, 25), (31, 25), (31, 24), (31, 23), (32, 23), (33, 23), (34, 23)]
FLAT = (21, 19, 58, 28)  # x0, y0, x1, y1 of the flat in hc_flat, walls included


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def scale(c, f):
    r, g, b = hexrgb(c)
    return "#%02x%02x%02x" % (int(r * f), int(g * f), int(b * f))


def dark(g, x0, y0, x1, y1, f):
    for y in range(y0, y1):
        for x in range(x0, x1):
            cell = g.c[y][x]
            cell[1], cell[2] = scale(cell[1], f), scale(cell[2], f)


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


def load(name):
    return Grid.from_svg(os.path.join(S, f"{name}.svg"), W, H)


def floor_at(g, x, y):
    g.c[y][x][0], g.c[y][x][1] = "·", scale(DIM, 0.5)


def person(g, at, colour):
    g.c[at[1]][at[0]][0] = "@"
    g.c[at[1]][at[0]][1] = colour
    g.c[at[1]][at[0]][3] = True


def open_door(g, at):
    g.c[at[1]][at[0]] = ["·", AMBER, scale(AMBER, 0.25), False]


# ---- the four pictures, the same in every concept

def picture(i):
    if i == 0:
        g = load("hc_keypad")
        g.put(51, 14, "[0][7][5][7]", RED, bold=True)
        g.put(51, 15, "DENIED      ", RED, bold=True)
        g.c[11][69][1] = RED
        return g
    if i == 1:
        g = load("hc_walkway")
        flat = load("hc_flat")
        x0, y0, x1, y1 = FLAT
        floor_at(flat, *PLAYER)
        dark(flat, x0, y0, x1, y1, 0.32)
        # The flat seen through the open door, two rows lower from out here.
        for y in range(y0 + 1, y1):
            for x in range(x0, x1):
                g.c[y + 2][x] = list(flat.c[y][x])
        open_door(g, DOOR_OUT)
        for y, f in ((22, 0.55), (23, 0.4), (24, 0.3)):
            for x in range(38, 41):
                c = g.c[y][x]
                c[2] = scale(AMBER, 0.12 * f)
                c[1] = scale(DIM, f + 0.2)
        return g
    g = load("hc_flat")
    x0, y0, x1, y1 = FLAT
    dark(g, x0, y0 + 1, x1, y1, 0.42)
    open_door(g, DOOR_IN)
    floor_at(g, *PLAYER)
    for y, f in ((20, 0.5), (21, 0.4), (22, 0.3)):
        for x in range(38, 41):
            g.c[y][x][2] = scale(AMBER, 0.12 * f)
    person(g, STEP_IN, WHITE)
    person(g, CASTELLAN, AMBER)
    if i == 3:
        for x, y in SEREN_WALK:
            g.c[y][x][0], g.c[y][x][1] = "·", scale(MAGENTA, 0.45)
        person(g, SEREN_AT, MAGENTA)
    return g


def speaker_cell(i):
    return [PLAYER, DOOR_OUT, CASTELLAN, SEREN_AT][i]


# ---- A · film

def film(i):
    pic = picture(i)
    g = Grid(W, H, BLACK)
    off = (W - MAP_W) // 2
    for y in range(MAP_Y0, MAP_Y1):
        for x in range(MAP_W):
            g.c[y][x + off] = list(pic.c[y][x])
    sx, sy = speaker_cell(i)
    if i != 1:
        g.put(sx + off, sy - 1, "▾", WHO[BEATS[i]["who"]], bold=True)
    g.put(off, 1, "HOUSE CALL", scale(MAGENTA, 0.6), bold=True)
    b = BEATS[i]
    # What happens, in the top bar: the stage line, centred and dim (Jeffrey, round 1).
    g.put((W - len(b["stage"])) // 2, 3, b["stage"], DIM)
    who, line = b["who"], b["line"]
    text = f"{who}  {line}"
    x = (W - len(text)) // 2
    g.put(x, 37, who, WHO[who], bold=True)
    g.put(x + len(who) + 2, 37, line, WHITE)
    hint = "space next · esc skip"
    g.put(off + MAP_W - len(hint), 39, hint, FAINT)
    return g


# ---- B · bubbles

def wash(g, x0, y0, x1, y1, f=0.35):
    dark(g, x0, y0, x1, y1, f)


def bubble(g, x, y, who, text, width, tail):
    colour = WHO[who]
    lines = wrap(text, width - 4)
    h = len(lines) + 2
    g.fill(x, y, width, h, PANEL)
    top = "╭─ " + who + " " + "─" * (width - len(who) - 5) + "╮"
    g.put(x, y, top, colour)
    g.put(x + 3, y, who, colour, bold=True)
    for j, l in enumerate(lines):
        g.put(x, y + 1 + j, "│", colour)
        g.put(x + 2, y + 1 + j, l, TEXT)
        g.put(x + width - 1, y + 1 + j, "│", colour)
    g.put(x, y + h - 1, "╰" + "─" * (width - 2) + "╯", colour)
    tx, ty, ch = tail
    g.put(tx, ty, ch, colour, bold=True)


def bubbles(i):
    g = picture(i)
    wash(g, 0, 0, W, MAP_Y0 - 1)
    wash(g, MAP_W + 1, MAP_Y0, W, MAP_Y1)
    g.fill(0, 36, W, 4, PANEL)
    g.put(1, 36, "cinematic · house call", scale(MAGENTA, 0.8), bold=True)
    g.put(1, 39, "space next   esc skip", DIM)
    b = BEATS[i]
    stage = " ▸ " + b["stage"] + " "
    g.put(1, 6, stage, DIM, bg=PANEL)
    if i == 0:
        bubble(g, 24, 16, "VEX", b["line"], 16, (39, 19, "▾"))
    elif i == 1:
        bubble(g, 41, 22, "A VOICE", b["line"], 22, (40, 22, "◂"))
    elif i == 2:
        bubble(g, 42, 25, "CASTELLAN", b["line"], 30, (41, 26, "◂"))
    else:
        bubble(g, 4, 17, "SEREN", b["line"], 32, (35, 22, "▾"))
    return g


# ---- C · script

def script_lines(upto):
    out = []
    for j, b in enumerate(BEATS[: upto + 1]):
        now = j == upto
        f = 1.0 if now else 0.5
        if "heading" in b:
            if out:
                out.append(("", FAINT, False))
            out.append((b["heading"], scale(CYAN, 0.8 * f), True))
        for l in wrap(b["stage"], 36):
            out.append((l, scale(DIM, 0.85 * f), False))
        out.append(("", FAINT, False))
        out.append(((" " * 8) + b["who"], scale(WHO[b["who"]], f), True))
        for k, l in enumerate(wrap(b["line"], 28)):
            mark = "▸ " if now and k == 0 else "  "
            out.append(((" " * 4) + mark + l, scale(TEXT if now else DIM, f), now))
        out.append(("", FAINT, False))
    return out


def script(i):
    g = picture(i)
    wash(g, 0, 0, MAP_W, MAP_Y0 - 1, 0.5)
    x0 = MAP_W + 1
    g.fill(x0, MAP_Y0, W - x0, MAP_Y1 - MAP_Y0, PANEL)
    g.put(x0 + 1, MAP_Y0, "▌HOUSE CALL", MAGENTA, bold=True)
    g.put(x0 + 1, MAP_Y0 + 1, "─" * (W - x0 - 2), scale(MAGENTA, 0.4))
    lines = script_lines(i)
    room = MAP_Y1 - MAP_Y0 - 3
    for j, (l, c, bold) in enumerate(lines[-room:]):
        g.put(x0 + 1, MAP_Y0 + 2 + j, l, c, bold=bold)
    sx, sy = speaker_cell(i)
    if i != 1:
        g.put(sx, sy - 1, "▾", WHO[BEATS[i]["who"]], bold=True)
    g.fill(0, 36, W, 4, PANEL)
    g.put(1, 36, "cinematic · house call", scale(MAGENTA, 0.8), bold=True)
    g.put(1, 39, "space next   esc skip", DIM)
    return g


CONCEPTS = [
    ("A", "Film", film,
     "The HUD and the side panel go. Black bars close in, the map sits framed in the "
     "middle, and the lines run as subtitles under it; what happens reads dim in the top "
     "bar (Jeffrey's pick, with B's stage line added). A small marker sits over whoever "
     "speaks."),
    ("B", "Bubbles", bubbles,
     "Nothing moves out of the way: the HUD and the panel only dim. Each line floats "
     "beside its speaker on the map with a tail pointing at them, and a faint line in "
     "the map's corner says what happens. It stays closest to playing the game."),
    ("C", "Script", script,
     "The map stays put and the side panel becomes the scene's screenplay: a heading, "
     "what happens, then who speaks, typed out line by line. Earlier lines stay above, "
     "dimmed, so you can read back. The HUD dims; the footer shows the keys."),
]


def page(only=None):
    out = [
        "<!doctype html><meta charset='utf-8'><title>House call · round 1</title><style>",
        CSS,
        "body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;"
        "margin:24px auto;max-width:1240px;padding:0 16px}"
        "h1{font-size:22px;margin:0 0 4px}h2{font-size:18px;margin:36px 0 4px;color:#fff}"
        "p{max-width:820px;color:#a0aab9}.grid{display:grid;grid-template-columns:1fr 1fr;"
        "gap:18px}.cap{font-size:13px;color:#a0aab9;margin:6px 0 0}"
        ".fr{overflow:hidden}@media(max-width:900px){.grid{grid-template-columns:1fr}}",
        "</style>",
        "<h1>House call · the cinematic at your door</h1>",
        "<p>You walk up to your door with the story waiting, and the game takes over for a "
        "moment: it keys your code, the pad buzzes red, the door opens on a dark flat, you "
        "walk in, Castellan waits on the couch, Seren comes out of the bedroom from your "
        "rig. Three ways to show it, four frames each, over the game's own frames at "
        "120×40. Keys in every one: <b>space</b> moves to the next beat, <b>esc</b> skips "
        "the scene.</p>",
    ]
    if only:
        out = out[:-2]
    for key, name, fn, words in CONCEPTS:
        if only and key != only:
            continue
        out.append(f"<h2>{key} · {name}</h2><p>{words}</p><div class='grid'>")
        for i, b in enumerate(BEATS):
            out.append("<div class='fr'>" + fn(i).html(0.56) + f"<div class='cap'>{b['title']}</div></div>")
        out.append("</div>")
    return "\n".join(out)


if __name__ == "__main__":
    # gen.py out.html [A|B|C]: the whole page, or one concept alone (for a phone).
    only = sys.argv[2] if len(sys.argv) > 2 else None
    open(sys.argv[1] if len(sys.argv) > 1 else "round1.html", "w").write(page(only))
