"""Ripperdoc mockups, round 1: the chair (A ledger, B chair screen, C work order) and
three clinics on the map. Rounds 2 and 3 (gen2.py, gen3.py) import from here.

    SHOTS=dir/ python3 gen.py round1.html

SHOTS holds the shot harness's SVGs (SHOT_DIR=dir/ cargo test -p strata-zero shots::
-- --ignored): attributes_ledger_limb, attributes_picker_site and _swap at 100x30, and
throwaway shots taken for this mock (tmp_clinic_*, tmp_picker_over, tmp_pop_chair,
tmp_pop_closed: a clinic's floor, an over-budget pick, the chair's window and the shut
door, drawn with the game's Popup). round1-3.html are the approved pages."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")

CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#5c6673"
TEXT = "#cdd4de"
PANEL = "#04060a"
RULE = "#283c4b"
BAR = "#003c46"
GOOD = "#50e678"
AMBER = "#ffc850"
RED = "#ff4f5e"
BLOOD = "#c8323c"
STEEL = "#9fd8e8"

# interior palette, from the clinic shot
WALL = "#6c4f42"
WALL_BG = "#090a0e"
FLOOR_BG = "#271d19"
FLOOR_DOT = "#3e2f29"
COLD_BG = "#0f1c22"
COLD_DOT = "#24363f"
DOOR = "#dd8223"
DOOR_BG = "#231a17"
WIN = "#bf0096"
LAMP = "#fff0c8"
FURN = "#9c7260"
COUNTER = "#d8873a"
COLD_FURN = "#7fb6c8"

# ---------------------------------------------------------------- the map

JOIN = {
    (0, 0, 0, 0): "─", (1, 0, 0, 0): "│", (0, 1, 0, 0): "│", (1, 1, 0, 0): "│",
    (0, 0, 1, 0): "─", (0, 0, 0, 1): "─", (0, 0, 1, 1): "─",
    (1, 0, 1, 0): "┘", (1, 0, 0, 1): "└", (0, 1, 1, 0): "┐", (0, 1, 0, 1): "┌",
    (1, 1, 1, 0): "┤", (1, 1, 0, 1): "├", (1, 0, 1, 1): "┴", (0, 1, 1, 1): "┬",
    (1, 1, 1, 1): "┼",
}
WALLISH = set("#+w~^")


def furniture(ch, cold):
    dot, bg = (COLD_DOT, COLD_BG) if cold else (FLOOR_DOT, FLOOR_BG)
    f = COLD_FURN if cold else FURN
    return {
        ".": ("·", dot, bg),
        "K": ("═", COUNTER, bg),
        "S": ("·", f, bg),
        "H": ("≣", f, bg),
        "M": ("⚙", f, bg),
        "B": ("▤", f, bg),
        "L": ("▮", f, bg),
        "F": ("▮", COLD_FURN, bg),
        "C": ("▬", f, bg),
        "O": ("○", f, bg),
        "D": ("▭", f, bg),
        "T": ("▭", STEEL, bg),
        "V": ("◘", "#3fa0b0", bg),
        "*": ("●", LAMP, bg),
        "X": ("╦", CYAN, "#163640"),
        "=": ("▦", "#3a4a52", bg),
        "@": ("@", "#ffffff", bg),
        "d": ("@", GOOD, bg),
        "n": ("@", DIM, bg),
        "v": ("▼", DOOR, bg),
        "a": ("▲", DOOR, bg),
    }.get(ch)


def stamp(g, ox, oy, rows, cold_rooms=()):
    """Draws a layout at (ox, oy): walls joined, doors, windows, curtains; tiles in a
    cold rect take the surgery's light."""
    h, w = len(rows), max(len(r) for r in rows)
    at = lambda x, y: rows[y][x] if 0 <= y < h and 0 <= x < len(rows[y]) else " "

    def cold(x, y):
        return any(x0 <= x <= x1 and y0 <= y <= y1 for x0, y0, x1, y1 in cold_rooms)

    for y in range(h):
        for x in range(len(rows[y])):
            c = rows[y][x]
            if c == " ":
                continue
            if c in WALLISH:
                n = (at(x, y - 1) in WALLISH, at(x, y + 1) in WALLISH,
                     at(x - 1, y) in WALLISH, at(x + 1, y) in WALLISH)
                if c == "#":
                    g.put(ox + x, oy + y, JOIN[tuple(int(b) for b in n)], WALL, bg=WALL_BG)
                elif c == "+":
                    g.put(ox + x, oy + y, "▯", DOOR, bg=DOOR_BG)
                elif c == "w":
                    g.put(ox + x, oy + y, "▫", WIN, bg=WALL_BG)
                elif c == "~":
                    g.put(ox + x, oy + y, "┆", MAGENTA, bg=FLOOR_BG)
                elif c == "^":
                    g.put(ox + x, oy + y, "▯", "#7a6a60", bg=WALL_BG)
                continue
            glyph = furniture(c, cold(x, y))
            if glyph:
                ch, fg, bg = glyph
                g.put(ox + x, oy + y, ch, fg, bg=bg, bold=c in "@dX")


def map_frame(title, here, line2=None):
    g = Grid.from_svg(S + "tmp_clinic_1_Night_Market_100x30.svg", 100, 30)
    # the generic building's outline goes; fog from elsewhere on the map fills it
    for y in range(5, 25):
        for x in range(0, 59):
            if g.c[y][x][0] not in "░▒▓█":
                g.c[y][x] = list(g.c[(y + 9 - 5) % 20 + 5][(x + 31) % 59])
            if g.c[y][x][0] not in "░▒▓█":
                g.c[y][x] = ["▒", "#120e0c", PANEL, False]
    g.put(61, 0, " " * 30, CYAN)
    g.put(61, 0, title, CYAN, bold=True)
    for y in range(5, 25):
        g.put(60, y, " " * 39, TEXT, bg=PANEL)
    g.put(61, 5, here, TEXT)
    g.put(60, 6, "─" * 40, RULE)
    if line2:
        for i, (t, col) in enumerate(line2):
            g.put(61, 7 + i, t, col)
    g.put(60, 23, " " * 39, TEXT)
    g.put(61, 23, "[F5]", CYAN)
    g.put(65, 23, "Inspect ", TEXT)
    g.put(73, 23, "[F6]", CYAN)
    g.put(77, 23, "Legend", TEXT)
    return g


def popup(g, x, y, w, title, sub, rows, foot=None):
    """A Space window: the name in its colour, a subtitle, lettered rows."""
    h = len(rows) + 4 + (1 if foot else 0)
    for j in range(h):
        g.put(x, y + j, " " * w, TEXT, bg="#070a10")
    g.put(x, y, "┌" + "─" * (w - 2) + "┐", CYAN, bg="#070a10")
    for j in range(1, h - 1):
        g.put(x, y + j, "│", CYAN, bg="#070a10")
        g.put(x + w - 1, y + j, "│", CYAN, bg="#070a10")
    g.put(x, y + h - 1, "└" + "─" * (w - 2) + "┘", CYAN, bg="#070a10")
    g.put(x + 2, y + 1, title[0], title[1], bold=True)
    g.put(x + 2, y + 2, sub, DIM)
    for i, row in enumerate(rows):
        cx = x + 2
        for t, col, *b in row:
            g.put(cx, y + 3 + i, t, col, bold=bool(b and b[0]))
            cx += len(t)
    if foot:
        g.put(x + 2, y + h - 2, foot, FAINT)


NOODLE = [
    "###############################",
    "#F..T........F#H.M.M.M.M....HH#",
    "#.....X.......#...............#",
    "#.*...=..d....~.......*....n..#",
    "#F............#...............#",
    "###############KKKKKKKKKKKKKK.#",
    "#........*....S.S.S.S.S.S.S...#",
    "#.............................#",
    "#..O.S...O.S.....@............#",
    "#w###w####w####+####w####w####w",
]

CELLAR = [
    "################################",
    "#HHHH......#....M..M...........#",
    "#..........#............*......#",
    "#..*.......+......X............#",
    "#HHHH......#......=..d.....B...#",
    "#..........#..........T....B...#",
    "######+#####...................#",
    "#a.@.......#..............*....#",
    "################################",
]

DENTIST = [
    "##################################",
    "#C.C.......D.n.#.LLF..F#B........#",
    "#..............#.......#.......*.#",
    "#..V.....*.....+...X...+.........#",
    "#C.C...........#...=.T.#B........#",
    "#........@.....#..d..*.#.........#",
    "#w####w####+####################w#",
]


def clinic_maps():
    out = []
    g = map_frame("Hot Pot Lou · L", "Hot Pot Lou · noodle bar",
                  [("A curtain behind the stoves. The cook", DIM),
                   ("doesn't look up when you walk through.", DIM)])
    stamp(g, 14, 8, NOODLE, cold_rooms=[(1, 1, 13, 4)])
    out.append(("A", "Behind the noodle bar",
                "A noodle bar on the Night Market. The street sees a counter and stools; "
                "past the stoves a bead curtain (┆) opens on the back room: the chair (╦) "
                "under a cold lamp, a tray, coolers of chrome, a drain in the floor. The doc "
                "is the cook's sister.", g))

    g = map_frame("Laundromat 24 · B1", "Laundromat 24 · basement",
                  [("Racks of arms on hooks. Somebody", DIM),
                   ("hosed the floor down an hour ago.", DIM)])
    stamp(g, 13, 9, CELLAR, cold_rooms=[(12, 1, 30, 7)])
    out.append(("B", "Down the cellar stair",
                "The street door is a laundromat's; the stair (▲) goes down to a cellar. A "
                "stockroom of racked salvage on the left, the surgery on the right: the chair, "
                "a generator and an autoclave thumping, two recovery cots. Its own map on B1.", g))

    g = map_frame("Saint Ida Dental · L", "Saint Ida Dental · dentist",
                  [("The sign still says DENTAL. The", DIM),
                   ("fish tank has no fish.", DIM)])
    stamp(g, 12, 10, DENTIST, cold_rooms=[(16, 1, 23, 5)])
    out.append(("C", "The shuttered dentist",
                "An Old Town dental surgery that never quite closed. A waiting room of couches "
                "and a dead fish tank (◘), a receptionist at the desk, the surgery through the "
                "middle door, a recovery room with two beds at the back.", g))
    return out


# ---------------------------------------------------------------- the chair

def deck():
    return Grid.from_svg(S + "attributes_ledger_limb_100x30.svg", 100, 30)


def hint(g, text, col=DIM, x=2):
    g.put(1, 28, " " * 98, col, bg=PANEL)
    g.put(x, 28, text, col)


def chair_a():
    g = deck()
    # the header: in the chair
    g.put(48, 4, " " * 50, TEXT)
    g.put(48, 4, "CHROME", CYAN, bold=True)
    g.put(55, 4, "· IN DOC LOU'S CHAIR", AMBER, bold=True)
    g.put(76, 4, "── LOAD ", RULE)
    g.put(84, 4, "▰▰▰▰▰▰▰▰▰▰▱▱", CYAN)
    g.put(97, 4, " ", TEXT)
    g.put(84 - 0, 4, "▰▰▰▰▰▰▰▰▰", CYAN)
    g.put(93, 4, "19/24", TEXT)
    for y in range(4, 27):
        g.put(46, y, "║", AMBER)
    hint(g, "↑↓ select · space what fits · x pull it out · ← attributes · esc get out of the chair")
    # the doc's line where the flavour text was
    for y in range(22, 26):
        g.put(67, y, " " * 31, TEXT)
    g.put(68, 22, "Doc Lou:", GOOD, bold=True)
    g.put(68, 23, "“Hold still. Or don't. It's", DIM)
    g.put(68, 24, " your arm.”", DIM)

    h = deck()
    for y in range(4, 5):
        pass
    hint(h, "↑↓ select · ← attributes · Q/E tabs · tab close", DIM)
    h.put(50, 28, "space · x  only in a ripperdoc's chair", FAINT)
    return g, h


def chair_b():
    g = Grid(100, 30, PANEL)
    edge = AMBER
    g.put(0, 0, "┏" + "━" * 98 + "┓", edge)
    for y in range(1, 29):
        g.put(0, y, "┃", edge)
        g.put(99, y, "┃", edge)
    g.put(0, 29, "┗" + "━" * 98 + "┛", edge)
    g.put(3, 0, " THE CHAIR · DOC LOU · Hot Pot Lou, back room ", AMBER, bold=True)
    g.put(78, 0, " LOAD 19/24 ", CYAN, bold=True)

    # left: the body, site by site
    sites = [
        ("NEURAL", "Neural port", 2), ("EYES", "Target lens", 1), ("EARS", "—", 0),
        ("JAW", "Bulldog jaw", 1), ("SPINE", "Redline spine", 3), ("BACK", "Drone cradle", 2),
        ("ORGANS", "Adrenal pump", 2), ("SKIN", "Subdermal mesh", 2),
        ("L ARM", "Piston arm", 3), ("R ARM", "—", 0), ("LEGS", "—", 0),
    ]
    g.put(3, 2, "ON THE BODY", CYAN, bold=True)
    g.put(15, 2, "─" * 23, RULE)
    for i, (site, piece, load) in enumerate(sites):
        y = 4 + i * 2
        sel = site == "R ARM"
        if sel:
            g.bg(2, y, 37, BAR)
        g.put(3, y, ("▸ " if sel else "  ") + site.ljust(8), CYAN if sel else DIM, bold=sel)
        g.put(14, y, piece, TEXT if piece != "—" else FAINT, bold=sel)
        if load:
            g.put(34, y, "◆" * load, FAINT)
    for y in range(2, 27):
        g.put(41, y, "│", RULE)

    # middle: a figure, the pointed site lit
    fig = [
        "      ▄▄▄      ",
        "     █████     ",
        "      ▀█▀      ",
        "   ▄▄█████▄▄   ",
        "  ██ █████ ██  ",
        "  ██ █████ ██  ",
        "  ▀▀ █████ ▀▀  ",
        "     ██ ██     ",
        "     ██ ██     ",
        "     ██ ██     ",
        "     ▀▀ ▀▀     ",
    ]
    for j, row in enumerate(fig):
        for i, ch in enumerate(row):
            if ch == " ":
                continue
            col = "#2a3a44"
            if j in (4, 5, 6) and i in (2, 3):  # the right arm (her right, our left)
                col = CYAN
            if j in (4, 5, 6) and i in (11, 12):
                col = "#6b7f8a"
            g.put(44 + i, 6 + j, ch, col)
    g.put(44, 19, " R ARM · empty ", CYAN, bold=True)
    g.put(44, 20, " sockets: hand,", FAINT)
    g.put(44, 21, "  forearm", FAINT)
    for y in range(2, 27):
        g.put(61, y, "│", RULE)

    # right: the tray, what in the pack fits the pointed site
    g.put(64, 2, "ON THE TRAY", CYAN, bold=True)
    g.put(76, 2, "─ fits R ARM ─────", RULE)
    tray = [("Grip arm", "BODY +1 · carry +4", "¤ —"),
            ("Piston arm", "BODY +1 · Shove", "¤ —")]
    for i, (n, line, fee) in enumerate(tray):
        y = 4 + i * 3
        sel = i == 0
        if sel:
            g.bg(63, y, 35, BAR)
            g.bg(63, y + 1, 35, BAR)
        g.put(64, y, ("▸ " if sel else "  ") + n, CYAN if sel else TEXT, bold=True)
        g.put(66, y + 1, line, DIM)
        g.put(92, y, fee, AMBER)
    g.put(64, 12, "FEE", FAINT)
    g.put(68, 12, "per piece · later", FAINT)
    g.put(64, 22, "Doc Lou:", GOOD, bold=True)
    g.put(64, 23, "“Anaesthetic's extra.", DIM)
    g.put(64, 24, " Regret's free.”", DIM)

    g.put(3, 28, "↑↓ site · → tray · space put it in · x take it out · esc get up", DIM)
    return g


def chair_c():
    g = map_frame("Hot Pot Lou · L", "Doc Lou · ripperdoc",
                  [("at the chair in the back room", DIM)])
    stamp(g, 14, 8, NOODLE, cold_rooms=[(1, 1, 13, 4)])
    for y in range(5, 25):
        for x in range(0, 59):
            c = g.c[y][x]
            c[1] = "#3a3f47" if c[0] not in "@" else c[1]
    rows = [
        [("“Anaesthetic's extra. Regret's free.”", DIM)],
        [("", TEXT)],
        [("PUT IN", CYAN, True)],
        [("a ", AMBER, True), ("Grip arm          ", TEXT), ("→ R ARM          ", DIM), ("¤ —", AMBER)],
        [("b ", AMBER, True), ("Monowire spool    ", TEXT), ("→ L ARM forearm  ", DIM), ("¤ —", AMBER)],
        [("TAKE OUT", CYAN, True)],
        [("c ", AMBER, True), ("Piston arm        ", TEXT), ("← L ARM          ", DIM), ("¤ —", AMBER)],
        [("d ", AMBER, True), ("Target lens       ", TEXT), ("← EYES           ", DIM), ("¤ —", AMBER)],
        [("e ", AMBER, True), ("Redline spine     ", TEXT), ("← SPINE          ", DIM), ("¤ —", AMBER)],
        [("  ", AMBER), ("…4 more", FAINT)],
        [("", TEXT)],
        [("t ", AMBER, True), ("Talk", TEXT)],
    ]
    popup(g, 6, 7, 47, ("DOC LOU", GOOD), "ripperdoc · LOAD 19/24", rows, "esc leave the chair")
    return g


# ---------------------------------------------------------------- the page

def page(path):
    a_in, a_out = chair_a()
    chairs = [
        ("A", "The chair unlocks the ledger",
         "Space at the chair: <b>Sit in the chair</b>. The deck opens on ATTRIBUTES, the ledger "
         "in an operating mode: an amber rule, the doc named in the CHROME header, the doc's "
         "line where the flavour text sits, <i>esc gets out of the chair</i>. Everything else is "
         "today's ledger. Anywhere else (right) the ledger only looks; its hint says why.",
         [a_in, a_out]),
        ("B", "The chair is its own screen",
         "Sitting down opens a new device: the body site by site on the left, a figure with "
         "the pointed site lit, and the tray on the right: what in your pack fits that site, "
         "each with its fee. The ATTRIBUTES ledger becomes read-only everywhere.",
         [chair_b()]),
        ("C", "The doc takes a work order",
         "You talk to the doc, not the chair: a Space window of lettered jobs, PUT IN and "
         "TAKE OUT, each with its fee, one key a piece. No pointer, no screen; a swap is a "
         "take-out then a put-in. Fast, shop-like, and the least to build.",
         [chair_c()]),
    ]
    maps = clinic_maps()

    def term(g, scale):
        return g.html(scale)

    parts = ["<style>%s .term{margin:6px 0} .pair{display:flex;gap:14px;flex-wrap:wrap} "
             ".opt{border:1px solid #2a3340;border-radius:8px;padding:12px 16px;margin:14px 0;cursor:pointer}"
             ".opt.selected{border-color:#00e5ff;box-shadow:0 0 0 1px #00e5ff}"
             ".opt h3{margin:0 0 4px} .opt p{margin:0 0 8px;max-width:1000px;line-height:1.45}"
             ".tag{display:inline-block;background:#00e5ff;color:#000;font-weight:700;border-radius:4px;"
             "padding:0 7px;margin-right:8px}</style>" % CSS]
    parts.append("<h2>Ripperdocs · round 1</h2>")
    parts.append('<p class="subtitle">Two questions on one page: what sitting in the chair does, '
                 "and what a clinic looks like on the map. Click one of each if you like; "
                 "answer in the terminal. All drawn at 100×30 on the game's real frames.</p>")
    parts.append("<h3>1 · The chair</h3>")
    parts.append('<div class="options">')
    for key, name, desc, grids in chairs:
        parts.append('<div class="opt" data-choice="chair-%s" onclick="toggleSelect(this)">' % key.lower())
        parts.append('<h3><span class="tag">%s</span>%s</h3><p>%s</p><div class="pair">' % (key, name, desc))
        scale = 0.78 if len(grids) > 1 else 0.9
        for gr in grids:
            parts.append(term(gr, scale))
        parts.append("</div></div>")
    parts.append("</div>")
    parts.append("<h3>2 · The clinic</h3>")
    parts.append('<p class="subtitle">Three hand-laid interiors, each a front business with the '
                 "chair in back. Fully explored here; the cold-lit tiles are the surgery. "
                 "╦ is the chair, green @ the doc, grey @ the front's staff.</p>")
    parts.append('<div class="options">')
    for key, name, desc, gr in maps:
        parts.append('<div class="opt" data-choice="clinic-%s" onclick="toggleSelect(this)">' % key.lower())
        parts.append('<h3><span class="tag">%s</span>%s</h3><p>%s</p>' % (key, name, desc))
        parts.append(term(gr, 0.9))
        parts.append("</div>")
    parts.append("</div>")
    open(path, "w").write("\n".join(parts))


if __name__ == "__main__":
    page(sys.argv[1])
