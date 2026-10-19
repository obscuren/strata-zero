"""The deck screen's chrome from the game's own shots, and the pieces every mockup shares."""

from cells import Grid, pixels, shrink, fold
import os
import textwrap

# The game's shots, rendered by `SHOT_DIR=... cargo test -p strata-zero shots:: -- --ignored`.
SHOTS = os.environ.get("SHOT_DIR", ".").rstrip("/") + "/"

CYAN = "#00e5ff"
LIT = "#00c8e0"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#5c6673"
TEXT = "#cdd4de"
PANEL = "#04060a"
RULE = "#283c4b"
BAR = "#003c46"
DONE = "#5ac88c"
SLOT_USED = ("#3f7c87", "#1a9aaa")
SLOT_FREE = ("#4f5864", "#2a7480")
DOT = (DIM, "#00a5b8")
SOCK_FULL = (CYAN, CYAN)
SOCK_EMPTY = (DIM, "#00a5b8")

TABS = ["SKILLS", "STATS", "GEAR & INVENTORY", "WORKBENCH", "JOBS", "JOURNAL", "MAP", "TALENTS"]


def base(w=100, h=30, active=2):
    """The deck screen with an empty body: real frame, header and ghost from the shots."""
    gear = Grid.from_svg(SHOTS + "gear_marks_100x30.svg", w, h)
    inv = Grid.from_svg(SHOTS + "inventory_marks_100x30.svg", w, h)
    g = Grid.from_svg(SHOTS + "gear_marks_100x30.svg", w, h)
    # Tab row: the ghost both shots share, the deck's labels dropped.
    for y in (3, 4):
        for x in range(1, w - 1):
            a, b = gear.c[y][x], inv.c[y][x]
            g.c[y][x] = [" ", TEXT, a[2] if a[2] != BAR else b[2], False]
    # Body: flat glass in its commonest colour, so the map's ghost draws no edges.
    from collections import Counter
    tint = Counter(gear.c[y][x][2] for y in range(5, 27) for x in range(1, w - 1)).most_common(1)[0][0]
    for y in range(5, 28):
        for x in range(1, w - 1):
            g.c[y][x] = [" ", TEXT, tint, False]
    # The strip row and hints are redrawn by the caller.
    for x in range(1, w - 1):
        g.c[28][x] = [" ", DIM, g.c[28][x][2], False]
    tabbar(g, active)
    return g


def tabbar(g, active, y=3):
    x = 2
    starts = []
    for i, t in enumerate(TABS):
        label = ("▌%s▐" if i == active else " %s ") % t
        starts.append(x)
        g.put(x, y, label, LIT if i == active else DIM, bold=i == active)
        x += len(label) + 2
    for cx in range(1, g.w - 1):
        g.c[y + 1][cx][0] = "─"
        g.c[y + 1][cx][1] = RULE
    for cx in range(starts[active], starts[active] + len(TABS[active]) + 2):
        g.c[y + 1][cx][0] = "━"
        g.c[y + 1][cx][1] = MAGENTA


def hints(g, text, y=28):
    g.put(2, y, text, DIM)


def strip(g, text, y=27, color=CYAN):
    g.put(2, y, text, color)


def vrule(g, x, y0, y1, ch="║"):
    for y in range(y0, y1):
        g.put(x, y, ch, RULE)


def hrule(g, x0, x1, y, ch="═"):
    for x in range(x0, x1):
        g.put(x, y, ch, RULE)


def bar(g, x0, x1, y):
    g.bg(x0, y, x1 - x0, BAR)


def marks_spans(slots_used, slots_free, sockets, selected):
    """[(text, colour)] after a name: program slots, a dot, sockets."""
    k = 1 if selected else 0
    out = []
    if slots_used:
        out.append(("■" * slots_used, SLOT_USED[k]))
    if slots_free:
        out.append(("□" * slots_free, SLOT_FREE[k]))
    if out and sockets:
        out.append((" · ", DOT[k]))
    for s in sockets:
        out.append(("■", SOCK_FULL[k]) if s else ("□", SOCK_EMPTY[k]))
    return out


def put_spans(g, x, y, spans, right, bg=None, bold=False):
    for text, col in spans:
        g.put(x, y, text, col, bg=bg, bold=bold, right=right)
        x += len(text)
    return x


def cut(name, room):
    if len(name) <= room:
        return name
    return name[: max(room - 1, 0)].rstrip() + "…"


def row(g, x0, x1, y, lead, name, marks=(), selected=False, focused=True, tone=TEXT, tail=None):
    """A list row: cursor, lead, name cut to leave its marks room, marks, a right tail."""
    lit = selected and focused
    if lit:
        bar(g, x0, x1, y)
    cur = "▸ " if selected else "  "
    ccol = LIT if lit else (DIM if selected else TEXT)
    g.put(x0 + 1, y, cur, ccol, bold=lit)
    marks_w = sum(len(t) for t, _ in marks)
    tail_w = len(tail[0]) + 3 if tail else 0
    room = (x1 - x0 - 3) - len(lead) - (marks_w + 1 if marks else 0) - tail_w
    head = lead + cut(name, room)
    col = LIT if lit else tone
    x = put_spans(g, x0 + 3, y, [(head, col)], x1, bold=lit)
    if marks:
        put_spans(g, x + 1, y, [(t, c) for t, c in marks], x1, bold=lit)
    if tail:
        g.put(x1 - len(tail[0]) - 1, y, tail[0], tail[1], bold=lit)


def card(g, x, y, w, h, lines):
    """The item card: [(text, colour, bold)]; long text wraps."""
    cy = y
    for text, col, bold in lines:
        for part in (textwrap.wrap(text, w) or [""]):
            if cy >= y + h:
                return
            g.put(x, cy, part, col, bold=bold)
            cy += 1


# ---- drawings, shrunk from the blueprint shots

def crop_art(name, box):
    g = Grid.from_svg(SHOTS + name + ".svg", 100, 30)
    x0, x1, y0, y1 = box
    return pixels(g, x0, y0, x1 - x0 + 1, y1 - y0 + 1, PANEL)


BOXES = {
    "ratchet_fitted": ("blueprint_ratchet_fitted_100x30", (8, 93, 6, 20)),
    "ratchet": ("blueprint_ratchet_100x30", (8, 75, 9, 20)),
    "deck": ("blueprint_deck_100x30", (4, 90, 3, 23)),
    "hushcan": ("hardware_hushcan_100x30", (14, 85, 7, 20)),
    "redeye": ("hardware_redeye_100x30", (25, 73, 6, 22)),
    "coldfin_2": ("hardware_coldfin_2_100x30", (24, 73, 5, 22)),
    "sidecar": ("hardware_sidecar_100x30", (24, 75, 5, 22)),
}


def preview(key, mode="avg"):
    shot, box = BOXES[key]
    p = crop_art(shot, box)
    if len(p) % 4:
        p += [[None] * len(p[0]) for _ in range(4 - len(p) % 4)]
    s = shrink(p, 2, 2, mode)
    return fold(s, PANEL)


def full(key):
    shot, box = BOXES[key]
    g = Grid.from_svg(SHOTS + shot + ".svg", 100, 30)
    x0, x1, y0, y1 = box
    return g.crop(x0, y0, x1 - x0 + 1, y1 - y0 + 1)


def viewport(g, x0, y0, w, h, art=None, caption=None, missing=None):
    """A solid dark window with corner ticks; the drawing centred in it."""
    g.fill(x0, y0, w, h, PANEL)
    for (cx, cy, ch) in [(x0, y0, "┌"), (x0 + w - 1, y0, "┐"), (x0, y0 + h - 1, "└"), (x0 + w - 1, y0 + h - 1, "┘")]:
        g.put(cx, cy, ch, RULE)
    if art is not None:
        ax = x0 + (w - art.w) // 2
        ay = y0 + (h - art.h) // 2
        g.paste(art, ax, ay, skip_bg=PANEL)
    if missing:
        icon, label = missing
        mw, mh = 21, 5
        mx, my = x0 + (w - mw) // 2, y0 + (h - mh) // 2
        for i in range(mw):
            g.put(mx + i, my, "╌", FAINT)
            g.put(mx + i, my + mh - 1, "╌", FAINT)
        for j in range(mh):
            g.put(mx, my + j, "╎", FAINT)
            g.put(mx + mw - 1, my + j, "╎", FAINT)
        for (cx, cy, ch) in [(mx, my, "┌"), (mx + mw - 1, my, "┐"), (mx, my + mh - 1, "└"), (mx + mw - 1, my + mh - 1, "┘")]:
            g.put(cx, cy, ch, FAINT)
        g.put(mx + mw // 2, my + 1, icon, DIM, bold=True)
        g.put(mx + (mw - len(label)) // 2, my + 3, label, FAINT)
    if caption:
        g.put(x0 + w - len(caption) - 2, y0 + h - 1, caption, FAINT)
