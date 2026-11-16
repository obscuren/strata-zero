"""#103: the chrome in you on GEAR & INVENTORY, three ways, over the game's real 120 x 40
GEAR frame, with the ATTRIBUTES ledger's loadout (the debug runner with the attributes
shot's chrome: a Piston arm with an Arc hand on L ARM, a Drone cradle with a Scout drone).

    SHOT_DIR=<dir> cargo test -p strata-zero --target-dir target/smoke -- --ignored \\
        attributes_sheet gear_and_inventory
    python3 -I gen.py <dir> <out.html> [round]
"""

import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-16-equipment-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")
ROUND = sys.argv[3] if len(sys.argv) > 3 else "1"

BG = "#04060a"
BORDER = "#283c4b"
FAINT_BORDER = "#16212b"
LABEL = "#a0aab9"
EMPTY = "#5c6673"
FAINT = "#3a4450"
TEXT = "#cdd4de"
CYAN = "#00e5ff"
CYAN_DIM = "#00a5b8"
SEL_BG = "#003c46"
SOCKET = "#3f7c87"
MAGENTA = "#ff2bd6"
GREEN = "#5ad46a"

W, H = 120, 40
SEP = 62
OX, OY = 2, 11

# The loadout: each site, its piece, and what it gives in short.
CHROME = {
    "NEURAL": ("Neural port", "NET +1"),
    "EYES": ("Target lens", "REF +1"),
    "EARS": None,
    "JAW": ("Bulldog jaw", "Bite"),
    "SPINE": ("Redline spine", "MOV +1"),
    "BACK": ("Drone cradle", "1 bay"),
    "ORGANS": ("Adrenal pump", "HP +10"),
    "SKIN": ("Subdermal mesh", "ARM +2"),
    "L ARM": ("Piston arm", "BODY +1"),
    "R ARM": None,
    "LEGS": None,
}


def frame():
    g = Grid.from_svg(os.path.join(S, "gear_and_inventory_rack_120x40.svg"), W, H)
    g.fill(1, 9, W - 2, 28, BG)
    g.fill(1, 37, W - 2, 2, BG)
    for y in range(9, 37):
        g.put(SEP, y, "║", BORDER)
    return g


def header(g, chrome=True, load=True):
    """The view switch and, on chrome, the LOAD gauge, over the rule."""
    if chrome:
        g.put(2, 9, "MEAT", LABEL)
        g.put(8, 9, "▌", CYAN)
        g.put(9, 9, "CHROME", CYAN, bold=True)
        g.put(15, 9, "▐", CYAN)
    else:
        g.put(2, 9, "▌", CYAN)
        g.put(3, 9, "MEAT", CYAN, bold=True)
        g.put(7, 9, "▐", CYAN)
        g.put(10, 9, "CHROME", LABEL)
    g.put(19, 9, "c", CYAN)
    g.put(21, 9, "switch", EMPTY)
    if load:
        g.put(SEP - 25, 9, "LOAD ", LABEL)
        g.put(SEP - 20, 9, "▰" * 10, CYAN)
        g.put(SEP - 10, 9, "▱▱", BORDER)
        g.put(SEP - 7, 9, "19/24", TEXT, bold=True)
    g.put(1, 10, "─" * (SEP - 1), BORDER)


def tile(g, x, y, w, h, label, lines=None, sel=False, faint=False, sel_line=None):
    """A part's tile. `lines` are (text, colour) runs per row; `sel_line` lights one row
    instead of the whole tile."""
    bc = CYAN if sel else (FAINT_BORDER if faint else BORDER)
    g.put(x, y, "┌" + "─" * (w - 2) + "┐", bc)
    g.put(x + 1, y, " %s " % label, CYAN_DIM if sel else (FAINT if faint else LABEL), bold=sel)
    for j in range(1, h - 1):
        g.put(x, y + j, "│", bc)
        g.put(x + w - 1, y + j, "│", bc)
        g.fill(x + 1, y + j, w - 2, 1, SEL_BG if (sel and sel_line is None) else BG)
    g.put(x, y + h - 1, "└" + "─" * (w - 2) + "┘", bc)
    if lines is None:
        if not faint:
            g.put(x + w // 2, y + h // 2, "·", EMPTY)
        return
    top = y + 1 + max(0, (h - 2 - len(lines)) // 2)
    for i, runs in enumerate(lines):
        cx = x + 1
        lit = sel_line == i
        if lit:
            g.fill(x + 1, top + i, w - 2, 1, SEL_BG)
        for text, colour in runs:
            text = text[: x + w - 1 - cx]
            g.put(cx, top + i, text, CYAN if (lit or (sel and sel_line is None)) and colour == TEXT
                  else colour, bold=lit or (sel and sel_line is None and colour == TEXT))
            cx += len(text)


def hline(g, a, b):
    ax, ay, aw, ah = a
    bx = b[0]
    y = ay + ah // 2
    g.put(ax + aw - 1, y, "├", BORDER)
    for x in range(ax + aw, bx):
        g.put(x, y, "─", BORDER)
    g.put(bx, y, "┤", BORDER)


def vline(g, a, b, x):
    ax, ay, aw, ah = a
    by = b[1]
    g.put(x, ay + ah - 1, "┬", BORDER)
    for y in range(ay + ah, by):
        g.put(x, y, "│", BORDER)
    g.put(x, by, "┴", BORDER)


def name2(name):
    """A piece's name on two lines of 9, as the meat tiles wrap it."""
    words, a, b = name.split(), "", ""
    for wd in words:
        if not b and len((a + " " + wd).strip()) <= 9:
            a = (a + " " + wd).strip()
        else:
            b = (b + " " + wd).strip()
    if len(b) > 9:
        b = b[:8] + "…"
    return [a, b]


# ---------------------------------------------------------------- the body, meat places

SMALL, BIG = (11, 5), (12, 7)
POS = {
    "head": (24, 0) + SMALL, "face": (36, 0) + SMALL,
    "lhand": (0, 7) + SMALL, "larm": (12, 7) + SMALL, "torso": (24, 6) + BIG,
    "rarm": (37, 7) + SMALL, "rhand": (49, 7) + SMALL,
    "back": (12, 14) + SMALL, "waist": (24, 14) + SMALL,
    "legs": (24, 20) + SMALL, "feet": (36, 20) + SMALL,
}
SPINE_X, BACK_J = 33, 20


def at(key):
    x, y, w, h = POS[key]
    return (OX + x, OY + y, w, h)


def lines_of_body(g):
    hline(g, at("head"), at("face"))
    hline(g, at("lhand"), at("larm"))
    hline(g, at("larm"), at("torso"))
    hline(g, at("torso"), at("rarm"))
    hline(g, at("rarm"), at("rhand"))
    hline(g, at("legs"), at("feet"))
    vline(g, at("head"), at("torso"), OX + SPINE_X)
    vline(g, at("waist"), at("legs"), OX + SPINE_X)
    ty = OY + 13
    g.put(OX + SPINE_X, OY + 12, "┬", BORDER)
    g.put(OX + SPINE_X, ty, "┤", BORDER)
    g.put(OX + SPINE_X, OY + 14, "┴", BORDER)
    g.put(OX + BACK_J, ty, "┌", BORDER)
    for x in range(OX + BACK_J + 1, OX + SPINE_X):
        g.put(x, ty, "─", BORDER)
    g.put(OX + BACK_J, OY + 14, "┴", BORDER)


def site_row(site, short=None):
    got = CHROME[site]
    if got:
        return [("■ ", CYAN_DIM), (short or site, TEXT)]
    return [("□ ", EMPTY), (short or site, EMPTY)]


def one_site(site):
    """A part with one site: the piece's name and what it gives, as the meat tiles."""
    got = CHROME[site]
    if not got:
        return [[("meat", EMPTY)]]
    a, b = name2(got[0])
    return [[(a, TEXT)], [(b, TEXT)], [(got[1], LABEL)]]


# ---------------------------------------------------------------- the right: the card

def card(g, x, y, art=True):
    """The Piston arm's card, as ATTRIBUTES draws it, its drawing on top."""
    if art:
        src = Grid.from_svg(os.path.join(S, "attributes_ledger_limb_120x40.svg"), W, H)
        g.paste(src.crop(75, 11, 43, 6), x + 6, y)
        y += 7
    g.put(x, y, "PISTON ARM", CYAN, bold=True)
    g.put(x, y + 1, "L ARM · load 3 · ", LABEL)
    g.put(x + 17, y + 1, "EMP −2", MAGENTA)
    g.put(x, y + 2, "BODY +1", GREEN)
    g.put(x + 7, y + 2, " · grants ", LABEL)
    g.put(x + 17, y + 2, "Shove", CYAN)
    g.put(x, y + 4, "■ hand    ", CYAN)
    g.put(x + 10, y + 4, "Arc hand", TEXT, bold=True)
    g.put(x + 10, y + 5, "Jolt · SHK +10", CYAN)
    g.put(x, y + 6, "□ forearm ", LABEL)
    g.put(x + 10, y + 6, "empty", EMPTY)
    g.put(x + 10, y + 7, "Monowire spool, Launcher sleeve", EMPTY)
    g.put(x, y + 9, "A piston in a sleeve, bolted on at the elbow.", TEXT)
    g.put(x, y + 10, "Good for arguments that have gone physical.", TEXT)
    return y + 11


def footer(g, hint, note=True):
    g.put(2, 38, hint, LABEL)
    if note:
        g.put(W - 3 - 44, 37, "chrome goes in and out in a ripperdoc's chair", EMPTY)


# ---------------------------------------------------------------- A: sites on the parts

def concept_a():
    """The meat view's tiles, each holding the sites on that part: a part with one site
    shows its piece as the meat tiles do, a part with several lists them, ■ fitted, □ not.
    The hands show what the arm's hand socket holds. Waist and feet take no chrome: faint."""
    g = frame()
    header(g)
    lines_of_body(g)
    tile(g, *at("head"), "HEAD", [site_row("NEURAL"), site_row("EARS"), site_row("JAW")])
    tile(g, *at("face"), "FACE", one_site("EYES"))
    tile(g, *at("torso"), "TORSO", [site_row("ORGANS"), site_row("SKIN")])
    tile(g, *at("back"), "BACK", [site_row("SPINE"), site_row("BACK"),
                                  [("  └ ", BORDER), ("bay", LABEL), (" ■", CYAN_DIM)]])
    tile(g, *at("larm"), "L ARM", [[("Piston", TEXT)], [("arm", TEXT)],
                                   [("BODY +1 ", LABEL), ("■□", SOCKET)]], sel=True)
    tile(g, *at("rarm"), "R ARM", one_site("R ARM"))
    tile(g, *at("lhand"), "L HAND", [[("Arc hand", TEXT)], [("in the arm", EMPTY)]])
    tile(g, *at("rhand"), "R HAND", [[("meat", EMPTY)]])
    tile(g, *at("legs"), "LEGS", one_site("LEGS"))
    tile(g, *at("waist"), "WAIST", faint=True)
    tile(g, *at("feet"), "FEET", faint=True)
    card(g, SEP + 2, 9)
    footer(g, "←↑↓→ body, ↑↓ through a part's sites · c meat/chrome · space inspect · "
              "Q/E tabs · tab close")
    return g


# ---------------------------------------------------------------- B: the ledger beside it

def concept_b():
    """The body as a map: each part's tile shows a pip per site (■ fitted, □ not); the
    ledger you know from ATTRIBUTES takes the right, grouped by part, with the card under
    it. The cursor walks the ledger and lights the part it's on."""
    g = frame()
    header(g)
    lines_of_body(g)

    def pips(*sites):
        return [[("■" if CHROME[s] else "□", CYAN_DIM if CHROME[s] else EMPTY) for s in sites]]

    tile(g, *at("head"), "HEAD", [[("■□■", CYAN_DIM)], [("2 of 3", LABEL)]])
    tile(g, *at("face"), "FACE", [[("■", CYAN_DIM)], [("1 of 1", LABEL)]])
    tile(g, *at("torso"), "TORSO", [[("■■", CYAN_DIM)], [("2 of 2", LABEL)]])
    tile(g, *at("back"), "BACK", [[("■■", CYAN_DIM)], [("2 of 2", LABEL)]])
    tile(g, *at("larm"), "L ARM", [[("■", CYAN_DIM)], [("1 of 1", LABEL)]], sel=True)
    tile(g, *at("rarm"), "R ARM", [[("□", EMPTY)], [("0 of 1", EMPTY)]])
    tile(g, *at("lhand"), "L HAND", faint=True)
    tile(g, *at("rhand"), "R HAND", faint=True)
    tile(g, *at("legs"), "LEGS", [[("□", EMPTY)], [("0 of 1", EMPTY)]])
    tile(g, *at("waist"), "WAIST", faint=True)
    tile(g, *at("feet"), "FEET", faint=True)
    # The ledger, grouped by part.
    x, y = SEP + 2, 9
    rows = [
        ("group", "head"), ("site", "NEURAL", "Neural port"), ("site", "EARS", None),
        ("site", "JAW", "Bulldog jaw"),
        ("group", "face"), ("site", "EYES", "Target lens"),
        ("group", "torso"), ("site", "ORGANS", "Adrenal pump"), ("site", "SKIN", "Subdermal mesh"),
        ("group", "back"), ("site", "SPINE", "Redline spine"), ("site", "BACK", "Drone cradle"),
        ("sock", "└", "Scout drone"),
        ("group", "arms"), ("site", "L ARM", "Piston arm"), ("sock", "├", "Arc hand"),
        ("sock", "└", "□ forearm"), ("site", "R ARM", None),
        ("group", "legs"), ("site", "LEGS", None),
    ]
    yy = y
    for r in rows[:16]:
        if r[0] == "group":
            g.put(x + 1, yy, "── %s ──" % r[1], EMPTY)
        elif r[0] == "site":
            lit = r[1] == "L ARM"
            if lit:
                g.fill(x, yy, 27, 1, SEL_BG)
                g.put(x, yy, "▸", CYAN)
            g.put(x + 2, yy, r[1], LABEL)
            if r[2]:
                g.put(x + 10, yy, r[2], CYAN if lit else TEXT, bold=lit)
            else:
                g.put(x + 10, yy, "· meat", EMPTY)
        else:
            g.put(x + 11, yy, r[1], BORDER)
            g.put(x + 13, yy, r[2], TEXT)
        yy += 1
    g.put(x + 26, 9, "▲", EMPTY)
    g.put(x + 26, 24, "▼", EMPTY)
    for j in range(9, 25):
        g.put(x + 28, j, "│", BORDER)
    # The card beside the ledger, and its drawing under both.
    cx = x + 30
    g.put(cx, 9, "PISTON ARM", CYAN, bold=True)
    g.put(cx, 10, "load 3 · ", LABEL)
    g.put(cx + 9, 10, "EMP −2", MAGENTA)
    g.put(cx, 11, "BODY +1", GREEN)
    g.put(cx, 12, "grants ", LABEL)
    g.put(cx + 7, 12, "Shove", CYAN)
    g.put(cx, 14, "■ hand ", CYAN)
    g.put(cx + 7, 14, "Arc hand", TEXT, bold=True)
    g.put(cx, 15, "□ forearm ", LABEL)
    g.put(cx + 10, 15, "empty", EMPTY)
    g.put(cx, 17, "A piston in a sleeve,", TEXT)
    g.put(cx, 18, "bolted on at the elbow.", TEXT)
    g.put(cx, 19, "Good for arguments that", TEXT)
    g.put(cx, 20, "have gone physical.", TEXT)
    g.put(SEP, 25, "╠", BORDER)
    g.put(SEP + 1, 25, "═" * (W - 2 - SEP), BORDER)
    src = Grid.from_svg(os.path.join(S, "attributes_ledger_limb_120x40.svg"), W, H)
    g.paste(src.crop(75, 11, 43, 6), SEP + 8, 28)
    footer(g, "↑↓ ledger · ← body · c meat/chrome · space inspect · Q/E tabs · tab close")
    return g


# ---------------------------------------------------------------- C: a tile per site

CPOS = {
    "EARS": (0, 0), "NEURAL": (12, 0), "JAW": (24, 0), "EYES": (36, 0),
    "L HAND": (0, 6), "L ARM": (12, 6), "ORGANS": (24, 6), "R ARM": (36, 6), "R HAND": (48, 6),
    "BACK": (12, 13), "SPINE": (24, 13), "SKIN": (36, 13),
    "LEGS": (24, 19),
}


def cat(key, h=5):
    x, y = CPOS[key]
    return (OX + x, OY + y, 11, h)


def concept_c():
    """The chrome body drawn on its own: a tile per site where it sits, joined by the
    lines of the nerves rather than the clothes. Each tile shows its piece as the meat
    tiles do; the arms' hand sockets hang off them as the hands."""
    g = frame()
    header(g)
    for a, b in [("EARS", "NEURAL"), ("NEURAL", "JAW"), ("JAW", "EYES"),
                 ("L HAND", "L ARM"), ("L ARM", "ORGANS"), ("ORGANS", "R ARM"),
                 ("R ARM", "R HAND"), ("BACK", "SPINE"), ("SPINE", "SKIN")]:
        hline(g, cat(a), cat(b))
    vline(g, cat("NEURAL"), cat("L ARM"), OX + 17)
    g.put(OX + 17, OY + 5, "│", BORDER)
    # The spine: neural down through the organs to the spine and the legs.
    nx = OX + 29
    vline(g, cat("JAW"), cat("ORGANS"), nx)
    vline(g, cat("ORGANS"), cat("SPINE"), nx)
    vline(g, cat("SPINE"), cat("LEGS"), nx)
    for site in ["EARS", "NEURAL", "JAW", "EYES", "ORGANS", "BACK", "SPINE", "SKIN", "R ARM",
                 "LEGS"]:
        got = CHROME[site]
        tile(g, *cat(site), site, one_site(site) if got else None)
    tile(g, *cat("L ARM"), "L ARM", [[("Piston", TEXT)], [("arm", TEXT)],
                                     [("BODY +1 ", LABEL), ("■□", SOCKET)]], sel=True)
    tile(g, *cat("L HAND"), "L HAND", [[("Arc hand", TEXT)], [("", TEXT)], [("Jolt", LABEL)]])
    tile(g, *cat("R HAND"), "R HAND", faint=True)
    g.put(OX + 24 + 4, OY + 13 + 6, "", BORDER)
    card(g, SEP + 2, 9)
    footer(g, "←↑↓→ the chrome · c meat/chrome · space inspect · Q/E tabs · tab close")
    return g


CONCEPTS = [
    ("A", "A · Sites on the parts",
     "The same body as the meat view; <b>c</b> swaps what the tiles hold. A part with one "
     "site (an arm, the face, the legs) shows its piece the way the meat tiles show "
     "clothes. A part with several (the head's NEURAL, EARS, JAW; the torso; the back) "
     "lists them, ■ fitted, □ not, and the cursor steps through them. The hands show what "
     "the arm's hand socket holds. Waist and feet take no chrome and go faint. The card "
     "on the right is ATTRIBUTES' card, drawing and all.", concept_a),
    ("B", "B · The ledger beside the body",
     "The body becomes a map: each part shows a pip per site and a count. The ledger you "
     "know from ATTRIBUTES moves to the right, grouped by part now, with the card beside "
     "it and the drawing under both. The cursor walks the ledger and lights the part "
     "it's on. Closest to today, so least new to learn; the body does the least.",
     concept_b),
    ("C", "C · A tile per site",
     "The chrome drawn as its own body: a tile for each site where it sits, joined along "
     "the nerves (head across the top, the arms and organs through the middle, back, "
     "spine and skin below, legs at the bottom). Every tile shows its piece in full, as "
     "the meat tiles do. The most to look at; the meat and chrome views don't line up "
     "tile for tile.", concept_c),
]


# ---------------------------------------------------------------- round 2: C, laid out

# Sites and the sockets of what's in them, each a tile, where they sit. A socket's tile
# shows only while its host holds a piece with that socket.
C2 = {
    "EYES": (12, 0), "NEURAL": (24, 0), "EARS": (36, 0), "JAW": (48, 0),
    "LHAND": (0, 6), "L ARM": (12, 6), "ORGANS": (24, 6), "R ARM": (36, 6), "RHAND": (48, 6),
    "LFORE": (12, 12), "SPINE": (24, 12), "RFORE": (36, 12),
    "BAY": (0, 18), "BACK": (12, 18), "LEGS": (24, 18), "SKIN": (36, 18),
}


def c2(key):
    x, y = C2[key]
    return (OX + x, OY + y, 11, 5)


def chrome_body(g, sel="L ARM", sockets=True):
    trunk = OX + 29
    for a, b in [("EYES", "NEURAL"), ("NEURAL", "EARS"), ("EARS", "JAW"),
                 ("L ARM", "ORGANS"), ("ORGANS", "R ARM"), ("BAY", "BACK")]:
        if sockets or "BAY" not in (a, b):
            hline(g, c2(a), c2(b))
    if sockets:
        hline(g, c2("LHAND"), c2("L ARM"))
        vline(g, c2("L ARM"), c2("LFORE"), OX + 17)
    vline(g, c2("NEURAL"), c2("ORGANS"), trunk)
    vline(g, c2("ORGANS"), c2("SPINE"), trunk)
    # Under the spine the trunk branches: the back left, the skin right, the legs down.
    g.put(trunk, OY + 16, "┬", BORDER)
    g.put(trunk, OY + 17, "┼", BORDER)
    g.put(trunk, OY + 18, "┴", BORDER)
    g.put(OX + 17, OY + 17, "┌", BORDER)
    g.put(OX + 41, OY + 17, "┐", BORDER)
    for x in range(OX + 18, OX + 41):
        if x != trunk:
            g.put(x, OY + 17, "─", BORDER)
    g.put(OX + 17, OY + 18, "┴", BORDER)
    g.put(OX + 41, OY + 18, "┴", BORDER)

    def piece(site, extra=None):
        got = CHROME[site]
        if not got:
            return None
        a, b = name2(got[0])
        return [[(a, TEXT)], [(b, TEXT)], extra or [(got[1], LABEL)]]

    for site in ["EYES", "NEURAL", "EARS", "JAW", "ORGANS", "R ARM", "SPINE", "LEGS", "SKIN"]:
        tile(g, *c2(site), site, piece(site), sel=sel == site)
    tile(g, *c2("BACK"), "BACK", piece("BACK", [("1 bay ", LABEL), ("■", SOCKET)]),
         sel=sel == "BACK")
    tile(g, *c2("L ARM"), "L ARM", piece("L ARM", [("BODY +1 ", LABEL), ("■□", SOCKET)]),
         sel=sel == "L ARM")
    if sockets:
        tile(g, *c2("LHAND"), "HAND", [[("Arc hand", TEXT)], [("", TEXT)], [("Jolt", LABEL)]],
             sel=sel == "LHAND")
        tile(g, *c2("LFORE"), "FOREARM", None, sel=sel == "LFORE")
        tile(g, *c2("BAY"), "BAY", [[("Scout", TEXT)], [("drone", TEXT)], [("docked", LABEL)]],
             sel=sel == "BAY")


def picker(g, x, y):
    """In the chair, space on the empty forearm: what fits from the bag, as ATTRIBUTES'
    FITS card does today."""
    g.put(x, y, "L FOREARM", CYAN, bold=True)
    g.put(x, y + 1, "in the Piston arm · empty", LABEL)
    g.put(x, y + 3, "FITS", CYAN, bold=True)
    g.put(x + 5, y + 3, "─" * 30, BORDER)
    g.put(x + 36, y + 3, "from your bag", EMPTY)
    rows = [("Monowire spool", "L2", "¤ 400"), ("Launcher sleeve", "L3", "¤ 650")]
    for i, (name, load, fee) in enumerate(rows):
        yy = y + 4 + i
        if i == 0:
            g.fill(x, yy, 54, 1, SEL_BG)
            g.put(x, yy, "▸", CYAN)
        g.put(x + 2, yy, name, CYAN if i == 0 else TEXT, bold=i == 0)
        g.put(x + 40, yy, load, LABEL)
        g.put(x + 46, yy, fee, LABEL)
    g.put(x, y + 7, "load 19 → 21 of 24 · Doc Ilse fits it for ¤ 400", LABEL)
    g.put(x, y + 9, "A spool of monowire under the skin of the forearm,", TEXT)
    g.put(x, y + 10, "paid out through the knuckles.", TEXT)


def round2_view():
    g = frame()
    header(g)
    chrome_body(g)
    card(g, SEP + 2, 9)
    footer(g, "←↑↓→ chrome · c meat/chrome · Q/E tabs · tab close")
    return g


def round2_chair():
    g = frame()
    header(g)
    chrome_body(g, sel="LFORE")
    picker(g, SEP + 2, 9)
    g.put(2, 38, "↑↓ pick · space fit · esc back · c meat/chrome · Q/E tabs · esc chair",
          LABEL)
    g.put(W - 3 - 30, 37, "in Doc Ilse's chair · ¤ 2300", CYAN_DIM)
    return g


def round2_meat_arms():
    """Plain meat arms, no cradle: no socket tiles, the trunk and sites only."""
    for site in ("L ARM", "BACK"):
        CHROME[site] = None
    g = frame()
    header(g)
    chrome_body(g, sel="NEURAL", sockets=False)
    card(g, SEP + 2, 9, art=False)
    g.fill(SEP + 2, 9, 54, 12, BG)
    g.put(SEP + 2, 9, "NEURAL PORT", CYAN, bold=True)
    g.put(SEP + 2, 10, "NEURAL · load 2 · ", LABEL)
    g.put(SEP + 20, 10, "EMP −2", MAGENTA)
    g.put(SEP + 2, 11, "NET +1 · grants Jack in", LABEL)
    g.put(SEP + 2, 13, "A jack behind the ear. The Net comes in through it.", TEXT)
    footer(g, "←↑↓→ chrome · c meat/chrome · Q/E tabs · tab close")
    CHROME["L ARM"] = ("Piston arm", "BODY +1")
    CHROME["BACK"] = ("Drone cradle", "1 bay")
    return g


ROUND2 = [
    ("C1", "On CHROME, out of the chair",
     "A tile per site, the head across the top, the arms and the organs through the "
     "middle, the spine down the centre and branching under it to the back, the legs and "
     "the skin. A socket gets a tile of its own while what holds it is in: the Piston "
     "arm's HAND and FOREARM, the cradle's BAY. The card is ATTRIBUTES' card. The LOAD "
     "gauge sits on the header row.", round2_view),
    ("C2", "In the chair, space on the empty forearm",
     "The card turns into FITS, what in your bag fits there, with the load after and the "
     "doc's fee, as ATTRIBUTES does it today. space fits it, x pulls what's in a tile.",
     round2_chair),
    ("C3", "Meat arms, no cradle",
     "With nothing in the arms and no cradle, the socket tiles aren't there: the chrome "
     "body grows as chrome adds sockets (and, later, parts).", round2_meat_arms),
]

ROUNDS = {"1": ("#103 · chrome on GEAR &amp; INVENTORY, three ways", CONCEPTS),
          "2": ("#103 · C, laid out", ROUND2)}

CSS = """
:root{--bg:#0a0d13;--fg:#cdd4de;--muted:#8a95a5;--rule:#1d2a38;--head:#ffffff;
 --cyan:#00e5ff;--magenta:#ff00c8;color-scheme:dark}
""" + term.CSS + """
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding:24px 16px 80px}
main{max-width:1600px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px;color:var(--head);text-wrap:balance}
h2{font-size:18px;margin:0 0 6px;color:var(--cyan)}
p{max-width:900px}
.sub{color:var(--muted);margin:0 0 18px}
.opt{border:1px solid var(--rule);border-radius:8px;padding:16px;margin:22px 0;cursor:pointer}
.opt.selected{border-color:var(--magenta);box-shadow:0 0 0 1px var(--magenta)}
figure{margin:8px 0;overflow-x:auto;max-width:100%;min-width:0}
figcaption{color:var(--muted);font-size:13px;margin-top:6px}
"""


def main():
    heading, concepts = ROUNDS[ROUND]
    out = ["<h1>%s</h1>" % heading,
           '<p class="sub">Drawn over the real 120×40 GEAR &amp; INVENTORY frame, with the '
           "chrome from the ATTRIBUTES shots. <b>c</b> switches MEAT and CHROME (the key is "
           "a placeholder). Outside a ripperdoc's chair the view only looks and inspects; "
           "in the chair, space on a site opens what fits from your bag, as ATTRIBUTES does "
           "today. Click one to mark it.</p>"]
    for key, title, blurb, draw in concepts:
        out.append('<section class="opt" onclick="this.classList.toggle(\'selected\')">')
        out.append("<h2>%s</h2><p>%s</p>" % (title, blurb))
        out.append("<figure>%s<figcaption>GEAR &amp; INVENTORY, 120×40, on CHROME</figcaption>"
                   "</figure></section>" % term.render(draw(), 11.0))
    html = ("<!DOCTYPE html><html><head><meta charset='utf-8'>"
            "<meta name='viewport' content='width=device-width,initial-scale=1'>"
            "<title>Chrome view</title><style>%s</style></head><body><main>%s</main>"
            "</body></html>" % (CSS, "".join(out)))
    with open(OUT, "w") as f:
        f.write(html)


if __name__ == "__main__":
    main()
