"""Container glyph mockups (#96): one small floor drawn three ways.

    python3 -I gen.py mock.html

A · a glyph per kind, B · solid bodies (a filled cell with a detail on it),
C · set into the wall (lockers, cabinets and safes in the wall line, crates and desks
on the floor). Colours are the game's indoor palette from the indoor lighting shots."""

import html
import sys

OUT = sys.argv[1]
W, H = 46, 16

FLOOR_BG = "#1a222e"
FLOOR_DOT = "#2a364a"
WALL_FG = "#6e87a0"
WALL_BG = "#0a0b10"
FURN = "#84a2c0"
LAMP = "#fff0c8"
DOOR = "#dcebf5"
WINDOW = "#00acbf"
NET = "#00e6d2"
YOU = "#ffffff"
DIM = "#5b6b7d"

# ---- the floor: rooms as rects, everything else wall
rooms = [
    (1, 1, 12, 5),    # lockers
    (14, 1, 28, 5),   # security office
    (30, 1, 44, 5),   # executive office
    (1, 7, 44, 8),    # corridor
    (1, 10, 20, 14),  # storage
    (22, 10, 31, 14),  # first aid
    (33, 10, 44, 14),  # guest room
]
grid = [["#"] * W for _ in range(H)]
for x0, y0, x1, y1 in rooms:
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            grid[y][x] = "."
for x, y in [(6, 6), (20, 6), (37, 6), (10, 9), (26, 9), (38, 9)]:
    grid[y][x] = "+"
for x in [4, 8, 18, 24, 34, 40]:
    grid[0][x] = "w"

# things on the floor, by tile
things = {}


def put(x, y, k):
    things[(x, y)] = k


for x in range(2, 10):
    put(x, 1, "L")  # a bank of lockers on the north wall
for x in range(2, 10):
    put(x, 3, "b")  # a bench
put(16, 2, "D"); put(17, 2, "D"); put(16, 3, "h")
put(26, 1, "T"); put(27, 1, "S"); put(22, 4, "C")
put(33, 2, "D"); put(34, 2, "D"); put(35, 2, "D"); put(34, 3, "h")
put(44, 1, "S")
for x in range(3, 18, 3):
    put(x, 11, "C"); put(x, 13, "C")
put(4, 12, "C")
put(23, 10, "M"); put(24, 10, "M"); put(27, 12, "B"); put(28, 12, "B")
put(35, 13, "B"); put(36, 13, "B"); put(44, 10, "D"); put(44, 14, "L")
put(20, 3, "*"); put(37, 3, "*"); put(6, 4, "*"); put(10, 12, "*"); put(26, 13, "*"); put(39, 12, "*")
put(14, 7, "@")

# which containers are in which state (closed by default)
state = {(27, 1): "net", (44, 1): "net", (4, 1): "open", (5, 1): "empty",
         (3, 13): "empty", (6, 11): "open", (24, 10): "open", (17, 2): "net"}

DOUBLE = {0: "═", 1: "║", 2: "═", 3: "╚", 4: "║", 5: "║", 6: "╔", 7: "╠",
          8: "═", 9: "╝", 10: "═", 11: "╩", 12: "╗", 13: "╣", 14: "╦", 15: "╬"}


def wallish(x, y):
    return 0 <= x < W and 0 <= y < H and grid[y][x] in "#w+"


def wall_glyph(x, y):
    m = (1 if wallish(x, y - 1) else 0) | (2 if wallish(x + 1, y) else 0) \
        | (4 if wallish(x, y + 1) else 0) | (8 if wallish(x - 1, y) else 0)
    return DOUBLE[m]


def floor_cell(x, y):
    return ("·" if (x * 7 + y * 13) % 23 == 0 else " ", FLOOR_DOT, FLOOR_BG, False)


OPENED = {"L": "▯", "S": "○", "C": "☐", "M": "┼", "D": "▭"}


# ---- each option: kind, state -> (glyph, fg, bg, bold)
def opt_a(k, s):
    g, fg = {
        "L": ("▥", FURN), "S": ("◙", "#c9d3dc"), "C": ("☒", "#b0905a"),
        "M": ("╋", "#5fd38a"), "D": ("◨", FURN),
    }[k]
    if s in ("open", "empty"):
        g = OPENED[k]
    if s == "empty":
        fg = DIM
    if s == "net":
        fg = NET
    return (g, fg, FLOOR_BG, k == "S")


def opt_b(k, s):
    body = {"L": "#3a4a5c", "S": "#2b2f36", "C": "#5a4a32", "M": "#b9c3cc", "D": "#3e4c5c"}[k]
    g, fg = {"L": ("┃", "#9fb6cc"), "S": ("◉", "#d8c890"), "C": ("╳", "#a89060"),
             "M": ("╋", "#c0392b"), "D": ("╍", "#a0b8d0")}[k]
    if s in ("open", "empty"):
        g = {"L": "▯", "S": "○", "C": "□", "M": "┼", "D": "─"}[k]
    if s == "empty":
        body, fg = "#20262e", DIM
    if s == "net":
        fg = NET
    return (g, fg, body, k == "S")


IN_WALL = {"L", "M", "S"}


# round 2: B's bodies, each piece set in the wall or standing on the floor by itself —
# the locker bank, the executive safe, the first-aid cabinet and the guest room's
# locker in the wall; the security office's safe, the crates and the desks on the floor
WALL_SET = {(x, 1) for x in range(2, 10)} | {(44, 1), (23, 10), (24, 10), (44, 14)}


def render(opt, in_wall):
    cells = []
    for y in range(H):
        row = []
        for x in range(W):
            t = grid[y][x]
            k = things.get((x, y))
            if t == "#":
                c = (wall_glyph(x, y), WALL_FG, WALL_BG, False)
            elif t == "w":
                c = ("▫", WINDOW, WALL_BG, False)
            elif t == "+":
                c = ("▯", DOOR, FLOOR_BG, False)
            else:
                c = floor_cell(x, y)
            if k == "*":
                c = ("●", LAMP, FLOOR_BG, False)
            elif k == "@":
                c = ("@", YOU, FLOOR_BG, True)
            elif k == "b":
                c = ("╥", FURN, FLOOR_BG, False)
            elif k == "h":
                c = ("╦", "#7d93a8", FLOOR_BG, False)
            elif k == "T":
                c = ("▣", NET, FLOOR_BG, False)
            elif k == "B":
                c = ("▤", FURN, FLOOR_BG, False)
            elif k:
                c = opt(k, state.get((x, y), "closed"))
            row.append(c)
        cells.append(row)
    if in_wall:
        # lift wall-set containers into the wall line they stand against
        for (x, y), k in things.items():
            wall_set = (x, y) in WALL_SET if in_wall == "some" else k in IN_WALL
            if not wall_set:
                continue
            for dx, dy in [(0, -1), (1, 0), (0, 1), (-1, 0)]:
                if grid[y + dy][x + dx] in "#w":
                    g, fg, bg, b = opt(k, state.get((x, y), "closed"))
                    if opt is opt_a:
                        bg = WALL_BG
                    cells[y + dy][x + dx] = (g, fg, bg, b)
                    cells[y][x] = floor_cell(x, y)
                    break
    return cells


STYLES = {}


def span(c, cls=""):
    g, fg, bg, b = c
    name = STYLES.setdefault((fg, bg, b), f"s{len(STYLES)}")
    extra = f" {cls}" if cls else ""
    return f'<i class="{name}{extra}">{html.escape(g)}</i>'


def style_sheet():
    return "".join(
        f".{n}{{color:{fg};background:{bg}{';font-weight:700' if b else ''}}}"
        for (fg, bg, b), n in STYLES.items())


def pre(cells):
    return "<pre class=pf>" + "\n".join("".join(span(c) for c in row) for row in cells) + "</pre>"


def legend(opt, wall):
    names = {"L": "locker", "S": "safe", "C": "crate", "M": "medicine cabinet", "D": "desk drawer"}
    rows = ["<tr><th></th><td>closed</td><td>open</td><td>empty</td><td>on the Net</td></tr>"]
    for k, n in names.items():
        cs = []
        for s in ["closed", "open", "empty", "net"]:
            g, fg, bg, b = opt(k, s)
            if wall and k in IN_WALL:
                bg = WALL_BG
            cs.append("<td>" + span((g, fg, bg, b), "cell") + "</td>")
        rows.append(f"<tr><th>{n}</th>{''.join(cs)}</tr>")
    return f"<table class=lg>{''.join(rows)}</table>"


OPTS = [
    ("A", "a glyph per kind", opt_a, False,
     "Each kind is its own shape: lockers ▥, a safe ◙, a crate ☒, a medicine cabinet ╋, "
     "a desk with a drawer ◨. Opened, the shape opens; emptied, it dims; a lock on the Net is cyan."),
    ("B", "solid bodies", opt_b, False,
     "Each container is a filled cell, its body colour by kind, with a detail on it: a "
     "locker's seam, the safe's dial, a crate's bracing, the cabinet's red cross, a drawer's "
     "handle. It reads as an object more than a symbol."),
    ("C", "set into the wall", opt_a, True,
     "A's shapes, but lockers, cabinets and safes sit in the wall line, like windows do; "
     "crates and desks stand on the floor, and the rooms keep their floor clear."),
]

CSS = """
@font-face{font-family:Fira;src:url('../../../../crates/strata-zero/fonts/FiraCode-Regular.ttf')}
@font-face{font-family:Fira;font-weight:700;src:url('../../../../crates/strata-zero/fonts/FiraCode-Bold.ttf')}
body{background:#05070a;color:#cdd4de;font-family:Fira,monospace;margin:24px}
h1{font-size:16px;color:#00e5ff;letter-spacing:.1em}
.opt{display:inline-block;vertical-align:top;margin:0 28px 28px 0;max-width:560px}
.opt h2{font-size:14px;color:#ffd060;margin:0 0 6px}
.opt p{font-size:12px;line-height:1.5;color:#9fb0c2;margin:8px 0}
.note{font-size:12px;line-height:1.5;color:#9fb0c2;margin:-16px 0 28px;max-width:1150px}
.win{border:1px solid #2a3440;background:#020304;padding:8px;display:inline-block}
pre.pf{margin:0;font-family:Fira,monospace;font-size:17px;line-height:1}
i{font-style:normal}
pre.pf i{display:inline-block;width:.6em;text-align:center}
.lg{border-collapse:collapse;font-size:12px;margin-top:6px}
.lg th{text-align:left;font-weight:400;color:#9fb0c2;padding:2px 10px 2px 0}
.lg td{padding:2px 8px;color:#5b6b7d;text-align:center}
.cell{display:inline-block;width:1.2em;font-size:17px;text-align:center}
"""

# ---- round 3: the take panel, by the security office's safe

CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
CDIM = "#a0aab9"
FAINT = "#78828f"
FAINTER = "#5c6673"
TEXT = "#cdd4de"
PANEL_BG = "#04060a"
BAR_BG = "#003c46"
AMBER = "#ffb000"
CASE = "#2b2f36"
CASE_HI = "#3d434d"
CASE_INK = "#8e98a6"
INSIDE = "#0d0f13"


def mix(a, b, t):
    pa = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(round(x + (y - x) * t) for x, y in zip(pa, pb))


class Canvas:
    """A panel's cells laid over the playfield at (x0, y0), the map ghosting through."""

    def __init__(self, cells, x0, y0, w, h, bg=PANEL_BG):
        self.c, self.x0, self.y0, self.w, self.h = cells, x0, y0, w, h
        for y in range(h):
            for x in range(w):
                g, fg, cbg, _ = cells[y0 + y][x0 + x]
                self.c[y0 + y][x0 + x] = (g, mix(fg, bg, 0.82), mix(cbg, bg, 0.8), False)

    def put(self, x, y, s, fg, bg=None, bold=False):
        for i, ch in enumerate(s):
            if 0 <= x + i < self.w and 0 <= y < self.h:
                old = self.c[self.y0 + y][self.x0 + x + i]
                self.c[self.y0 + y][self.x0 + x + i] = (ch, fg, bg or old[2], bold)

    def fill(self, x, y, w, h, bg):
        for yy in range(y, y + h):
            self.put(x, yy, " " * w, bg, bg)

    def frame(self, title=None, glyph=None):
        w, h = self.w, self.h
        self.put(0, 0, "┍" + "━" * (w - 2) + "┑", CYAN, PANEL_BG)
        self.put(0, h - 1, "┕" + "━" * (w - 2) + "┙", CYAN, PANEL_BG)
        for y in range(1, h - 1):
            self.put(0, y, "│", CYAN, PANEL_BG)
            self.put(w - 1, y, "│", CYAN, PANEL_BG)
        self.fill(1, 1, w - 2, h - 2, PANEL_BG)
        if title:
            x = 2
            self.put(x, 0, " ", CYAN, PANEL_BG)
            x += 1
            if glyph:
                self.put(x, 0, glyph[0] + " ", glyph[1], glyph[2] if len(glyph) > 2 else PANEL_BG, True)
                x += 2
            self.put(x, 0, title + " ", CYAN, PANEL_BG, True)

    def tag(self, text, fg):
        """A mark at the right end of the top rule, as " ● NET "."""
        self.put(self.w - 3 - len(text), 0, " " + text + " ", fg, PANEL_BG, True)

    def row(self, y, key, left, right="", fg=TEXT, rfg=CDIM, sel=False, off=False):
        bg = BAR_BG if sel else PANEL_BG
        if sel:
            self.fill(1, y, self.w - 2, 1, bg)
        kf = FAINTER if off else (CYAN if not sel else "#ffffff")
        self.put(2, y, key, kf, bg, True)
        self.put(5, y, left, FAINTER if off else fg, bg, sel)
        if right:
            self.put(self.w - 2 - len(right), y, right, FAINTER if off else rfg, bg)


def scene(safe_state):
    saved = dict(state), dict(things)
    state[(27, 1)] = safe_state
    del things[(14, 7)]
    things[(27, 2)] = "@"
    cells = render(opt_b, "some")
    state.clear(); state.update(saved[0]); things.clear(); things.update(saved[1])
    return cells


def a_locked_old():
    cells = scene("net")
    p = Canvas(cells, 1, 3, 30, 11)
    p.frame("SAFE", ("◉", NET, CASE))
    p.put(2, 2, "megacorp lock · grade 3", CDIM)
    p.put(2, 3, "on the Net · its node: S-14", NET)
    p.row(5, "k", "Keyjack", "2 fuses", sel=True)
    p.row(6, "f", "force it", "Body 6")
    p.row(7, "c", "card · EXECUTIVE", "none", off=True)
    p.put(2, 9, "esc leave it", FAINT)
    return cells


def a_locked(net=True):
    """Round 4: the Net shows only as a mark in the header and a jack-in row."""
    cells = scene("net" if net else "closed")
    p = Canvas(cells, 1, 3, 30, 11 if net else 10)
    p.frame("SAFE", ("◉", NET if net else "#d8c890", CASE))
    if net:
        p.tag("● NET", NET)
    p.put(2, 2, "megacorp lock · grade 3", CDIM)
    y = 4
    if net:
        p.row(y, "j", "jack in", sel=True)
        y += 1
    p.row(y, "k", "Keyjack", "2 fuses", sel=not net)
    p.row(y + 1, "f", "force it", "Body 6")
    p.row(y + 2, "c", "card · EXECUTIVE", "none", off=True)
    p.put(2, y + 4, "esc leave it", FAINT)
    return cells


def a_open():
    cells = scene("open")
    p = Canvas(cells, 1, 3, 30, 12)
    p.frame("SAFE · OPEN", ("○", "#d8c890", CASE))
    p.row(2, "a", "cash", "1,250 cr", sel=True)
    p.row(3, "b", "credchip", "400 cr")
    p.row(4, "c", "credchip", "LOCKED", rfg=MAGENTA)
    p.row(5, "d", "Frag ×2", "0.8 kg")
    p.row(6, "e", "Keycard · EXEC", "0.0 kg")
    p.put(2, 8, "t take all", CYAN)
    p.put(18, 8, "pack 9.4/20", CDIM)
    p.put(2, 10, "space take · esc close", FAINT)
    return cells


def b_case(cells, x0, y0, w, h):
    """The safe itself: a steel case in the body colour, its inside dark."""
    p = Canvas(cells, x0, y0, w, h, bg=CASE)
    p.fill(0, 0, w, h, CASE)
    p.put(0, 0, "▄" * w, CASE_HI, PANEL_BG)
    p.put(0, h - 1, "▀" * w, CASE_HI, PANEL_BG)
    return p


def b_locked():
    cells = scene("net")
    p = b_case(cells, 1, 3, 30, 11)
    p.put(2, 1, "MEGACORP", CASE_INK, CASE, True)
    p.put(21, 1, "G3", AMBER, CASE, True)
    p.put(10, 3, "╭─────╮", CASE_INK, CASE)
    p.put(10, 4, "│  ◉  │", CASE_INK, CASE)
    p.put(13, 4, "◉", "#d8c890", CASE, True)
    p.put(10, 5, "╰─────╯", CASE_INK, CASE)
    p.put(20, 4, "▬▬", CASE_INK, CASE)
    p.put(2, 7, "●", NET, CASE)
    p.put(4, 7, "NET · S-14", NET, CASE)
    p.put(2, 9, "k", CYAN, CASE, True)
    p.put(4, 9, "keyjack", TEXT, CASE)
    p.put(13, 9, "f", CYAN, CASE, True)
    p.put(15, 9, "force", TEXT, CASE)
    p.put(22, 9, "c", FAINTER, CASE, True)
    p.put(24, 9, "card", FAINTER, CASE)
    return cells


def b_open():
    cells = scene("open")
    p = b_case(cells, 1, 3, 30, 12)
    # the door swung open on the left, its dial facing you
    for y in range(1, 11):
        p.put(1, y, "▐", CASE_HI, CASE)
        p.put(2, y, "  ", CASE, CASE_HI)
    p.put(2, 5, "◉", "#d8c890", CASE_HI, True)
    p.fill(5, 1, 23, 9, INSIDE)
    shelf = "─" * 23
    p.put(5, 4, shelf, CASE_INK, INSIDE)
    p.put(5, 7, shelf, CASE_INK, INSIDE)
    p.put(6, 2, "a", CYAN, BAR_BG, True)
    p.fill(7, 2, 20, 1, BAR_BG)
    p.put(8, 2, "1,250 cr", "#ffffff", BAR_BG, True)
    p.put(6, 3, "b", CYAN, INSIDE, True)
    p.put(8, 3, "chip 400", TEXT, INSIDE)
    p.put(18, 3, "c", CYAN, INSIDE, True)
    p.put(20, 3, "chip ▣", MAGENTA, INSIDE)
    p.put(6, 5, "d", CYAN, INSIDE, True)
    p.put(8, 5, "Frag ×2", TEXT, INSIDE)
    p.put(6, 6, "e", CYAN, INSIDE, True)
    p.put(8, 6, "Keycard EXEC", TEXT, INSIDE)
    p.put(6, 8, "t take all", CYAN, INSIDE)
    p.put(18, 8, "9.4/20 kg", CDIM, INSIDE)
    p.put(6, 10, "space take · esc", CASE_INK, CASE)
    return cells


ROUND3 = [
    ("A", "the deck's popup", a_locked_old, a_open,
     "The verb popup's frame and rows. Locked, it names the lock and lists the ways in "
     "(a card you don't carry greyed). Open, it lists what's inside: a letter takes one "
     "line, t takes all, with the pack's load under it."),
    ("B", "the safe itself", b_locked, b_open,
     "The container drawn as itself, in its B body colour: locked, its door with the "
     "maker, grade, dial and Net light, the ways in stamped under it; open, the door swung "
     "aside and the contents on its shelves."),
]

ROUND2 = [
    ("B", "solid bodies, in the wall or on the floor", opt_b, "some",
     "Picked: B. Each piece is set in the wall or stands on the floor by itself. Here the "
     "locker bank, the executive safe (top right), the first-aid cabinet and the guest "
     "room's locker are in the wall; the security office's safe (beside its terminal), "
     "the crates and the desks stand on the floor."),
]

body = ["<h1>CONTAINERS · #96 · round 4 · A, the Net in the header</h1>",
        f'<div class=opt><h2>A · locked, on the Net</h2><div class=win>{pre(a_locked(True))}</div></div>',
        f'<div class=opt><h2>A · locked, off the Net</h2><div class=win>{pre(a_locked(False))}</div></div>',
        f'<div class=opt><h2>A · open</h2><div class=win>{pre(a_open())}</div></div>',
        "<p class=note>A lock on the Net carries ● NET in its header and a jack-in row (j) "
        "above the Keyjack, force and card; off the Net, neither shows.</p>",
        "<h1>round 3 · the take panel</h1>"]
for key, title, locked, opened, note in ROUND3:
    body.append(f'<div class=opt><h2>{key} · {title} · locked</h2><div class=win>{pre(locked())}</div></div>'
                f'<div class=opt><h2>{key} · open</h2><div class=win>{pre(opened())}</div></div>'
                f'<p class=note>{html.escape(note)}</p>')
body.append("<h1>round 2</h1>")
for key, title, opt, wall, note in ROUND2:
    body.append(f'<div class=opt><h2>{key} · {title}</h2><div class=win>{pre(render(opt, wall))}</div>'
                f"<p>{html.escape(note)}</p>{legend(opt, False)}</div>")
body.append("<h1>round 1 · glyph options</h1>")
for key, title, opt, wall, note in OPTS:
    body.append(f'<div class=opt><h2>{key} · {title}</h2><div class=win>{pre(render(opt, wall))}</div>'
                f"<p>{html.escape(note)}</p>{legend(opt, wall)}</div>")
with open(OUT, "w") as f:
    f.write("<!doctype html><meta charset=utf-8><title>Containers mock</title>"
            f"<style>{CSS}{style_sheet()}</style>{''.join(body)}")
print("wrote", OUT)
