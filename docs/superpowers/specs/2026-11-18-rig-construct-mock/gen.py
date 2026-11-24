"""#104 mock: your rig's own construct, drawn over the game's own Net frame at 120x40.

    python3 -I gen.py            writes rig-construct.html beside it

Run from this folder. `shots/` holds the frames from the shot tests net_open,
site_net_shots and desk_stock (seed 8).

One moment in every look: VEX jacked in at home on a Civitas cabinet (Tinman 2X core,
Switchback relay, Longwire line, Sandbag firewall), three backdoors held of five lines,
`sift` running on a paydata shard, and three sticks in the reader: an open credstick,
a locked one, and a black stick with its ICE coiled round it.

A · the room: one floor plan, the rig's parts as rooms, the backdoors as doors.
B · the rack: the rack in section, a level per bay, built like a site's open tower.
C · the rings: you at the core, the sticks round you, the lines as spokes.
"""

import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SPECS = os.path.join(HERE, "..")
sys.path.insert(0, os.path.join(SPECS, "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(SPECS, "2026-11-02-minimap-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402

SHOTS = os.path.join(HERE, "shots")

# the Net, as the game draws it (render/netlook)
VOID = "#010204"
DOT = "#0d2a24"
LEVEL = "#01191d"
WALL = "#00b7cc"
WALL_BG = "#01181c"
TRACE = "#01705e"
FOLD = "#006773"
FOLD_BG = "#010d11"
FW = "#ff2848"
FW_BG = "#1f070c"
ICE_BLUE = "#9fe6ff"
WHITE = "#e6f0f5"
AMBER = "#ffb000"
TERMINAL = "#8c96ff"
YOU = "#468cff"
YOU_GLOW = "#0e2254"
RED = "#ff2848"
# the rig's kinds, as the desk tints its sleds
CORE = "#00e5ff"
RELAY = "#50e678"
LINE = "#ffb000"
# the panel
PANEL = "#04060a"
TEXT = "#cdd4de"
DIM = "#8a93a3"
FAINT = "#4a525e"
RULE = "#283c4b"
LABEL = "#a0aab9"

MX, MY, MW, MH = 0, 5, 79, 30
PX = 81

BOXES = {
    "light": "┌┐└┘─│",
    "double": "╔╗╚╝═║",
    "heavy": "┏┓┗┛━┃",
    "round": "╭╮╰╯─│",
    "dash": "┌┐└┘┄┆",
}


def put(g, x, y, s, fg, bg=None, bold=False):
    g.put(x, y, s, fg, bg, bold)


def rput(g, x, y, s, fg, bg=None, bold=False):
    """Right-aligned so the text ends at column x."""
    g.put(x - len(s) + 1, y, s, fg, bg, bold)


def fill(g, x0, y0, x1, y1, bg):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if 0 <= x < g.w and 0 <= y < g.h:
                g.c[y][x] = [" ", TEXT, bg, False]


def box(g, x0, y0, x1, y1, fg, bg=None, style="light", bold=False):
    tl, tr, bl, br, h, v = BOXES[style]
    for x in range(x0 + 1, x1):
        put(g, x, y0, h, fg, bg, bold)
        put(g, x, y1, h, fg, bg, bold)
    for y in range(y0 + 1, y1):
        put(g, x0, y, v, fg, bg, bold)
        put(g, x1, y, v, fg, bg, bold)
    put(g, x0, y0, tl, fg, bg, bold)
    put(g, x1, y0, tr, fg, bg, bold)
    put(g, x0, y1, bl, fg, bg, bold)
    put(g, x1, y1, br, fg, bg, bold)


def hline(g, x0, x1, y, ch, fg, bg=None, bold=False):
    for x in range(x0, x1 + 1):
        put(g, x, y, ch, fg, bg, bold)


def vline(g, x, y0, y1, ch, fg, bg=None, bold=False):
    for y in range(y0, y1 + 1):
        put(g, x, y, ch, fg, bg, bold)


def title_in(g, x0, x1, y, s, fg, bg=None, bold=True):
    """A label set into a border, centred between x0 and x1."""
    s = " %s " % s
    put(g, x0 + (x1 - x0 + 1 - len(s)) // 2, y, s, fg, bg, bold)


def void(g):
    """The map pane as the void with its grid of dots."""
    fill(g, MX, MY, MX + MW - 1, MY + MH - 1, VOID)
    for y in range(MY, MY + MH):
        for x in range(MX, MX + MW):
            if (x % 4 == 1) and ((y - MY) % 2 == 0):
                put(g, x, y, "·", DOT)


# ------------------------------------------------------------------ the frame

def frame():
    g = Grid.from_svg(os.path.join(SHOTS, "net_open.svg"), 120, 40)
    void(g)
    fill(g, 80, MY, 119, MY + MH - 1, PANEL)
    for y in range(MY, MY + MH):
        put(g, 79, y, "│", RULE, PANEL)
    # the top band's right side
    fill(g, 81, 0, 119, 0, PANEL)
    put(g, 81, 0, "zenbo · your rig", AMBER, PANEL, True)
    put(g, 113, 0, "☾ 22:00", TEXT, PANEL)
    fill(g, 81, 2, 119, 2, PANEL)
    put(g, 81, 2, "construct · unseen", DIM, PANEL)
    # the footer
    fill(g, 0, 36, 119, 36, PANEL)
    put(g, 1, 36, "⚡", AMBER, PANEL)
    put(g, 4, 36, "zenbo · your rig", TEXT, PANEL)
    put(g, 48, 36, "TRACE", DIM, PANEL)
    put(g, 54, 36, "▱" * 10 + " 0", FAINT, PANEL)
    put(g, 117, 36, "[", "#788291", PANEL)
    put(g, 118, 36, "?", CORE, PANEL, True)
    put(g, 119, 36, "]", "#788291", PANEL)
    return g


MAGENTA = "#ff00c8"
HUD_RED = "#ff2828"


def hp(g, cur, mx):
    """The HUD's HP row at `cur` of `mx`: the gone part dims, as `bar_spans` draws it."""
    fill(g, 1, 1, 78, 1, PANEL)
    put(g, 1, 1, "HP", LABEL, PANEL)
    rput(g, 13, 1, "%d/%d" % (cur, mx), TEXT, PANEL)
    full = round(63 * cur / mx)
    put(g, 15, 1, "█" * full, RELAY, PANEL)
    put(g, 15 + full, 1, "░" * (63 - full), "#285c3c", PANEL)


def slots_row(g, slots):
    """The footer's PROGRAMS row: `slots` are names, None for an empty slot."""
    fill(g, 0, 39, 119, 39, PANEL)
    put(g, 1, 39, "PROGRAMS", LABEL, PANEL)
    x = 12
    for i, name in enumerate(slots):
        if name:
            put(g, x, 39, "[%d]" % (i + 1), CORE, PANEL, True)
            put(g, x + 4, 39, name, TEXT, PANEL)
            put(g, x + 5 + len(name), 39, "●", CORE, PANEL)
            x += 4 + len(name) + 2 + 4
        else:
            put(g, x, 39, "[%d]" % (i + 1), LABEL, PANEL)
            put(g, x + 4, 39, "──", LABEL, PANEL)
            x += 4 + 2 + 4


def footer_fight(g, prompt, keys, turn, ice):
    """The footer on turns: ENGAGED, the prompt, the turn order with the ICE in sight,
    and the pools on the move half."""
    rput(g, 112, 36, "ENGAGED", MAGENTA, PANEL, True)
    fill(g, 0, 37, 119, 38, PANEL)
    put(g, 1, 37, "▸ " + prompt, CORE, PANEL, True)
    put(g, 4 + len(prompt) + 3, 37, keys, DIM, PANEL)
    put(g, 1, 38, turn, CORE, PANEL, True)
    rput(g, 119, 38, ice, HUD_RED, PANEL)
    slots_row(g, ["CRACK", "ATTACK", "SHIELD", None, None])
    # MOV ▰▰▰▱ 3/4   AP ●● 2/2, the move half's badge lit
    pools = [(" MOV ", True), ("▰▰▰▱", None), (" 3/4", False), ("   ", False),
             (" AP ", False), ("●●", None), (" 2/2", False)]
    w = sum(len(s) for s, _ in pools)
    x = 120 - w
    for s, kind in pools:
        if kind is True:
            put(g, x, 39, s, "#000000", CORE, True)
        elif kind is None:
            put(g, x, 39, s, CORE, PANEL)
        else:
            put(g, x, 39, s, TEXT if s.strip()[:1].isdigit() else DIM, PANEL)
        x += len(s)


def rule(g, y):
    hline(g, 80, 119, y, "─", RULE, PANEL)


def panel(g, here, black=("MASTIFF", RED), seat2=None, body=None, sticks=None,
          held=None):
    """The rig's panel, the same in every look; `here` is the line about what's under
    you, `black` the black stick's tail, `seat2` the second seat's job if it has one,
    `body` the BODY line's words, `sticks` the reader's rows when they aren't the three
    of rounds 1-3, `held` a line's name whose tail reads held."""
    y = MY
    put(g, PX, y, "ZENBO", AMBER, PANEL, True)
    put(g, PX + 6, y, "· your rig", TEXT, PANEL)
    y += 1
    put(g, PX, y, "Civitas cabinet · 4 bays", DIM, PANEL)
    y += 1
    put(g, PX, y, "FIREWALL", FW, PANEL, True)
    put(g, PX + 9, y, "▮▯▯", FW, PANEL)
    put(g, PX + 13, y, "sandbag", DIM, PANEL)
    y += 1
    rule(g, y)
    y += 1
    put(g, PX, y, "LINES", LABEL, PANEL, True)
    put(g, PX + 6, y, "3/5", TEXT, PANEL)
    put(g, PX + 10, y, "· hold 5 days", DIM, PANEL)
    y += 1
    lines = [
        ("▯", RELAY, "Kiroflux Mart", "net · 2d"),
        ("▯", RELAY, "Hextek Holdings", "site · 4d"),
        ("▯", RELAY, "Stack 101", "key · 6h"),
        ("▯", FAINT, "free line", ""),
        ("▯", FAINT, "free line", ""),
    ]
    for glyph, col, name, tail in lines:
        put(g, PX + 1, y, glyph, col, PANEL, True)
        put(g, PX + 3, y, name, TEXT if col != FAINT else FAINT, PANEL)
        if name == held:
            rput(g, 117, y, tail.split(" · ")[0] + " · held", CORE, PANEL)
        else:
            rput(g, 117, y, tail, DIM, PANEL)
        y += 1
    put(g, PX + 1, y, "▸", ICE_BLUE, PANEL, True)
    put(g, PX + 3, y, "the grid", TEXT, PANEL)
    rput(g, 117, y, "your block", DIM, PANEL)
    y += 1
    rule(g, y)
    y += 1
    put(g, PX, y, "READER", LABEL, PANEL, True)
    put(g, PX + 7, y, "%d/3" % sum(1 for s in sticks or [0, 0, 0]
                                    if s == 0 or s[2] != "free slot"), TEXT, PANEL)
    y += 1
    sticks = sticks or [
        ("¤", AMBER, "Credstick", "¤340 · open", DIM),
        ("▣", ICE_BLUE, "Locked stick", "2 locks · 58%", DIM),
        ("◆", RED if black[1] == RED else AMBER, "Black stick", black[0], black[1]),
    ]
    for glyph, col, name, tail, tcol in sticks:
        put(g, PX + 1, y, glyph, col, PANEL, True)
        put(g, PX + 3, y, name, TEXT if col != FAINT else FAINT, PANEL)
        rput(g, 117, y, tail, tcol, PANEL)
        y += 1
    rule(g, y)
    y += 1
    put(g, PX, y, "SEATS", LABEL, PANEL, True)
    put(g, PX + 6, y, "2/2" if seat2 else "1/2", TEXT, PANEL)
    put(g, PX + 10, y, "· Tinman 2X", DIM, PANEL)
    y += 1
    put(g, PX + 1, y, "◉", CORE, PANEL, True)
    put(g, PX + 3, y, "sift · Paydata shard", TEXT, PANEL)
    rput(g, 117, y, "4h", DIM, PANEL)
    y += 1
    if seat2:
        put(g, PX + 1, y, "◉", CORE, PANEL, True)
        put(g, PX + 3, y, seat2[0], TEXT, PANEL)
        rput(g, 117, y, seat2[1], DIM, PANEL)
        y += 1
    rule(g, y)
    y += 1
    for line in here:
        x = PX
        for s, col, *b in line if isinstance(line, list) else [line]:
            put(g, x, y, s, col, PANEL, bool(b and b[0]))
            x += len(s)
        y += 1
    y = MY + MH - 2
    put(g, PX, y, "BODY", LABEL, PANEL, True)
    x = PX + 5
    for s, col in body or [("at home", DIM)]:
        put(g, x, y, s, col, PANEL)
        x += len(s)
    y += 1
    put(g, PX, y, "[F5]Inspect [F6]Legend", DIM, PANEL)


HERE_LOCKED = [
    ("▣ Locked stick · ¤1,200 inside", TEXT),
    ("  2 locks · crack 58% · space", DIM),
]


# ------------------------------------------------------------------ shared pieces

def boss(g, x, y):
    """MASTIFF: black ICE three cells square, coiled round the stick at (x+1, y+1)."""
    rows = ["▛▀▜", "▌◆▐", "▙▄▟"]
    for j, r in enumerate(rows):
        for i, ch in enumerate(r):
            put(g, x + i, y + j, ch, RED, "#2a0710", True)
    put(g, x + 1, y + 1, "◆", WHITE, "#2a0710", True)


def you(g, x, y, bg=LEVEL):
    put(g, x, y, "@", YOU, YOU_GLOW, True)


def cord(g, pts, bg):
    """A dashed cord through the cells `pts`."""
    for (x, y), nxt in zip(pts, pts[1:] + [None]):
        prev = None
        put(g, x, y, "┄" if nxt is None or nxt[1] == y else "┆", "#2f6aa8", bg)


def lane_out(g, x0, x1, y, col, bg=VOID):
    hline(g, x0, x1, y, "═", col, bg)


# ------------------------------------------------------------------ A · the room

def slot(g, x, y, glyph, col, name, tail, bg=LEVEL, tail_col=None, border="#0b5560"):
    """A stick on its slot: a small box with the stick in it, its name and state under."""
    box(g, x, y, x + 6, y + 4, border, bg, "light")
    put(g, x + 3, y + 2, glyph, col, bg, True)
    put(g, x + (7 - len(name)) // 2, y + 5, name, TEXT, bg)
    tail_col = tail_col or (RED if glyph == "◆" else DIM)
    put(g, x + (7 - len(tail)) // 2, y + 6, tail, tail_col, bg)


# MASTIFF awake: its coat is the eight cells round its core, a plate a cell
COAT = [(0, 0, "▛"), (1, 0, "▀"), (2, 0, "▜"), (2, 1, "▐"),
        (2, 2, "▟"), (1, 2, "▄"), (0, 2, "▙"), (0, 1, "▌")]
ICE_BG = "#2a0710"
BROKEN = "#8a2236"
TINT = "#16080c"
LEASH = "#6a1a28"
BITE = "#ff4060"
BITE_BG = "#4a0c18"


def mastiff(g, x, y, broken=()):
    """MASTIFF off its stick, its broken plates gone to grit."""
    for i, (dx, dy, ch) in enumerate(COAT):
        if i in broken:
            put(g, x + dx, y + dy, "░", BROKEN, ICE_BG)
        else:
            put(g, x + dx, y + dy, ch, RED, ICE_BG, True)
    put(g, x + 1, y + 1, "◆", WHITE, ICE_BG, True)


def leash(g, x0, y0, x1, y1):
    """The ground MASTIFF can reach from its stick: tinted, with a dashed edge on the
    sides that face the hall."""
    watched(g, x0, y0, x1, y1)
    vline(g, x0, y0, y1 - 1, "┆", LEASH, TINT)
    hline(g, x0 + 1, x1, y1, "┄", LEASH, TINT)
    put(g, x0, y1, "└", LEASH, TINT)


def watched(g, x0, y0, x1, y1, bg=LEVEL):
    """The black ICE's watch: a red tint on the open cells round it."""
    for yy in range(y0, y1 + 1):
        for xx in range(x0, x1 + 1):
            c = g.c[yy][xx]
            if c[2] == bg:
                c[2] = "#16080c"


def look_a(state="calm"):
    """The room. `state`: calm (MASTIFF coiled on its stick), fight (turn 4, the bite
    marked) or after (MASTIFF broken, the stick open)."""
    g = frame()
    # the firewall ring
    x0, y0, x1, y1 = 4, MY, 74, MY + MH - 2
    fill(g, x0, y0, x1, y1, FW_BG)
    box(g, x0, y0, x1, y1, FW, FW_BG, "double", True)
    title_in(g, x0, x1, y0, "FIREWALL · sandbag ▮▯▯", FW, FW_BG)
    # the floor inside
    fx0, fy0, fx1, fy1 = 6, MY + 1, 72, MY + MH - 3
    fill(g, fx0, fy0, fx1, fy1, LEVEL)
    box(g, fx0, fy0, fx1, fy1, WALL, WALL_BG, "double")

    # PORTS · the relay's room, west
    px1 = 25
    vline(g, px1, fy0 + 1, fy1 - 1, "│", TRACE, LEVEL)
    put(g, px1, fy0, "╤", WALL, WALL_BG)
    put(g, px1, fy1, "╧", WALL, WALL_BG)
    put(g, fx0 + 2, fy0 + 1, "PORTS", RELAY, LEVEL, True)
    put(g, fx0 + 2, fy0 + 2, "Switchback · 5", DIM, LEVEL)
    ports = [
        ("Kiroflux Mart", "2d left", True),
        ("Hextek Holdings", "4d left", True),
        ("Stack 101", "6h left", True),
        ("free line", "", False),
        ("free line", "", False),
    ]
    for i, (name, left, held) in enumerate(ports):
        y = fy0 + 5 + i * 4
        col = RELAY if held else FAINT
        put(g, fx0, y, "▯", col, WALL_BG, True)
        # a held line runs out through the firewall to its building
        if held:
            lane_out(g, 0, fx0 - 1, y, RELAY, VOID)
            put(g, x0, y, "╬", FW, FW_BG, True)
        put(g, fx0 + 2, y, name, TEXT if held else FAINT, LEVEL)
        if held:
            put(g, fx0 + 2, y + 1, left, DIM, LEVEL)
        if i < 4:
            hline(g, fx0 + 1, px1 - 1, y + 2, "┄", "#0b3a35", LEVEL)

    # SEATS · the core's room, east
    sx0 = 53
    vline(g, sx0, fy0 + 1, fy1 - 1, "│", TRACE, LEVEL)
    put(g, sx0, fy0, "╤", WALL, WALL_BG)
    put(g, sx0, fy1, "╧", WALL, WALL_BG)
    put(g, sx0 + 2, fy0 + 1, "SEATS", CORE, LEVEL, True)
    put(g, sx0 + 2, fy0 + 2, "Tinman 2X · 2", DIM, LEVEL)
    # seat one: sift at work on the paydata shard
    box(g, sx0 + 2, fy0 + 5, sx0 + 17, fy0 + 9, CORE, LEVEL, "round")
    put(g, sx0 + 4, fy0 + 6, "◉ sift", CORE, LEVEL, True)
    put(g, sx0 + 4, fy0 + 7, "Paydata shard", TEXT, LEVEL)
    put(g, sx0 + 4, fy0 + 8, "▰▰▰▱▱ 4h", DIM, LEVEL)
    if state in SUMMON + TERM:
        job = SEAT2.get(state)
        if job:
            # seat two: the job round 4 summoned, or the one the shell ran in its place
            box(g, sx0 + 2, fy0 + 11, sx0 + 17, fy0 + 15, CORE, LEVEL, "round")
            put(g, sx0 + 4, fy0 + 12, "◉ " + job[0], CORE, LEVEL, True)
            put(g, sx0 + 4, fy0 + 13, job[1], TEXT, LEVEL)
            put(g, sx0 + 4, fy0 + 14, job[2], DIM, LEVEL)
        else:
            box(g, sx0 + 2, fy0 + 12, sx0 + 17, fy0 + 14, "#0b5560", LEVEL, "round")
            put(g, sx0 + 4, fy0 + 13, "free seat", FAINT, LEVEL)
    elif state == "fight":
        # seat two joins the fight: it runs your Attack at MASTIFF on its own turn
        box(g, sx0 + 2, fy0 + 11, sx0 + 17, fy0 + 15, CORE, LEVEL, "round")
        put(g, sx0 + 4, fy0 + 12, "◉ attack", CORE, LEVEL, True)
        put(g, sx0 + 4, fy0 + 13, "MASTIFF", TEXT, LEVEL)
        put(g, sx0 + 4, fy0 + 14, "seat 2 · 55%", DIM, LEVEL)
    else:
        # seat two: free
        box(g, sx0 + 2, fy0 + 12, sx0 + 17, fy0 + 14, "#0b5560", LEVEL, "round")
        put(g, sx0 + 4, fy0 + 13, "free seat", FAINT, LEVEL)

    # the hall: the reader and its sticks
    put(g, 28, fy0 + 1, "READER", AMBER, LEVEL, True)
    put(g, 35, fy0 + 1, "· 3 slots", DIM, LEVEL)
    sy = fy0 + 4
    if state in SUMMON + TERM:
        hall_summon(g, state, sy)
    else:
        slot(g, 27, sy, "¤", AMBER, "Credstick", "¤340")
        slot(g, 36, sy, "▣", ICE_BLUE, "Locked", "2 locks")
    if state in SUMMON + TERM:
        pass
    elif state == "calm":
        slot(g, 45, sy, "◆", RED, "Black", "MASTIFF")
        boss(g, 47, sy + 1)
        watched(g, 43, sy - 2, 52, sy + 8)
        # you at the locked stick
        you(g, 39, sy + 8)
    elif state == "fight":
        # MASTIFF off its stick on its leash; three plates of its coat broken on the
        # side you hit from
        leash(g, 43, sy - 2, 52, sy + 14)
        slot(g, 45, sy, "◆", RED, "Black", "awake")
        put(g, 48, sy + 7, "┇", RED, TINT, True)
        mastiff(g, 47, sy + 8, broken=(0, 6, 7))
        # the bite it's winding up: the tiles it hits next turn
        for yy in range(sy + 8, sy + 11):
            for xx in range(44, 47):
                put(g, xx, yy, "×", BITE, BITE_BG)
        put(g, 44, sy + 9, "@", YOU, BITE_BG, True)
        # seat two's Attack, across the wall from the seats
        put(g, 50, sy + 9, "◂", CORE, TINT, True)
        hline(g, 51, 52, sy + 9, "━", CORE, TINT)
        hline(g, 53, 54, sy + 9, "━", CORE, LEVEL)
        put(g, 55, sy + 9, "┤", CORE, LEVEL)
    else:
        # MASTIFF broken: grit where it stood, the stick open
        slot(g, 45, sy, "◆", AMBER, "Black", "¤4,800", tail_col=DIM)
        for dx, dy, ch in [(0, 8, "▝"), (2, 8, "░"), (1, 9, "·"), (0, 10, "▖"),
                           (2, 10, "▗"), (3, 9, "·"), (-1, 9, "·")]:
            put(g, 47 + dx, sy + dy, ch, BROKEN, LEVEL)
        you(g, 48, sy + 7)
    # LINE: the trunk out of the hall to the grid
    put(g, 27, fy1 - 1, "LINE", LINE, LEVEL, True)
    put(g, 32, fy1 - 1, "Longwire · 5d", DIM, LEVEL)
    put(g, 41, fy1 - 1, "", DIM, LEVEL)
    put(g, 39, fy1, "▯", ICE_BLUE, WALL_BG, True)
    vline(g, 39, fy1 + 1, y1 - 1, "║", ICE_BLUE, FW_BG)
    put(g, 39, y1, "╬", FW, FW_BG, True)
    vline(g, 39, y1 + 1, MY + MH - 1, "║", ICE_BLUE, VOID)
    put(g, 41, MY + MH - 1, "▸ the grid · your block", ICE_BLUE, VOID)
    slots_row(g, ["CRACK", "ATTACK", "SHIELD", None, None])
    bitten = [("at home · ", DIM), ("HP 31/45", TEXT)]
    if state in SUMMON + TERM:
        summon_over(g, state, fx0, fy0, sx0)
    elif state == "calm":
        panel(g, HERE_LOCKED)
    elif state == "fight":
        hp(g, 31, 45)
        panel(g, HERE_FIGHT, black=("awake", RED), seat2=("attack · MASTIFF", "55%"),
              body=bitten)
        footer_fight(g, "MASTIFF bites the × tiles next turn",
                     "move off them · 3 shield · 2 attack",
                     "TURN 4  YOU › seat 2 › MASTIFF",
                     "▣ MASTIFF · black ICE · coat 5/8 · 3 tiles")
    else:
        hp(g, 31, 45)
        panel(g, HERE_OPEN, black=("¤4,800 · open", DIM), body=bitten)
    return g


HERE_FIGHT = [
    [("◆ MASTIFF", RED, True), (" · black ICE", DIM)],
    [("  COAT  ", LABEL), ("▮▮▮▮▮", RED), ("▯▯▯", FAINT), (" 5/8", TEXT)],
    [("  CORE  ", LABEL), ("sealed under the coat", DIM)],
    [("  BITE  ", LABEL), ("next turn · the × tiles", TEXT)],
    [("  LEASH ", LABEL), ("the red ground", DIM)],
]

HERE_OPEN = [
    [("◆ Black stick", AMBER, True), (" · ¤4,800 inside", TEXT)],
    ("  open · summon · space", DIM),
]


# ------------------------------------------------------------------ round 4 · summon

# the three moments of an evening at the reader: the credstick cashed, the Keepalive
# shard installed into a seat, Coz's ledger read
SUMMON = ("cash", "seat", "read")
# round 5: the rig's terminal in the room, its shell open, and the room after run and kill
TERM = ("term", "shell", "ran")
KEEPALIVE = ("keepalive", "Kiroflux Mart", "noise 3 of 36")
SEAT2 = {"seat": KEEPALIVE, "read": KEEPALIVE, "term": KEEPALIVE, "shell": KEEPALIVE,
         "ran": ("sniff", "Hextek", "noise 0 of 36")}
HOLO_FAINT = "#590046"


def empty_slot(g, x, y, bg=LEVEL):
    box(g, x, y, x + 6, y + 4, "#0b3a40", bg, "light")
    put(g, x + 1, y + 5, "free", FAINT, bg)


def hall_summon(g, state, sy):
    """The reader as round 4 loads it: the credstick, a Keepalive shard a Copy job paid,
    and Coz's ledger shard."""
    if state == "cash":
        # the ¤340 dropping out of the stick into VEX
        slot(g, 27, sy, "¤", FAINT, "Credstick", "empty", border=AMBER)
        put(g, 30, sy + 7, "┊", AMBER, LEVEL)
        put(g, 29, sy + 8, "·", AMBER, LEVEL)
        put(g, 30, sy + 8, "¤", AMBER, LEVEL, True)
        put(g, 31, sy + 8, "·", AMBER, LEVEL)
        you(g, 30, sy + 9)
        put(g, 32, sy + 9, "+¤340", AMBER, LEVEL, True)
        slot(g, 36, sy, "◈", CORE, "Shard", "keepalive")
    else:
        slot(g, 27, sy, "¤", FAINT, "Credstick", "empty")
        empty_slot(g, 36, sy)
    slot(g, 45, sy, "▤", MAGENTA, "Shard", "ledger")
    if state == "read":
        you(g, 48, sy + 8)
    elif state != "cash":
        you(g, 39, sy + 8)


def ping(g, fx0, fy0, sx0):
    """keepalive's ping from seat 2, over the hall and through the PORTS wall to the
    backdoor it holds."""
    y2 = fy0 + 13
    put(g, sx0 + 2, y2, "┤", CORE, LEVEL)
    put(g, sx0 + 1, y2, "└", CORE, LEVEL)
    vline(g, sx0 + 1, fy0 + 4, y2 - 1, "┊", CORE, LEVEL)
    put(g, sx0 + 1, fy0 + 3, "┐", CORE, LEVEL)
    put(g, sx0, fy0 + 3, "┼", CORE, LEVEL)
    hline(g, 27, sx0 - 1, fy0 + 3, "┈", CORE, LEVEL)
    put(g, 40, fy0 + 3, "●", CORE, LEVEL, True)
    put(g, 26, fy0 + 3, "┌", CORE, LEVEL)
    put(g, 26, fy0 + 4, "┊", CORE, LEVEL)
    put(g, 26, fy0 + 5, "┘", CORE, LEVEL)
    put(g, 25, fy0 + 5, "┼", CORE, LEVEL)
    hline(g, 23, 24, fy0 + 5, "┈", CORE, LEVEL)
    put(g, 22, fy0 + 5, "◂", CORE, LEVEL, True)
    put(g, fx0 + 2, fy0 + 6, "held · keepalive", CORE, LEVEL)


def holo(g, x, y, w, rows, label, head, close):
    """A reading's HOLO card, as `ReadCard` draws it: the bracket frame, the label and
    head, the signal and clock, the rows, the key to close."""
    h = len(rows) + 7
    fill(g, x, y, x + w - 1, y + h - 1, PANEL)
    hline(g, x, x + w - 1, y, "▀", HOLO_FAINT, PANEL)
    hline(g, x, x + w - 1, y + h - 1, "▄", HOLO_FAINT, PANEL)
    vline(g, x, y, y + h - 1, "▌", HOLO_FAINT, PANEL)
    vline(g, x + w - 1, y, y + h - 1, "▐", HOLO_FAINT, PANEL)
    put(g, x, y, "▛▀▀", MAGENTA, PANEL)
    put(g, x + w - 3, y, "▀▀▜", MAGENTA, PANEL)
    put(g, x, y + h - 1, "▙▄▄", MAGENTA, PANEL)
    put(g, x + w - 3, y + h - 1, "▄▄▟", MAGENTA, PANEL)
    for cx, ch in [(x, "▌"), (x + w - 1, "▐")]:
        put(g, cx, y + 1, ch, MAGENTA, PANEL)
        put(g, cx, y + h - 2, ch, MAGENTA, PANEL)
    right = x + w
    put(g, x + 3, y + 1, label, MAGENTA, PANEL, True)
    put(g, x + 4 + len(label), y + 1, "▸ " + head, DIM, PANEL)
    put(g, right - 16, y + 1, "▂▄▆█", MAGENTA, PANEL)
    put(g, right - 11, y + 1, "22:00", DIM, PANEL)
    for j, segs in enumerate(rows):
        xx = x + 3
        for s, col, *b in segs:
            put(g, xx, y + 3 + j, s, col, PANEL, bool(b and b[0]))
            xx += len(s)
    put(g, x + 3, y + h - 2, "space", MAGENTA, PANEL, True)
    put(g, x + 9, y + h - 2, close, DIM, PANEL)


def ledger_row(date, amount, bar, tail, district):
    return [("%s  ¤%6s  " % (date, amount), TEXT), ("%-9s" % bar, FAINT),
            (tail, DIM if district else FAINT)]


LEDGER_ROWS = [
    [("LEDGER  ", DIM), ("214 entries · 2068–2071", TEXT)],
    [("owner   ", DIM), ("Coz", AMBER, True), (" · fixer", TEXT)],
    [("ICE     open · cracked 02:13 last night", MAGENTA)],
    [],
    ledger_row("02.11", "4 200", "▓▓▓▓▓▓", "Saltyards", True),
    ledger_row("03.04", "600", "▓▓▓▓", "▓▓▓▓▓▓▓", False),
    ledger_row("07.19", "12 000", "▓▓▓▓▓▓▓▓", "the Slab", True),
    ledger_row("11.02", "1 800", "▓▓▓▓▓", "▓▓▓▓▓▓▓▓▓", False),
    [("  …210 more", FAINT)],
    [],
    [("▚▞ too slow. —S", MAGENTA, True)],
]


def summon_over(g, state, fx0, fy0, sx0):
    """What a summon changes past the reader: the ¤ up top, seat 2's line to the
    backdoor it works on, the ledger's card, the terminal and its shell, and the rig's
    panel."""
    if state == "cash":
        put(g, 17, 0, "¤ 660", AMBER, PANEL, True)
        put(g, 23, 0, "+340", AMBER, PANEL)
    else:
        put(g, 19, 0, "660", TEXT, PANEL)
        if state == "ran":
            ping_hextek(g, fy0, sx0)
        else:
            ping(g, fx0, fy0, sx0)
    if state in TERM:
        terminal_node(g, sx0 + 3, fy0 + 19)
    if state == "read":
        holo(g, 6, 9, 66, LEDGER_ROWS, "HOLO", "shard · read at the rig",
             "close · the shard stays in the reader")
    spent = ("¤", DIM, "Credstick", "empty", DIM)
    ledger = ("▤", MAGENTA, "Ledger shard", "story · Coz", DIM)
    free = ("·", FAINT, "free slot", "", FAINT)
    if state == "cash":
        panel(g, HERE_CASH, sticks=[spent, ("◈", CORE, "Keepalive shard", "rig program",
                                             DIM), ledger])
    elif state in ("seat", "read"):
        panel(g, HERE_SEAT if state == "seat" else HERE_READ,
              sticks=[spent, free, ledger if state == "seat"
                      else ("▤", MAGENTA, "Ledger shard", "read", DIM)],
              seat2=("keepalive · Kiroflux Mart", "held"), held="Kiroflux Mart")
    elif state == "ran":
        panel(g, HERE_RAN, sticks=[spent, free, ("▤", MAGENTA, "Ledger shard", "read", DIM)],
              seat2=("sniff · Hextek Holdings", "0m"))
    else:
        panel(g, HERE_TERM, sticks=[spent, free, ("▤", MAGENTA, "Ledger shard", "read", DIM)],
              seat2=("keepalive · Kiroflux Mart", "held"), held="Kiroflux Mart")
    if state == "shell":
        dim_all(g)
        jack_window(g)


def ping_hextek(g, fy0, sx0):
    """sniff's line from seat 2, over the hall and through the PORTS wall to Hextek."""
    put(g, sx0 + 2, fy0 + 12, "┤", CORE, LEVEL)
    put(g, sx0 + 1, fy0 + 12, "└", CORE, LEVEL)
    put(g, sx0 + 1, fy0 + 11, "┐", CORE, LEVEL)
    put(g, sx0, fy0 + 11, "┼", CORE, LEVEL)
    hline(g, 27, sx0 - 1, fy0 + 11, "┈", CORE, LEVEL)
    put(g, 44, fy0 + 11, "●", CORE, LEVEL, True)
    put(g, 26, fy0 + 11, "└", CORE, LEVEL)
    put(g, 26, fy0 + 10, "┐", CORE, LEVEL)
    put(g, 25, fy0 + 10, "┼", CORE, LEVEL)
    hline(g, 17, 24, fy0 + 10, "┈", CORE, LEVEL)
    put(g, 16, fy0 + 10, "◂", CORE, LEVEL, True)


HELD_BG = "#0f3a1f"


def terminal_node(g, x, y):
    """The rig's terminal as the grid draws a terminal you hold: ◇ in green, its name
    and its verb beside it."""
    put(g, x, y, "◇", RELAY, HELD_BG, True)
    put(g, x + 2, y, "TERMINAL", RELAY, LEVEL, True)
    put(g, x + 2, y + 1, "[r] shell", LABEL, LEVEL)


def scale(hexcol, k):
    r, gg, b = (int(hexcol[i:i + 2], 16) for i in (1, 3, 5))
    return "#%02x%02x%02x" % (round(r * k), round(gg * k), round(b * k))


def dim_all(g):
    """Everything behind an open shell, as the game dims it: the text to a third."""
    for row in g.c:
        for c in row:
            c[1] = scale(c[1], 0.35)
            c[2] = scale(c[2], 0.7) if c[2] else c[2]


JACK = "#00ffc8"
JACK_BG = "#020a0c"
JACK_DOT = "#002e24"
JACK_KEY = "#5f8a86"
SHELL = [
    ("jack › zenbo · your rig", False),
    ("handshake ·········· ok", False),
    ("seat: vex", False),
    ("Authorised users only. Sessions are logged.", False),
    ("vex@zenbo:~$ ps", True),
    (" #  JOB        ON                      UP      FOUND", False),
    (" 1  sift       paydata shard           3h 20m", False),
    (" 2  keepalive  kiroflux-mart           40m", False),
    ("Tinman 2X core · 2 of 2 seats busy", False),
    ("vex@zenbo:~$ kill 2", True),
    ("killed 2 · keepalive · kiroflux-mart", False),
    ("vex@zenbo:~$ run sniff hextek", True),
    ("3 · sniff on hextek-holdings · listening for door codes", False),
    ("noise: high · every hour leaves a line in their auth.log", False),
    ("vex@zenbo:~$ ▮", True),
]


def jack_window(g, x0=24, y0=8, x1=95, y1=31):
    """The JACK window every Net terminal's shell opens in (`device::jack`), as the
    net_shell shot draws it: the host on its top edge, you on the right, a dot grid on
    the glass, esc on the bottom edge."""
    fill(g, x0, y0, x1, y1, JACK_BG)
    box(g, x0, y0, x1, y1, JACK, JACK_BG, "light")
    put(g, x0 + 3, y0, "◇ JACK · ZENBO", JACK, JACK_BG, True)
    put(g, x0 + 2, y0, " ", TEXT, JACK_BG)
    put(g, x0 + 17, y0, " ", TEXT, JACK_BG)
    rput(g, x1 - 2, y0, " vex ", RELAY, JACK_BG)
    put(g, x0 + 2, y1, " ", TEXT, JACK_BG)
    put(g, x0 + 3, y1, "esc · back to your rig", JACK_KEY, JACK_BG)
    put(g, x0 + 25, y1, " ", TEXT, JACK_BG)
    for y in range(y0 + 2, y1):
        if y % 2 == 0:
            for x in range(x0 + 3, x1, 6):
                put(g, x, y, "·", JACK_DOT, JACK_BG)
    for j, (line, typed) in enumerate(SHELL):
        y = y0 + 2 + j
        fill(g, x0 + 3, y, x0 + 3 + len(line), y, JACK_BG)
        put(g, x0 + 3, y, line, TEXT, JACK_BG, typed)


HERE_TERM = [
    [("◇ TERMINAL", RELAY, True), (" · your rig's shell", DIM)],
    ("  r opens it from anywhere here", TEXT),
    ("  yours: no crack, no hold to run out", DIM),
]

HERE_RAN = [
    [("◉ sniff", CORE, True), (" · seat 2 · Hextek Holdings", TEXT)],
    ("  keepalive killed · Kiroflux 2d left", DIM),
    ("  the room is the rig's jobs, live", DIM),
]


HERE_CASH = [
    [("¤ Credstick", AMBER, True), (" · summoned", DIM)],
    [("  +¤340", AMBER, True), (" · you carry ¤660", TEXT)],
    ("  the empty stick stays in the reader", DIM),
]

HERE_SEAT = [
    [("◈ keepalive", CORE, True), (" · installed", DIM)],
    ("  seat 2 · holds Kiroflux Mart", TEXT),
    ("  the shard is spent · its slot free", DIM),
]

HERE_READ = [
    [("▤ Ledger shard", MAGENTA, True), (" · Coz's", DIM)],
    ("  read · it stays in the reader", DIM),
]


# ------------------------------------------------------------------ B · the rack

def look_b():
    g = frame()
    x0, x1 = 18, 62            # the firewall's walls
    top, bot = MY, MY + MH - 3
    # no dots in the gutter, as the tower's labels sit there
    fill(g, 0, MY, x0 - 1, MY + MH - 1, VOID)
    fill(g, x0, top, x1, bot, FW_BG)
    box(g, x0, top, x1, bot, FW, FW_BG, "double", True)
    title_in(g, x0, x1, top, "FIREWALL · sandbag ▮▯▯", FW, FW_BG)
    ix0, ix1 = x0 + 2, x1 - 2   # the rack's rails
    rx = ix0 + 3                # the riser, down the left rail

    # the head unit, opened out: the reader
    hy0, hy1 = top + 1, top + 13
    fill(g, ix0, hy0, ix1, hy1, LEVEL)
    box(g, ix0, hy0, ix1, hy1, WALL, WALL_BG, "double")
    rput(g, x0 - 2, hy0 + 1, "HEAD ▸", WHITE, VOID, True)
    rput(g, x0 - 2, hy0 + 2, "the reader", DIM, VOID)
    rput(g, x0 - 2, hy0 + 3, "3 slots", DIM, VOID)
    put(g, ix0 + 8, hy0 + 1, " CIVITAS 3 · INTEGRITY OK ", "#9fd8ff", "#06202a", True)
    sy = hy0 + 3
    slot(g, ix0 + 6, sy, "¤", AMBER, "Credstick", "¤340")
    slot(g, ix0 + 16, sy, "▣", ICE_BLUE, "Locked", "2 locks")
    slot(g, ix0 + 26, sy, "◆", RED, "Black", "MASTIFF")
    boss(g, ix0 + 28, sy + 1)
    watched(g, ix0 + 24, sy - 1, ix1 - 1, sy + 7)
    # you at the locked stick; the cord to your chair by the riser's hub
    you(g, ix0 + 19, sy + 7)
    put(g, ix0 + 19, hy1 - 1, "┘", "#2f6aa8", LEVEL)
    hline(g, ix0 + 6, ix0 + 18, hy1 - 1, "┄", "#2f6aa8", LEVEL)
    put(g, ix0 + 5, hy1 - 1, "◇", TERMINAL, LEVEL, True)
    put(g, ix0 + 21, hy1 - 1, "your chair", DIM, LEVEL)
    put(g, rx, hy1 - 1, "⬡", WHITE, LEVEL, True)

    def folded(y, kind, col, sled):
        """A folded bay: three rows between the rails, its walls, its kind on the left."""
        fill(g, ix0, y, ix1, y + 2, FOLD_BG)
        for yy in range(y, y + 3):
            put(g, ix0, yy, "│", FOLD, FOLD_BG)
            put(g, ix1, yy, "│", FOLD, FOLD_BG)
        hline(g, ix0, ix1, y - 1, "━", FOLD, FW_BG)
        rput(g, x0 - 2, y, kind, col, VOID, True)
        rput(g, x0 - 2, y + 1, sled, DIM, VOID)
        put(g, rx, y + 1, "⬡", WHITE, FOLD_BG, True)

    # CORE
    y = hy1 + 2
    folded(y, "CORE", CORE, "Tinman 2X")
    put(g, rx + 4, y + 1, "◉ sift ▰▰▰▱▱ 4h", CORE, FOLD_BG)
    put(g, rx + 22, y + 1, "· free seat", "#2a5a62", FOLD_BG)
    core_y = y
    # RELAY: free ports to the left, held ones by the east wall, each line out
    y += 4
    folded(y, "RELAY", RELAY, "Switchback")
    relay_y = y
    for px in (rx + 4, rx + 9):
        put(g, px, y + 1, "▯", FAINT, FOLD_BG, True)
    held = [  # port x, the row its line leaves by, name, days
        (ix1 - 6, y + 1, "Kiroflux", "2d"),
        (ix1 - 11, y, "Hextek", "4d"),
        (ix1 - 16, y + 2, "Stack 101", "6h"),
    ]
    for px, ly, name, left in held:
        put(g, px, y + 1, "▯", RELAY, FOLD_BG, True)
        if ly != y + 1:
            put(g, px, ly, "┌" if ly < y + 1 else "└", "#2a8a50", FOLD_BG)
        hline(g, px + 1, ix1 - 1, ly, "─", "#2a8a50", FOLD_BG)
        put(g, ix1, ly, "╪", RELAY, FOLD_BG)
        put(g, ix1 + 1, ly, "═", RELAY, FW_BG)
        put(g, x1, ly, "╬", FW, FW_BG, True)
        hline(g, x1 + 1, x1 + 2, ly, "═", RELAY, VOID)
        put(g, x1 + 4, ly, name, TEXT, VOID)
        put(g, x1 + 5 + len(name), ly, left, DIM, VOID)
    put(g, x1 + 4, y + 3, "2 free lines", FAINT, VOID)
    for yy in range(y, y + 4):
        for xx in range(x1 + 3, MW):
            if g.c[yy][xx][0] == "·":
                g.c[yy][xx][0] = " "
    # LINE: the trunk from the riser to the gate
    y += 4
    folded(y, "LINE", LINE, "Longwire")
    line_y = y
    gx = (ix0 + ix1) // 2 + 6
    hline(g, rx + 1, gx - 1, y + 1, "═", LINE, FOLD_BG)
    put(g, rx + 4, y + 1, " holds 5 days ", LINE, FOLD_BG, True)
    put(g, gx, y + 1, "╗", LINE, FOLD_BG)
    put(g, gx, y + 2, "║", LINE, FOLD_BG)
    put(g, gx, y + 3, "▯", ICE_BLUE, FW_BG, True)
    vline(g, gx, y + 4, bot - 1, "║", ICE_BLUE, FW_BG)
    put(g, gx, bot, "╬", FW, FW_BG, True)
    vline(g, gx, bot + 1, MY + MH - 1, "║", ICE_BLUE, VOID)
    put(g, gx + 2, MY + MH - 1, "▸ the grid · your block", ICE_BLUE, VOID)
    # the riser: hub to hub down the left rail
    for yy in range(hy1, line_y + 1):
        c = g.c[yy][rx]
        if c[0] != "⬡":
            put(g, rx, yy, "╨" if yy == hy1 else "║", "#a1a8ac", c[2])
    # the firewall bay is the skin itself
    rput(g, x0 - 2, bot - 1, "FIREWALL", FW, VOID, True)
    rput(g, x0 - 2, bot, "the skin", DIM, VOID)
    panel(g, HERE_LOCKED)
    return g


# ------------------------------------------------------------------ C · the rings

def look_c():
    g = frame()
    cx, cy = 39, MY + 14
    # the firewall: the outer ring
    fx0, fy0, fx1, fy1 = 9, MY + 1, 69, MY + MH - 3
    box(g, fx0, fy0, fx1, fy1, FW, None, "double", True)
    for y in range(fy0 + 1, fy1):
        for x in range(fx0 + 1, fx1):
            if g.c[y][x][0] == "·":
                g.c[y][x][1] = "#2a0d12"
    put(g, fx0 + 2, fy0, " FIREWALL · sandbag ▮▯▯ ", FW, VOID, True)
    # the reader: the inner ring, the sticks on it
    rx0, ry0, rx1, ry1 = cx - 16, cy - 6, cx + 16, cy + 6
    fill(g, rx0, ry0, rx1, ry1, LEVEL)
    box(g, rx0, ry0, rx1, ry1, WALL, WALL_BG, "round")
    title_in(g, rx0, rx1, ry0, "READER", AMBER, WALL_BG)
    # the core in the middle: your seats
    box(g, cx - 5, cy - 2, cx + 5, cy + 2, CORE, LEVEL, "double")
    put(g, cx - 3, cy - 1, "CORE", CORE, LEVEL, True)
    put(g, cx - 3, cy, "◉ sift", CORE, LEVEL)
    put(g, cx - 3, cy + 1, "· free", "#2a5a62", LEVEL)
    # the sticks: top, left, right
    put(g, cx, ry0 + 2, "¤", AMBER, LEVEL, True)
    put(g, cx + 2, ry0 + 2, "¤340", DIM, LEVEL)
    put(g, rx0 + 4, cy, "▣", ICE_BLUE, LEVEL, True)
    put(g, rx0 + 2, cy - 2, "2 locks", DIM, LEVEL)
    boss(g, rx1 - 7, cy - 1)
    put(g, rx1 - 9, cy + 3, "MASTIFF", RED, LEVEL, True)
    watched(g, rx1 - 11, cy - 3, rx1 - 1, cy + 4)
    you(g, rx0 + 6, cy)
    # the spokes: each relay line leaves the ring's side, out through the firewall
    spokes = [  # side, row, name, left, held
        ("w", cy - 3, "Kiroflux", "2d left", True),
        ("w", cy + 3, "Hextek", "4d left", True),
        ("e", cy - 3, "Stack 101", "6h left", True),
        ("e", cy + 3, "free line", "", False),
    ]
    for side, y, name, left, held in spokes:
        col = RELAY if held else "#1c3a28"
        if side == "w":
            put(g, rx0, y, "▯", col if held else FAINT, WALL_BG, True)
            hline(g, fx0 + 1, rx0 - 1, y, "─", col)
            put(g, fx0, y, "╬", FW, None, True)
            hline(g, 0, fx0 - 1, y, "═" if held else "─", col)
            put(g, 0, y - 1, name, TEXT if held else FAINT)
            put(g, 0, y + 1, left, DIM)
        else:
            put(g, rx1, y, "▯", col if held else FAINT, WALL_BG, True)
            hline(g, rx1 + 1, fx1 - 1, y, "─", col)
            put(g, fx1, y, "╬", FW, None, True)
            hline(g, fx1 + 1, MW - 1, y, "═" if held else "─", col)
            rput(g, MW - 1, y - 1, name, TEXT if held else FAINT)
            rput(g, MW - 1, y + 1, left, DIM)
    # the fifth line: up through the top
    put(g, cx + 10, ry0, "▯", FAINT, WALL_BG, True)
    vline(g, cx + 10, fy0 + 1, ry0 - 1, "│", "#1c3a28")
    put(g, cx + 10, fy0, "╬", FW, None, True)
    put(g, cx + 12, fy0 + 1, "free line", FAINT)
    # the gate: straight down to the grid
    put(g, cx, ry1, "▯", ICE_BLUE, WALL_BG, True)
    vline(g, cx, ry1 + 1, fy1 - 1, "║", ICE_BLUE)
    put(g, cx, fy1, "╬", FW, None, True)
    vline(g, cx, fy1 + 1, MY + MH - 1, "║", ICE_BLUE)
    put(g, cx + 2, MY + MH - 1, "▸ the grid · your block", ICE_BLUE)
    put(g, cx + 2, ry1 + 2, "LINE · Longwire · holds 5d", LINE)
    # your chair, inside the ring under the core
    put(g, cx - 8, ry1 - 2, "◇", TERMINAL, LEVEL, True)
    put(g, cx - 6, ry1 - 2, "your chair", DIM, LEVEL)
    vline(g, rx0 + 6, cy + 1, ry1 - 3, "┆", "#2f6aa8", LEVEL)
    put(g, rx0 + 6, ry1 - 2, "└", "#2f6aa8", LEVEL)
    hline(g, rx0 + 7, cx - 9, ry1 - 2, "┄", "#2f6aa8", LEVEL)
    panel(g, HERE_LOCKED)
    return g


# ------------------------------------------------------------------ the page

FRAMES = [
    ("before", "1", "Before: MASTIFF on its stick", "calm",
     "Look A as round 2 left it. MASTIFF is coiled round the black stick, and the red "
     "ground round it is how far it reaches once it's awake.",
     ["Stand at the black stick and Space asks first: Wake MASTIFF? It's black ICE, so "
      "its bites hurt your body back home. The deck's failsafe still dumps you before it "
      "kills you.",
      "Asleep, it does nothing. You can walk past it, crack the locked stick beside it "
      "and summon the credstick."]),
    ("fight", "2", "Turn 4: the bite", "fight",
     "MASTIFF is off its stick and the fight is on turns. Three plates of its coat are "
     "broken, on the side VEX has been hitting from. It's winding up a bite on the × "
     "tiles, and VEX is standing on one.",
     ["The leash. MASTIFF can't leave the red ground round its stick. Step off it and it "
      "can't reach you, but for every turn you're off, a plate of its coat grows back.",
      "The coat. Eight plates round a core, drawn on the board and shown as COAT 5/8 in "
      "the panel. Each Attack that lands breaks one, on the side you hit from. Once the "
      "coat is off, the core takes the last hits.",
      "The bite. Every few turns it marks the tiles it will hit on its next turn. Step "
      "off them or Shield. A bite hurts your body: HP 31/45 up top, and in the panel's "
      "BODY line.",
      "Your rig fights with you. A free core seat runs your Attack on its own turn: "
      "seat 2 in the turn order, its beam coming through the SEATS wall. A seat busy "
      "with a job sits the fight out unless you stop the job, so a second core is a "
      "second fighter.",
      "It's inside your own firewall, so the fight raises no trace and calls no tracer: "
      "TRACE stays at 0. If you're dumped or jack out, MASTIFF coils back onto its stick "
      "with its coat whole, and the stick waits for another go."]),
    ("after", "3", "MASTIFF broken", "after",
     "The core breaks, MASTIFF goes to grit, and the black stick opens. It's an open "
     "stick now, like the credstick: stand at it and Space summons what's on it.",
     ["A black stick pays more than a locked one: ¤4,800 here. It could hold a program "
      "or a story item instead. What each kind gives is the summon round's question.",
      "Seat 2 goes back to free. The bites stay on your body until you rest or get "
      "patched up."]),
]

FRAMES_SUMMON = [
    ("cash", "1", "Summon the credstick", "cash",
     "VEX stands at the credstick and presses Space. What's on it drops out of the stick "
     "into VEX: ¤340, and the ¤ up top counts from 320 to 660.",
     ["An open stick summons on Space: no turn, no roll. A credstick's ¤ comes to you, "
      "as if you'd picked it up off a floor.",
      "The stick stays in the reader, empty. Pull it at the desk. What an empty stick is "
      "good for is the credstick work's call.",
      "A locked or black stick summons the same way once it's open: crack its locks, or "
      "break its ICE, first."]),
    ("seat", "2", "Install a rig program", "seat",
     "Next, the Keepalive shard a Copy job paid. Space installs keepalive, and the rig "
     "asks which backdoor it should hold. VEX picks Kiroflux Mart, two days from "
     "shutting.",
     ["Summoning a rig program installs it, as sitting down with the shard in your pack "
      "does today. The shard is spent and its slot is free.",
      "It runs in the free core seat, and the room draws it working: seat 2's ping runs "
      "over the hall and through the PORTS wall to Kiroflux, whose clock stops. The "
      "LINES list says held.",
      "No free seat: it only installs, and you start it later with run at the terminal. "
      "A key backdoor can't be picked, as the manual says.",
      "Noise still builds on Kiroflux, 3 of 36 here. At 36 the backdoor burns, the ping "
      "stops and that door goes dark in the room."]),
    ("read", "3", "Read Coz's ledger", "read",
     "Last, the ledger shard from the morning after. Summoning a story shard opens its "
     "reading: the same HOLO card the rig shows today, over the room.",
     ["It's the shard's reading at the rig, row for row, so reading it here counts for "
      "the story as reading it at the desk does.",
      "The card sits over the map, not the middle of the screen, so the rig's panel stays "
      "readable beside it. Space closes it.",
      "The shard stays in the reader. Pull it at the desk to take it back to Coz."]),
]

KINDS = [
    ("¤ creds", "a credstick", "the ¤ comes to you; the empty stick stays in the reader"),
    ("a deck program", "Mask, Crack II",
     "goes into a free PROGRAMS slot, which flashes; with the deck full it asks which to "
     "swap out"),
    ("a rig program", "sniff, keepalive",
     "installs on the rig and runs in a free core seat, on a backdoor you pick"),
    ("paydata", "a paydata shard, a vault dump",
     "goes to a free seat for sift and comes back sifted, to sell"),
    ("a story shard", "Coz's ledger", "opens its reading; the shard stays in the reader"),
    ("locked or black", "any of the above",
     "nothing until its locks are cracked or its ICE is broken, then as above"),
]

FRAMES_TERM = [
    ("term", "1", "The terminal", "term",
     "Your rig's terminal is a node in the room, drawn as a terminal you hold is drawn "
     "on the grid: ◇ in green in the SEATS room, TERMINAL beside it and [r] shell under "
     "that. It's always yours, so there's nothing to crack and no hold to run out.",
     ["r opens it from anywhere in the room. On the grid, r works on the nearest terminal "
      "you hold, and here that's always this one, so you don't walk to it. Space at it "
      "opens it too, like everything else in the room.",
      "It sits with the seats because HELIX runs on the core, and what you do in it shows "
      "up beside it.",
      "The room as round 4 left it: keepalive in seat 2 holding Kiroflux Mart, sift in "
      "seat 1, the credstick spent."]),
    ("shell", "2", "r: the same shell", "shell",
     "The shell opens in the JACK window every Net terminal's shell uses, over the room, "
     "which dims behind it. It's your rig's own HELIX, logged in as vex, with every "
     "command the desk has.",
     ["Same window, same keys: the handshake, the notice, esc to go back. The top edge "
      "names ZENBO and you, with no HELD clock because it's yours.",
      "VEX checks ps, kills keepalive (job 2) and runs sniff on Hextek Holdings in the "
      "seat that freed up.",
      "It works both ways: what the room did is already in the shell. The keepalive "
      "summoned in round 4 is job 2 in ps, and the credstick's ¤ is on you."]),
    ("ran", "3", "Back in the room", "ran",
     "Esc, and the room has caught up. Seat 2 runs sniff now, its line goes to Hextek "
     "Holdings, and Kiroflux's clock runs again.",
     ["The room is drawn from the rig's jobs, so run and kill change it the moment "
      "enter goes in: a seat fills or empties, a line to a backdoor appears or goes.",
      "kill 2 took keepalive's line off Kiroflux: PORTS says 2d left again, and LINES "
      "net · 2d.",
      "run sniff hextek put sniff in seat 2, its line to Hextek's door. burn from the "
      "shell takes a door off the PORTS wall the same way.",
      "The shell opened from the desk changes the room too: the room is just the rig's "
      "state, however you got at it."]),
]

LEGEND_TERM = [
    ("◇", RELAY, "your rig's terminal: r opens its shell"),
    ("◉", CORE, "a job in a core seat"),
    ("┈", CORE, "a job's line to the backdoor it works on"),
    ("▯", RELAY, "a backdoor: a door out to that building"),
    ("¤", AMBER, "a credstick"),
    ("▤", MAGENTA, "a story shard"),
]

LEGEND_SUMMON = [
    ("¤", AMBER, "a credstick: summon it to cash it"),
    ("◈", CORE, "a rig program on a shard: summon it to install and run it"),
    ("▤", MAGENTA, "a story shard: summon it to read it"),
    ("◉", CORE, "a job in a core seat"),
    ("┈", CORE, "keepalive's ping to the backdoor it holds"),
    ("▯", RELAY, "a backdoor: a door out to that building"),
]

LEGEND = [
    ("¤", AMBER, "a credstick, open: summon it to cash it"),
    ("▣", ICE_BLUE, "a locked stick: crack its locks first"),
    ("◆", RED, "a black stick, with its ICE (MASTIFF) round it"),
    ("▯", RELAY, "a backdoor: a door out to that building"),
    ("▯", FAINT, "a free line: no backdoor on it yet"),
    ("◉", CORE, "a job in a core seat (sift)"),
    ("×", BITE, "a tile MASTIFF bites on its next turn"),
    ("┆", LEASH, "the edge of MASTIFF's leash"),
    ("░", BROKEN, "a broken plate of its coat"),
]


def page(grids, frames, legend, h1, moment, kinds=False):
    css = term.CSS + """
body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;margin:0;padding:28px 32px 60px}
h1{font-size:22px;margin:0 0 6px;color:#e6f0f5}
h2{font-size:18px;margin:34px 0 4px;color:#e6f0f5}
h2 b{color:#ffb000;margin-right:8px}
.lead{max-width:1180px;color:#a0aab9;margin:0 0 14px}
.look{margin:0 0 36px}
.look p{max-width:1180px;margin:4px 0 10px}
.look ul{max-width:1180px;margin:10px 0 0;padding-left:20px;color:#a0aab9}
.look li{margin:4px 0}
.legend{display:flex;flex-wrap:wrap;gap:6px 22px;max-width:1180px;margin:8px 0 18px;color:#a0aab9;font-size:14px}
.legend span.g{font-family:'StrataFira',monospace;font-weight:700;margin-right:6px}
.kinds{border-collapse:collapse;max-width:1180px;margin:8px 0;font-size:14px}
.kinds th{text-align:left;color:#e6f0f5;border-bottom:1px solid #283c4b;padding:4px 18px 4px 0}
.kinds td{color:#a0aab9;border-bottom:1px solid #182430;padding:5px 18px 5px 0;vertical-align:top}
.kinds td:first-child{color:#cdd4de;white-space:nowrap}
.moment{max-width:1180px;color:#a0aab9;border-left:3px solid #283c4b;padding:4px 12px;margin:10px 0 20px}
"""
    out = ['<!doctype html><html><head><meta charset="utf-8"><title>Rig construct · #104</title>'
           '<style>%s</style></head><body>' % css]
    out.append("<h1>%s</h1>" % html.escape(h1))
    out.append('<p class="lead">Jacking in at home lands you inside your rig instead of on the '
               "city's grid. Its parts shape the place, its firewall is the wall round it, the "
               "backdoors you hold are the ways out, a stick in the reader is something you "
               "can summon, crack or fight for, and its terminal opens the rig's own shell.</p>")
    out.append('<p class="moment">%s</p>' % moment)
    out.append('<div class="legend">')
    for glyph, col, words in legend:
        out.append('<div><span class="g" style="color:%s">%s</span>%s</div>'
                   % (col, html.escape(glyph), html.escape(words)))
    out.append("</div>")
    for (key, letter, name, _, lead, points), g in zip(frames, grids):
        out.append('<div class="look" id="%s"><h2><b>%s</b>%s</h2><p>%s</p>'
                   % (key, letter, html.escape(name), html.escape(lead)))
        out.append(term.render(g, 14))
        out.append("<ul>%s</ul></div>" % "".join("<li>%s</li>" % html.escape(p) for p in points))
    if not kinds:
        out.append("</body></html>")
        return "".join(out)
    out.append('<div class="look" id="kinds"><h2><b>·</b>What summon gives, by what\'s on it</h2>'
               "<p>Space at an open stick or shard. Where it goes depends on what it holds.</p>"
               '<table class="kinds"><tr><th>On it</th><th>For example</th><th>Summon</th></tr>')
    for kind, eg, does in KINDS:
        out.append("<tr><td>%s</td><td>%s</td><td>%s</td></tr>"
                   % (html.escape(kind), html.escape(eg), html.escape(does)))
    out.append("</table></div>")
    out.append("</body></html>")
    return "".join(out)


MOMENT_TERM = ("The moment: straight after round 4. keepalive holds Kiroflux Mart from "
               "seat 2 and <code>sift</code> has seat 1. VEX would rather have door codes "
               "out of Hextek Holdings, so they open the rig's terminal from the room.")


if __name__ == "__main__":
    # round 5: the terminal, in look A. Rounds 1 (three looks), 2 (look A, no chair),
    # 3 (the MASTIFF fight: FRAMES, LEGEND) and 4 (summoning: FRAMES_SUMMON,
    # LEGEND_SUMMON, KINDS) stay in rig-construct-round1..4.html.
    grids = [look_a(f[3]) for f in FRAMES_TERM]
    open(os.path.join(HERE, "rig-construct.html"), "w").write(
        page(grids, FRAMES_TERM, LEGEND_TERM,
             "Your rig's own Net · #104 · round 5 · the terminal", MOMENT_TERM))
    if "--text" in sys.argv:
        for g in grids:
            print(g.text())
