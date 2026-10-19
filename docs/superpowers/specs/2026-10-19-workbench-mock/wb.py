"""WORKBENCH concepts at 100x30, drawn over the game's own frame and the Ratchet's drawing.

    SHOT_DIR=../shots python3 wb.py out.html [screen ...]
"""

import sys
from cells import CSS, Grid, pixels, shrink, fold
from screen import *

WARN = MAGENTA
WARN_BG = "#2a0622"
GREEN = "#5ac88c"

# ---- the Ratchet, cut into its parts

RAT = Grid.from_svg(SHOTS + "blueprint_ratchet_100x30.svg", 100, 30)
ORIGIN = (8, 9)
REGIONS = {
    "receiver": lambda x, y: 27 <= x <= 65 and 9 <= y <= 14,
    "stock": lambda x, y: 6 <= x <= 26 and 9 <= y <= 16,
    "barrel": lambda x, y: 66 <= x <= 76 and 9 <= y <= 14,
    "mag": lambda x, y: (40 <= x <= 54 and 15 <= y <= 20) or (55 <= x <= 57 and y == 20),
    "trigger": lambda x, y: 55 <= x <= 66 and 15 <= y <= 19,
}


def cut_part(name):
    f = REGIONS[name]
    out = []
    for y in range(4, 24):
        for x in range(3, 96):
            c = RAT.c[y][x]
            if not f(x, y):
                continue
            if c[0] == " " and c[2] == PANEL:
                continue
            out.append((x - ORIGIN[0], y - ORIGIN[1], list(c)))
    return out


PARTS = {k: cut_part(k) for k in REGIONS}


def bounds(name, X, Y, off=(0, 0)):
    cs = PARTS[name]
    xs = [X + dx + off[0] for dx, _, _ in cs]
    ys = [Y + dy + off[1] for _, dy, _ in cs]
    return min(xs), min(ys), max(xs), max(ys)


def grey(c, k=0.38):
    if c is None:
        return c
    r, g, b = (int(c[i:i + 2], 16) for i in (1, 3, 5))
    v = (0.299 * r + 0.587 * g + 0.114 * b) * k
    ch = lambda x: max(0, min(255, round(x)))
    return "#%02x%02x%02x" % (ch(v), ch(v), ch(v * 1.08))


def stamp(g, name, X, Y, off=(0, 0), mode="colour"):
    """The part with the drawing's origin at X, Y, shifted by off. Panel backgrounds
    keep what is under them. `grey` as the modding mode greys; `ghost` darker still."""
    for dx, dy, c in PARTS[name]:
        x, y = X + dx + off[0], Y + dy + off[1]
        if not (0 <= x < g.w and 0 <= y < g.h):
            continue
        ch, fg, bg, bold = c
        under = g.c[y][x][2]
        if mode == "grey":
            fg, bg, bold = grey(fg), (under if bg == PANEL else grey(bg)), False
        elif mode == "ghost":
            fg, bg, bold = grey(fg, 0.26), (under if bg == PANEL else grey(bg, 0.26)), False
            if ch not in "▀▄█▌▐ ":
                ch = " "
        else:
            bg = under if bg == PANEL else bg
        g.c[y][x] = [ch, fg, bg, bold]


def corners(g, x0, y0, x1, y1, col=CYAN, heavy=True):
    a = "┏┓┗┛" if heavy else "┌┐└┘"
    for x, y, ch in [(x0 - 1, y0 - 1, a[0]), (x1 + 1, y0 - 1, a[1]), (x0 - 1, y1 + 1, a[2]), (x1 + 1, y1 + 1, a[3])]:
        g.put(x, y, ch, col)


def dashed(g, x0, y0, x1, y1, col):
    for x in range(x0, x1 + 1):
        g.put(x, y0, "╌", col)
        g.put(x, y1, "╌", col)
    for y in range(y0, y1 + 1):
        g.put(x0, y, "╎", col)
        g.put(x1, y, "╎", col)
    for x, y, ch in [(x0, y0, "┌"), (x1, y0, "┐"), (x0, y1, "└"), (x1, y1, "┘")]:
        g.put(x, y, ch, col)


def balloon(g, x, y, n, ok=True, sel=False):
    fg = (PANEL if sel else CYAN) if ok else (PANEL if sel else WARN)
    bg = (CYAN if ok else WARN) if sel else (BAR if ok else WARN_BG)
    g.put(x, y, " %d " % n, fg, bg=bg, bold=True)


# ---- the content the mockups show (the Ratchet's parts are placeholders)

BOM = [
    # (balloon, part, region, need, have)
    (1, "Stamped receiver", "receiver", 1, 1),
    (2, "Wire stock", "stock", 1, 1),
    (3, "Barrel 9×19", "barrel", 1, 0),
    (4, "Magazine", "mag", 1, 2),
    (5, "Trigger group", "trigger", 1, 1),
]

BUILDS = [
    ("electronics", [("Ratchet SMG", "4/5", False), ("Wire shiv", "ready", True), ("Hardhat visor", "1/2", False),
                     ("Optics tune", "ready", True), ("Servo rebuild", "2/6", False)]),
    ("medtech", [("Stim batch", "ready", True)]),
    ("streetwise", [("Fence caps", "ready", True)]),
]


# ======================================================================
# A · THE DRAWING SHEET: an exploded assembly drawing, a parts list, a title block

def concept_a(sel=2, ratchet_tb=None):
    g = base(active=3)
    sheet(g)
    # The exploded drawing.
    X, Y = 16, 7
    off = {"stock": (-5, 0), "barrel": (5, 0), "mag": (0, 1), "trigger": (0, 1), "receiver": (0, 0)}
    have = {r: h >= n for _, _, r, n, h in BOM}
    # Centre lines through the gaps, the way the parts come apart.
    for x in list(range(30, 35)) + list(range(74, 79)):
        g.put(x, 10, "╌", FAINT)
    for x in (56, 68):
        g.put(x, 13, "╎", FAINT)
    for r in ["stock", "receiver", "barrel", "trigger", "mag"]:
        stamp(g, r, X, Y, off[r], "colour" if have[r] else "ghost")
    # The missing part's outline.
    bx0, by0, bx1, by1 = bounds("barrel", X, Y, off["barrel"])
    dashed(g, bx0 - 1, by0, bx1 + 1, by1, WARN)
    g.put(bx0 - 1, by1 + 2, "none on hand", WARN)
    # Balloons and their leaders.
    spots = {1: (38, 6, [(39, 7, "│")]), 2: (17, 6, [(18, 7, "│")]), 3: (83, 6, [(84, 7, "│")]),
             4: (42, 17, [(45, 17, "─"), (46, 17, "─"), (47, 17, "─")]),
             5: (77, 16, [(75, 16, "─"), (76, 16, "─")])}
    for i, (n, part, r, need, hv) in enumerate(BOM):
        bx, by, lead = spots[n]
        for lx, ly, ch in lead:
            g.put(lx, ly, ch, CYAN if hv >= need else WARN)
        balloon(g, bx, by, n, hv >= need, i == sel)
    # The selected part's brackets.
    r = BOM[sel][2]
    bx0, by0, bx1, by1 = bounds(r, X, Y, off[r])
    corners(g, bx0 - 2, by0 - 1, bx1 + 2, by1 + 1)
    parts_list(g, [(n, part, need, hv) for n, part, _, need, hv in BOM], sel)
    title_block(g, ratchet_tb or [
        ("RATCHET SMG", CYAN, True),
        ("weapon · primary · 3.2 kg", DIM, False),
        ("needs scrapcraft · yours", DIM, False),
        ("1 more barrel 9×19", WARN, False),
        ("DWG 7/7 · assembly · drawn D'SHAWN", FAINT, False),
    ], "7/7")
    hints(g, "←→ drawings · ↑↓ parts · space assemble · tab tabs · esc close")
    return g


# ======================================================================
# B · THE BENCH: a cutting mat, the build in progress, the steps, the loose parts

MAT = "#06110f"
MAT_LINE = "#0f2622"
MAT_TICK = "#1f4a44"

STEPS = [
    ("lay the receiver", "done"),
    ("seat the trigger", "done"),
    ("pin the stock", "done"),
    ("slot the magazine", "now"),
    ("thread the barrel", "missing"),
]


def builds_list(g, x0, x1, y, sel_name, focused=True, w_tail=6):
    for skill, items in BUILDS:
        g.put(x0 + 2, y, "── %s ──" % skill, DIM)
        y += 1
        for name, tail, ok in items:
            sel = name == sel_name
            lit = sel and focused
            if lit:
                g.bg(x0, y, x1 - x0, BAR)
            g.put(x0 + 1, y, "▸" if sel else " ", LIT if lit else DIM, bold=lit)
            g.put(x0 + 3, y, name, LIT if lit else TEXT, bold=lit)
            g.put(x1 - len(tail) - 1, y, tail, (GREEN if ok else WARN) if not lit else LIT, bold=lit)
            y += 1
    return y


def concept_b():
    g = base(active=3)
    L = (1, 27)
    vrule(g, L[1], 5, 28, "║")
    y = builds_list(g, L[0], L[1], 5, "Ratchet SMG")
    # The steps of the selected build.
    y += 1
    hrule(g, L[0], L[1], y, "─")
    g.put(L[0] + 2, y, " ASSEMBLY ", DIM)
    y += 1
    for i, (step, state) in enumerate(STEPS):
        mark, col = {"done": ("✓", GREEN), "now": ("▸", LIT), "missing": ("✗", WARN)}[state]
        g.put(L[0] + 1, y, mark, col, bold=state == "now")
        g.put(L[0] + 3, y, "%d %s" % (i + 1, step), TEXT if state == "done" else col, bold=state == "now")
        y += 1
    g.put(L[0] + 3, y + 1, "step 4 of 5", FAINT)

    # The mat.
    mx0, mx1, my0, my1 = 28, 98, 5, 20
    g.fill(mx0, my0, mx1 - mx0 + 1, my1 - my0 + 1, MAT)
    for x in range(mx0, mx1 + 1):
        t = (x - mx0) % 10
        g.put(x, my0, "┴" if t == 0 else ("╵" if t == 5 else "·"), MAT_TICK if t in (0, 5) else MAT_LINE)
        if t == 0 and x + 1 <= mx1:
            g.put(x + 1, my0, str((x - mx0) // 10), MAT_TICK)
    for yy in range(my0 + 2, my1 + 1, 3):
        for x in range(mx0 + 5, mx1 + 1, 10):
            g.put(x, yy, "┼", MAT_LINE)
    X, Y = 29, 8
    # Built so far: receiver, trigger, stock. The magazine hovers by its well; the
    # barrel's place is an outline.
    for r in ["stock", "receiver", "trigger"]:
        stamp(g, r, X, Y)
    bx0, by0, bx1, by1 = bounds("barrel", X, Y)
    dashed(g, bx0, by0, min(bx1, mx1), by1, WARN)
    # the magazine, lifted out of its well, about to go in
    stamp(g, "mag", X, Y, (0, 2), "colour")
    mx_0, my_0, mx_1, my_1 = bounds("mag", X, Y, (0, 2))
    corners(g, mx_0, my_0, mx_1, my_1)

    # The loose parts, at the mat's foot.
    g.fill(mx0, 21, mx1 - mx0 + 1, 6, PANEL)
    hrule(g, mx0, mx1 + 1, 21, "─")
    g.put(mx0, 21, "╟", RULE)
    g.put(mx0 + 2, 21, " ON THE BENCH ", DIM)
    tiles = [("MAGAZINE", "2 in pack", True, True), ("COPPER COIL", "3 in pack", True, False),
             ("BARREL 9×19", "none", False, False)]
    tx = mx0 + 4
    for name, sub, ok, sel in tiles:
        w = max(len(name), len(sub)) + 5
        bg = "#3a3f47" if ok else "#1c1f24"
        fg = "#e8ecf0" if ok else FAINT
        g.fill(tx, 23, w, 2, bg)
        g.put(tx, 23, "▀▀", "#5c6670" if ok else "#2a2e34", bg="#1c1f24")
        g.put(tx, 24, "▀▀", "#5c6670" if ok else "#2a2e34", bg="#1c1f24")
        g.put(tx + 3, 23, name, fg, bold=ok)
        g.put(tx + 3, 24, sub, fg if ok else WARN)
        if sel:
            corners(g, tx, 23, tx + w - 1, 24, "#00a5b8")
        tx += w + 4
    g.put(mx0 + 2, 26, "MAGAZINE", CYAN, bold=True)
    g.put(mx0 + 11, 26, "▸ step 4 · slot the magazine into the well", "#e8e8ec")
    hints(g, "↑↓ builds · space fit the next part · backspace take one out · tab tabs · esc close")
    return g


# ======================================================================
# C · REGISTER + ASSEMBLY MODE: the tab lists what you can build, as GEAR lists what you
# carry; space opens the blueprint window in an assembly mode, a sibling of modding.

def preview_ratchet(missing=("barrel",)):
    """The Ratchet at half size, parts not on hand greyed."""
    canvas = Grid(100, 30, PANEL)
    for r in REGIONS:
        stamp(canvas, r, *ORIGIN, mode="grey" if r in missing else "colour")
    p = pixels(canvas, 6, 9, 72, 12, PANEL)
    if len(p) % 4:
        p += [[None] * len(p[0]) for _ in range(4 - len(p) % 4)]
    return fold(shrink(p, 2, 2, "mode"), PANEL)


def concept_c_tab():
    g = base(active=3)
    A, B, TOP, HR = (1, 49), (50, 99), (5, 14), 14
    # A: what you can build.
    y = TOP[0]
    rows = [("electronics", None)] + [(None, b) for b in BUILDS[0][1][:3]] + \
           [("medtech", None), (None, BUILDS[1][1][0]), ("streetwise", None), (None, BUILDS[2][1][0])]
    for head, b in rows:
        if head:
            g.put(A[0] + 3, y, "── %s ──" % head, DIM)
        else:
            name, tail, ok = b
            sel = name == "Ratchet SMG"
            icon = "/" if name == "Ratchet SMG" else ("◍" if name == "Stim batch" else ("▪" if name == "Fence caps" else "▣" if name == "Hardhat visor" else "/"))
            row(g, A[0], A[1], y, icon + " ", name, (), sel, True, tail=(tail, (GREEN if ok else WARN)))
        y += 1
    vrule(g, A[1], TOP[0], HR)
    # B: its parts, from the pack.
    g.put(B[0] + 3, TOP[0], "PARTS", DIM)
    g.put(B[1] - 12, TOP[0], "need  have", FAINT)
    for i, (n, part, r, need, hv) in enumerate(BOM):
        ok = hv >= need
        yy = TOP[0] + 1 + i
        g.put(B[0] + 2, yy, "■" if ok else "□", CYAN if ok else WARN)
        g.put(B[0] + 4, yy, part, TEXT if ok else WARN)
        g.put(B[1] - 11, yy, "%d" % need, DIM)
        g.put(B[1] - 5, yy, "%d" % hv, TEXT if ok else WARN)
    g.put(B[0] + 4, TOP[0] + 7, "+ electronics · yours", DIM)
    hrule(g, 1, 99, HR)
    g.put(A[1], HR, "╩", RULE)
    for x in range(A[0], A[1]):
        g.put(x, HR, "━", CYAN)
    # C: the card and the drawing, the missing part grey.
    card(g, 3, 15, 41, 12, [
        ("Ratchet SMG", CYAN, True),
        ("weapon · primary · 3.9 kg · ¤ 2150", DIM, False),
        ("4 of 5 parts · barrel missing", WARN, False),
        ("", TEXT, False),
        ("Stamped-receiver machine pistol with a filed-down selector. Jams on cheap ammo, sings on the good stuff.", TEXT, False),
    ])
    viewport(g, 46, 15, 52, 12, preview_ratchet(), "space assemble")
    hints(g, "↑↓ select · space assemble · tab tabs · esc close")
    return g


def window_frame(title):
    w = Grid.from_svg(SHOTS + "modding_the_ratchet_tray_100x30.svg", 100, 30)
    w.fill(3, 2, 94, 26, PANEL)
    # title row: ▗▄▄ TITLE ▄▄▄…▖
    for x in range(5, 97):
        w.c[1][x] = ["▄", CYAN, PANEL, False]
    w.put(5, 1, " %s " % title, CYAN, bold=True)
    return w


def concept_c_window():
    g = window_frame("ASSEMBLY · RATCHET SMG")
    X, Y = ORIGIN
    fitted = {"receiver", "stock", "trigger"}
    for r in REGIONS:
        if r in fitted:
            stamp(g, r, X, Y)
        else:
            stamp(g, r, X, Y, mode="ghost")
    # Empty places, as empty sockets: a dashed outline in what they take.
    for r in ("mag", "barrel"):
        x0, y0, x1, y1 = bounds(r, X, Y)
        dashed(g, x0 - 1, y0 - (1 if r == "barrel" else 0), x1 + 1, y1 + (1 if r == "barrel" else 0),
               FAINT if r == "mag" else WARN)
    x0, y0, x1, y1 = bounds("mag", X, Y)
    corners(g, x0 - 1, y0, x1 + 1, y1)
    # The PARTS tray.
    g.put(8, 22, "PARTS", "#6b6575")
    tx = 19
    for name, sub, sel in [("MAGAZINE", "2 in pack", True)]:
        w = max(len(name), len(sub)) + 4
        g.fill(tx + 2, 22, w - 2, 2, "#3a3f47")
        g.put(tx, 22, "▀▀", "#3a3f47", bg="#1c1f24")
        g.put(tx, 23, "▀▀", "#3a3f47", bg="#1c1f24")
        g.put(tx + 3, 22, name, "#e8ecf0", bg="#3a3f47", bold=True)
        g.put(tx + 3, 23, sub, "#e8ecf0", bg="#3a3f47")
        if sel:
            corners(g, tx, 22, tx + w - 1, 23, "#00a5b8")
    g.put(4, 25, "MAGAZINE", CYAN, bold=True)
    g.put(13, 25, "▸ mag well · 2 in pack · then 1 more: barrel 9×19 (none)", "#e8e8ec")
    g.put(4, 27, "←→ places · space fit · enter assemble when whole · esc back", DIM)
    return g



# ---- A for builds with no drawing: the same sheet, a process diagram in the field

def sheet(g):
    x0, x1, y0, y1 = 2, 97, 5, 26
    g.fill(x0, y0, x1 - x0 + 1, y1 - y0 + 1, PANEL)
    for x in range(x0, x1 + 1):
        g.put(x, y0, "─", RULE)
        g.put(x, y1, "─", RULE)
    for y in range(y0, y1 + 1):
        g.put(x0, y, "│", RULE)
        g.put(x1, y, "│", RULE)
    for x, y, ch in [(x0, y0, "┌"), (x1, y0, "┐"), (x0, y1, "└"), (x1, y1, "┘")]:
        g.put(x, y, ch, RULE)
    for i, zx in enumerate(range(x0 + 12, x1 - 4, 12)):
        g.put(zx, y0, "┬", RULE)
        g.put(zx - 6, y0, str(i + 1), FAINT)
    for i, zy in enumerate(range(y0 + 4, 20, 4)):
        g.put(x0, zy, "├", RULE)
        g.put(x0, zy - 2, "ABCD"[i], FAINT)
    hrule(g, x0 + 1, x1, 20, "─")
    g.put(x0, 20, "├", RULE)
    g.put(x1, 20, "┤", RULE)
    g.put(61, 20, "┬", RULE)
    vrule(g, 61, 21, 26, "│")
    g.put(61, 26, "┴", RULE)


def parts_list(g, bom, sel):
    # The header rides the rule, as GEAR's weight does.
    for x, t in [(5, " # "), (8, " PART "), (31, " QTY "), (37, " ON HAND ")]:
        g.put(x, 20, t, FAINT)
    for i, (n, part, need, hv) in enumerate(bom):
        y = 21 + i
        lit = i == sel
        ok = hv >= need
        if lit:
            g.bg(3, y, 58, BAR)
        col = LIT if lit else (TEXT if ok else WARN)
        g.put(4, y, "▸" if lit else " ", LIT)
        g.put(6, y, "%d" % n, col, bold=lit)
        g.put(9, y, part, col, bold=lit)
        g.put(33, y, "%d" % need, col)
        g.put(39, y, "%d" % hv, col)
        g.put(45, y, "■" if ok else "□", CYAN if ok else WARN)
        if not ok:
            g.put(48, y, "not in pack", WARN)


def title_block(g, lines, idx):
    for i, (t, c, b) in enumerate(lines):
        g.put(63, 21 + i, t, c, bold=b)
    g.put(87, 21, "◂ %s ▸" % idx, DIM)


def box(g, x0, y0, x1, y1, col, heavy=False):
    h, v, a = ("═", "║", "╔╗╚╝") if heavy else ("─", "│", "┌┐└┘")
    for x in range(x0, x1 + 1):
        g.put(x, y0, h, col)
        g.put(x, y1, h, col)
    for y in range(y0, y1 + 1):
        g.put(x0, y, v, col)
        g.put(x1, y, v, col)
    for x, y, ch in [(x0, y0, a[0]), (x1, y0, a[1]), (x0, y1, a[2]), (x1, y1, a[3])]:
        g.put(x, y, ch, col)


def part_box(g, x0, y, w, n, icon, name, need, hv, sel=False):
    ok = hv >= need
    col = CYAN if sel else (RULE if ok else WARN)
    if ok:
        box(g, x0, y, x0 + w - 1, y + 2, col)
    else:
        dashed(g, x0, y, x0 + w - 1, y + 2, col)
    g.put(x0 + 2, y + 1, icon, DIM if ok else WARN, bold=True)
    g.put(x0 + 4, y + 1, name, TEXT if ok else WARN)
    t = "×%d" % need
    g.put(x0 + w - len(t) - 2, y + 1, t, DIM if ok else WARN)
    balloon(g, x0 - 4, y + 1, n, ok, sel)


def concept_a_process(which="stim", sel=0):
    g = base(active=3)
    sheet(g)
    if which == "stim":
        ins = [(1, "▪", "Copper coil", 1, 3), (2, "◍", "Soy-noodle cup", 1, 2)]
        skill, steps, out_icon, out_name, out_n = "MEDTECH", ["boil", "filter", "pray"], "◍", "Stim patch", 2
        tb = [("STIM BATCH", CYAN, True), ("makes 2× stim patch · consumable", DIM, False),
              ("needs medtech · yours", DIM, False), ("every part on hand", GREEN, False),
              ("DWG 6/7 · process · drawn D'SHAWN", FAINT, False)]
        idx = "6/7"
    else:
        ins = [(1, "▪", "Scrap servo", 4, 2), (2, "▪", "Copper coil", 2, 3)]
        skill, steps, out_icon, out_name, out_n = "ELECTRONICS", ["strip", "rewind", "seat"], "◈", "Grip arm", 1
        tb = [("SERVO REBUILD", CYAN, True), ("makes grip arm · cyberware · arms", DIM, False),
              ("needs electronics · yours", DIM, False), ("2 more scrap servo", WARN, False),
              ("DWG 4/7 · process · drawn D'SHAWN", FAINT, False)]
        idx = "4/7"
    # The inputs, stacked down the left of the field.
    W = 26
    ys = [7, 13] if len(ins) == 2 else [6, 10, 14]
    for (n, icon, name, need, hv), y in zip(ins, ys):
        part_box(g, 12, y, W, n, icon, name, need, hv, sel=(n - 1 == sel))
    # Their lines, meeting and running into the process.
    jx = 12 + W + 6
    for (n, *_), y in zip(ins, ys):
        for x in range(12 + W, jx):
            g.put(x, y + 1, "─", FAINT)
    top, bot = ys[0] + 1, ys[-1] + 1
    for y in range(top, bot + 1):
        g.put(jx, y, "│", FAINT)
    g.put(jx, top, "┐", FAINT)
    g.put(jx, bot, "┘", FAINT)
    mid = (top + bot) // 2
    g.put(jx, mid, "├", FAINT)
    for x in range(jx + 1, jx + 5):
        g.put(x, mid, "─", FAINT)
    g.put(jx + 5, mid, "▶", FAINT)
    # The process: the skill, the steps.
    px0, px1 = jx + 7, jx + 26
    box(g, px0, mid - 3, px1, mid + 3, DIM, heavy=True)
    g.put(px0 + (px1 - px0 - len(skill)) // 2 + 1, mid - 3, skill, CYAN, bold=True)
    for i, st in enumerate(steps):
        g.put(px0 + 3, mid - 1 + i, "%d %s" % (i + 1, st), TEXT)
    # Out to the result.
    for x in range(px1 + 1, px1 + 5):
        g.put(x, mid, "─", FAINT)
    g.put(px1 + 5, mid, "▶", FAINT)
    ox0 = px1 + 7
    box(g, ox0, mid - 1, 95, mid + 1, CYAN)
    g.put(ox0 + 2, mid, out_icon, CYAN, bold=True)
    g.put(ox0 + 4, mid, "%d× %s" % (out_n, out_name), CYAN, bold=True)
    parts_list(g, [(n, name, need, hv) for n, _, name, need, hv in ins], sel)
    title_block(g, tb, idx)
    hints(g, "←→ drawings · ↑↓ parts · space build · tab tabs · esc close")
    return g


SCREENS = {
    "a_sheet": concept_a,
    "a_process": concept_a_process,
    "a_process_servo": lambda: concept_a_process("servo", 0),
    "b_bench": concept_b,
    "c_tab": concept_c_tab,
    "c_window": concept_c_window,
}

if __name__ == "__main__":
    out = sys.argv[1]
    which = sys.argv[2:] or list(SCREENS)
    page = ["<html><head><meta charset=utf-8><style>body{background:#0b0d12;margin:16px}%s</style></head><body>" % CSS]
    for k in which:
        page.append("<p style='color:#888;font:12px sans-serif'>%s</p>" % k)
        page.append(SCREENS[k]().html())
    page.append("</body></html>")
    open(out, "w").write("".join(page))
