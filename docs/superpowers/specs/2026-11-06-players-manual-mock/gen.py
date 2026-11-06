"""The MANUAL as the booklet in the box: three looks (A paper booklet, B open spread,
C ring binder), each as its cover, its contents and a page, at the minimum 100x30."""

import random
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from cells import Grid  # noqa: E402
import term  # noqa: E402

W, H = 100, 30

# the game's palette (ui::chrome)
CYAN, MAGENTA, TEXT, DIM, FAINT, RULE = "#00e5ff", "#ff00c8", "#cdd4de", "#a0aab9", "#78828f", "#283c4b"
PANEL = "#04060a"

# ---- the logo, as ui::logo draws it

LOGO = {
    'S': ["▄▀▀▀", "█   ", "▀▀▀▄", "   █", "▀▀▀ "],
    'T': ["▀▀█▀▀", "  █  ", "  █  ", "  █  ", "  ▀  "],
    'R': ["█▀▀▄", "█  █", "█▀▀▄", "█  █", "▀  ▀"],
    'A': ["▄▀▀▄", "█  █", "█▀▀█", "█  █", "▀  ▀"],
    'Z': ["▀▀▀▀█", "   █ ", "  █  ", " █   ", "█▀▀▀▀"],
    'E': ["█▀▀▀", "█   ", "█▀▀ ", "█   ", "▀▀▀▀"],
    'O': ["▄▀▀▄", "█  █", "█  █", "█  █", "▀▀▀▀"],
    ' ': ["  ", "  ", "  ", "  ", "  "],
}


def logo_rows(word):
    rows = ["", "", "", "", ""]
    for ch in word:
        g = LOGO[ch]
        w = max(len(r) for r in g)
        for i in range(5):
            rows[i] += g[i].ljust(w) + " "
    return [r.rstrip() for r in rows]


def logo_width(word):
    return max(len(r) for r in logo_rows(word))


def put_logo(g, x, y, word, fg, bg, shadow=None):
    rows = logo_rows(word)
    if shadow:
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch != " ":
                    g.put(x + i + 1, y + j, ch, shadow, bg)
    for j, row in enumerate(rows):
        for i, ch in enumerate(row):
            if ch != " ":
                g.put(x + i, y + j, ch, fg, bg)


# ---- half-block pixels

class Px:
    def __init__(self, w, h, fill):
        self.w, self.h = w, h
        self.p = [[fill] * w for _ in range(h)]

    def set(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.p[y][x] = c

    def rect(self, x, y, w, h, c):
        for j in range(y, y + h):
            for i in range(x, x + w):
                self.set(i, j, c)

    def blit(self, g, gx, gy):
        for j in range(self.h // 2):
            for i in range(self.w):
                t, b = self.p[2 * j][i], self.p[2 * j + 1][i]
                if t == b:
                    g.c[gy + j][gx + i] = [" ", t, t, False]
                else:
                    g.c[gy + j][gx + i] = ["▀", t, b, False]


def lerp(a, b, t):
    a = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(int(a[k] + (b[k] - a[k]) * t) for k in range(3))


def city_art(w, h, seed=7, logo=None):
    """A cross-section of the city, `w` cells by `h` rows: the Net's grid in the sky,
    towers with their windows lit, the street's neon, the undercity under it. With
    `logo`, the words over the sky in neon."""
    rng = random.Random(seed)
    px = Px(w, 2 * h, "#000000")
    H2 = 2 * h
    street = int(H2 * 0.74)
    for y in range(H2):
        if y < street:
            c = lerp("#140a2a", "#2a0f3a", y / street)
        else:
            c = lerp("#0b0710", "#020204", (y - street) / (H2 - street))
        for x in range(w):
            px.set(x, y, c)
    # the Net: a cyan grid running back to a horizon line high in the sky
    hz = max(3, int(H2 * 0.16))
    for x in range(w):
        px.set(x, hz, "#0a6d80")
    cx = w // 2
    for k in range(-10, 11):
        for y in range(0, hz):
            t = (hz - y) / hz
            x = int(cx + k * 3 + k * 9 * t)
            if 0 <= x < w and y % 1 == 0:
                px.set(x, y, "#0b5a6a" if y > 1 else "#11899f")
    for y in (1, 4, 8):
        if y < hz:
            for x in range(w):
                if px.p[y][x] != "#0b5a6a":
                    px.set(x, y, "#0a3d4a")
    # towers, back row dim, front row darker with windows
    for row, shade, lit in ((0, "#22183a", 0.10), (1, "#120d1e", 0.22)):
        x = rng.randint(-3, 1)
        while x < w:
            tw = rng.randint(4, 9)
            top = rng.randint(hz + 2, max(hz + 3, street - 5)) + (2 if row else 0)
            px.rect(x, top, tw, street - top, shade)
            if rng.random() < 0.45:
                px.rect(x + tw // 2, top - 3, 1, 3, shade)
            for yy in range(top + 2, street - 1, 2):
                for xx in range(x + 1, x + tw - 1, 2):
                    if rng.random() < lit:
                        px.set(xx, yy, rng.choice(["#ffd166", "#00e5ff", "#ff4fd8", "#ffe9a8"]))
            if row and rng.random() < 0.4 and street - 4 >= top + 2:
                sy = rng.randint(top + 2, street - 4)
                px.rect(x + 1, sy, max(2, tw - 2), 1, rng.choice(["#ff00c8", "#00e5ff"]))
            x += tw + rng.randint(0, 2)
    # the street: a neon line and its glow
    px.rect(0, street, w, 1, "#ff00c8")
    px.rect(0, street + 1, w, 1, "#5a0b48")
    # the undercity: pipes and a few lamps
    for y in (street + 4, street + 7):
        for x in range(w):
            if (x // 7 + y) % 3:
                px.set(x, y, "#1d2430")
    for x in range(3, w, 11):
        px.set(x, street + 5, "#ffb347")
    return px


def backdrop():
    """The title washed behind the booklet: faint rain over the dark."""
    g = Grid(W, H, PANEL)
    rng = random.Random(3)
    for y in range(H):
        for x in range(W):
            r = rng.random()
            if r < 0.03:
                g.put(x, y, "╎", "#1b2733")
            elif r < 0.04:
                g.put(x, y, "·", "#1f2a36")
    return g


# ---- the pages' words

CHAPTERS = [
    ("Getting started", 3), ("Your runner", 5), ("The screen", 8), ("Controls", 11),
    ("The deck", 14), ("The city and the world", 19), ("Fights", 23), ("The Net", 27),
    ("Chrome, clinics and drones", 33), ("Fixers and jobs", 37), ("Time and rest", 41),
    ("Saving and dying", 44),
]

SHORT = ["Start", "Runner", "Screen", "Controls", "Deck", "World", "Fights", "The Net",
         "Chrome", "Jobs", "Time", "Saving"]

PAGE = [
    ("h2", "On foot"),
    ("p", "You move one step at a time, and the city keeps its own time around you: "
          "people walk their routes, lights cycle, and the clock runs while you stand still."),
    ("table", [("hjkl yubn ↑↓←→", "move, eight ways"),
               ("< >", "stairs, a lift, a ladder or a manhole"),
               ("space", "the thing in reach: look, talk, take, jack in"),
               ("x", "look around with a cursor"),
               ("tab", "the deck"),
               ("?", "every key on one card")]),
    ("tip", "Not sure what something is? Stand next to it and press space."),
]


def wrap(text, width):
    words, rows, row = text.split(" "), [], ""
    for w in words:
        if row and len(row) + 1 + len(w) > width:
            rows.append(row)
            row = w
        else:
            row = (row + " " + w) if row else w
    rows.append(row)
    return rows


def body(g, x, y, w, ink, dim, key, keybg, accent, bg, rule):
    """PAGE's blocks from (x, y), `w` wide; returns the row after."""
    for kind, v in PAGE:
        if kind == "h2":
            g.put(x, y, v.upper(), accent, bg, bold=True)
            y += 1
        elif kind == "p":
            for row in wrap(v, w):
                g.put(x, y, row, ink, bg)
                y += 1
            y += 1
        elif kind == "table":
            g.put(x, y, "KEY", dim, bg, bold=True)
            g.put(x + 17, y, "WHAT IT DOES", dim, bg, bold=True)
            y += 1
            g.put(x, y, "─" * (w - 1), rule, bg)
            y += 1
            for k, what in v:
                g.put(x, y, k, key, keybg)
                rows = wrap(what, w - 17)
                for i, row in enumerate(rows):
                    g.put(x + 17, y + i, row, ink, bg)
                y += len(rows)
            y += 1
        elif kind == "tip":
            rows = wrap(v, w - 3)
            for i, row in enumerate(rows):
                g.put(x, y + i, "▌", accent, bg)
                g.put(x + 2, y + i, row, dim, bg)
            y += len(rows) + 1
    return y


def contents(g, x, y, w, ink, dim, num, sel_fg, sel_bg, bg, at=3, leader="."):
    for i, (name, page) in enumerate(CHAPTERS):
        picked = i == at
        fg = sel_fg if picked else ink
        b = sel_bg if picked else bg
        n = "%2d" % (i + 1)
        pg = str(page)
        dots = leader * (w - 4 - len(name) - len(pg) - 3)
        line = " %s  %s %s %s" % (n, name, dots, pg)
        g.put(x, y + i, line.ljust(w), fg, b, bold=picked)
        if not picked:
            g.put(x + 1, y + i, n, num, b)
            g.put(x + 5 + len(name), y + i, " " + dots + " ", dim, b)
        if picked:
            g.put(x, y + i, "▶", sel_fg, b, bold=True)


def hint(g, text, fg=DIM):
    g.put((W - len(text)) // 2, H - 1, text, fg, PANEL)


# ---- A: the paper booklet

PAPER, INK, INK_DIM, RED, KEYCAP = "#e7dfca", "#1d1b17", "#6e675b", "#c0143c", "#d3c8ae"
PW, PH = 60, 28


def paper_page(g):
    x0, y0 = (W - PW) // 2, 0
    g.fill(x0, y0, PW, PH, PAPER)
    # the page's shadow on the desk
    for j in range(1, PH):
        g.c[y0 + j][x0 + PW] = [" ", "#000000", "#010203", False]
    return x0, y0


def a_cover():
    g = backdrop()
    x0, y0 = paper_page(g)
    g.fill(x0, y0, PW, 1, RED)
    g.put(x0 + 2, y0, "PLAYER'S MANUAL", PAPER, RED, bold=True)
    g.put(x0 + PW - 19, y0, "FOR YOUR TERMINAL", PAPER, RED)
    put_logo(g, x0 + 4, y0 + 2, "STRATA", INK, PAPER, shadow=RED)
    put_logo(g, x0 + 4 + logo_width("STRATA") - logo_width("ZERO"), y0 + 8, "ZERO", INK, PAPER, shadow=RED)
    city_art(PW - 8, 10).blit(g, x0 + 4, y0 + 14)
    g.put(x0 + 4, y0 + 25, "h i g h   t e c h   ·   l o w   l i f e", INK, PAPER, bold=True)
    g.put(x0 + 4, y0 + 26, "A roguelike five strata deep.", INK_DIM, PAPER)
    g.put(x0 + PW - 10, y0 + 26, "v0.2.0", RED, PAPER, bold=True)
    hint(g, "space open · esc close")
    return g


def a_contents():
    g = backdrop()
    x0, y0 = paper_page(g)
    g.put(x0 + 4, y0 + 2, "CONTENTS", INK, PAPER, bold=True)
    g.put(x0 + 4, y0 + 3, "━" * 8, RED, PAPER)
    contents(g, x0 + 3, y0 + 5, PW - 7, INK, "#a59d8c", RED, PAPER, INK, PAPER)
    g.put(x0 + 4, y0 + 19, "Pages turn with ← →. The contents are always one", INK_DIM, PAPER)
    g.put(x0 + 4, y0 + 20, "key away: c.", INK_DIM, PAPER)
    g.put(x0 + (PW - 5) // 2, y0 + PH - 2, "— 1 —", INK_DIM, PAPER)
    hint(g, "↑↓ choose · space turn to · esc close")
    return g


def a_page():
    g = backdrop()
    x0, y0 = paper_page(g)
    g.put(x0 + 4, y0 + 1, "4 · CONTROLS", RED, PAPER, bold=True)
    g.put(x0 + PW - 15, y0 + 1, "STRATA ZERO", INK_DIM, PAPER)
    g.put(x0 + 4, y0 + 2, "─" * (PW - 8), INK_DIM, PAPER)
    body(g, x0 + 4, y0 + 4, PW - 8, INK, INK_DIM, INK, KEYCAP, RED, PAPER, INK_DIM)
    g.put(x0 + (PW - 6) // 2, y0 + PH - 2, "— 11 —", INK_DIM, PAPER)
    hint(g, "← → turn the page · c contents · esc close")
    return g


# ---- B: the open spread, in the game's own colours

SHEET, SPINE, STAPLE = "#0a0e15", "#151c27", "#8c96a4"
SW = 46


def spread(g, left=True):
    """Two pages and the spine between them; the left one left out for a closed cover."""
    lx, rx, y0 = 3, 3 + SW + 2, 0
    if left:
        g.fill(lx, y0, SW, PH, SHEET)
    g.fill(rx, y0, SW, PH, SHEET)
    sx = lx + SW
    if left:
        for j in range(PH):
            g.c[y0 + j][sx] = ["▐", SPINE, SHEET, False]
            g.c[y0 + j][sx + 1] = ["▌", SPINE, SHEET, False]
        for j in (6, 21):
            g.put(sx, y0 + j, "▐▌", STAPLE, SPINE)
    else:
        for j in range(PH):
            g.c[y0 + j][rx - 1] = ["▐", "#1f2836", PANEL, False]
    return lx, rx, y0


def frame_page(g, x, y, w, h, colour):
    g.put(x, y, "┍" + "━" * (w - 2) + "┑", colour, SHEET)
    for j in range(1, h - 1):
        g.put(x, y + j, "│", colour, SHEET)
        g.put(x + w - 1, y + j, "│", colour, SHEET)
    g.put(x, y + h - 1, "┕" + "━" * (w - 2) + "┙", colour, SHEET)


def b_cover():
    g = backdrop()
    rx, y0 = (W - SW) // 2, 0
    g.fill(rx, y0, SW, PH, SHEET)
    for j in range(PH):
        g.c[y0 + j][rx - 1] = ["▐", "#1f2836", PANEL, False]
    frame_page(g, rx, y0, SW, PH, CYAN)
    put_logo(g, rx + 5, y0 + 2, "STRATA", MAGENTA, SHEET)
    put_logo(g, rx + 5 + logo_width("STRATA") - logo_width("ZERO"), y0 + 8, "ZERO", MAGENTA, SHEET)
    city_art(SW - 4, 9, seed=11).blit(g, rx + 2, y0 + 14)
    g.put(rx + 3, y0 + 24, "PLAYER'S MANUAL", CYAN, SHEET, bold=True)
    g.put(rx + 3, y0 + 25, "high tech · low life", DIM, SHEET)
    g.put(rx + SW - 9, y0 + 25, "v0.2.0", FAINT, SHEET)
    hint(g, "space open · esc close")
    return g


def b_contents():
    g = backdrop()
    lx, rx, y0 = spread(g)
    g.put(lx + 3, y0 + 2, "CONTENTS", CYAN, SHEET, bold=True)
    g.put(lx + 3, y0 + 3, "━" * 8, MAGENTA, SHEET)
    contents(g, lx + 2, y0 + 5, SW - 4, TEXT, "#3a4656", CYAN, "#04060a", CYAN, SHEET)
    g.put(lx + 2, y0 + PH - 2, "1", FAINT, SHEET)
    # the right page: the first chapter's opening
    g.put(rx + 3, y0 + 2, "1", MAGENTA, SHEET, bold=True)
    g.put(rx + 5, y0 + 2, "GETTING STARTED", CYAN, SHEET, bold=True)
    for i, row in enumerate(wrap("Strata Zero runs in a terminal or in its own window. "
                                 "Both play the same game with the same keys; the window "
                                 "adds screen effects and keeps a few keys for itself.", SW - 6)):
        g.put(rx + 3, y0 + 4 + i, row, TEXT, SHEET)
    g.put(rx + SW - 3, y0 + PH - 2, "2", FAINT, SHEET)
    hint(g, "↑↓ choose · space turn to · ← → turn the page · esc close")
    return g


def b_page():
    g = backdrop()
    lx, rx, y0 = spread(g)
    for (x, n) in ((lx, 10), (rx, 11)):
        g.put(x + 2, y0 + 1, "CONTROLS" if n == 11 else "THE SCREEN", FAINT, SHEET)
        g.put(x + 2, y0 + 2, "─" * (SW - 4), RULE, SHEET)
        g.put(x + 2 if n == 10 else x + SW - 4, y0 + PH - 2, str(n), FAINT, SHEET)
    left = ["Down the right of the map run the panes:",
            "what's here, your jobs, the map, and the",
            "log. F1 to F4 open and close each one,",
            "and v pulls the one you want over the map",
            "as a popup.", "",
            "Along the bottom sit your health, your",
            "focus and the clock. The clock runs while",
            "you stand still; the city doesn't wait."]
    for i, row in enumerate(left):
        g.put(lx + 3, y0 + 4 + i, row, TEXT, SHEET)
    body(g, rx + 3, y0 + 4, SW - 6, TEXT, FAINT, CYAN, SHEET, CYAN, SHEET, RULE)
    hint(g, "← → turn the page · c contents · esc close")
    return g


# ---- C: the ring binder with its tabs

BINDER, RING, TABS = "#0d1219", "#5c6673", ["#ff00c8", "#00e5ff", "#ffd166", "#7cff6b", "#ff8a3d", "#b18cff"]
CW = 72


def binder(g, at):
    x0, y0 = 4, 0
    g.fill(x0, y0, CW, PH, BINDER)
    # the rings down the left edge
    for j in range(1, PH - 1, 3):
        g.put(x0 - 2, y0 + j, "━━", RING, PANEL)
        g.put(x0, y0 + j, "○", RING, BINDER)
    # one tab per chapter down the right edge, the open one pulled out
    tx = x0 + CW
    for i, (name, _) in enumerate(CHAPTERS):
        c = TABS[i % len(TABS)]
        label = "%2d %s" % (i + 1, SHORT[i])
        y = y0 + 1 + i * 2
        if i == at:
            g.put(tx, y, " " + label.ljust(12), "#04060a", c, bold=True)
        else:
            g.put(tx, y, " " + label.ljust(11), c, "#141b25")
    return x0, y0


def c_cover():
    g = backdrop()
    x0, y0 = binder(g, -1)
    put_logo(g, x0 + (CW - logo_width("STRATA ZERO")) // 2, y0 + 2, "STRATA ZERO", MAGENTA, BINDER)
    city_art(CW - 6, 12, seed=5).blit(g, x0 + 3, y0 + 9)
    g.put(x0 + 4, y0 + 22, "OPERATOR'S MANUAL", CYAN, BINDER, bold=True)
    g.put(x0 + 4, y0 + 23, "for the terminal and the window", DIM, BINDER)
    g.put(x0 + CW - 12, y0 + 23, "rev 0.2.0", FAINT, BINDER)
    g.put(x0 + 4, y0 + 25, "▲ property of the runner who found it", MAGENTA, BINDER)
    hint(g, "space open · ↑↓ the tabs · esc close")
    return g


def c_contents():
    g = backdrop()
    x0, y0 = binder(g, 3)
    g.put(x0 + 4, y0 + 2, "CONTENTS", CYAN, BINDER, bold=True)
    contents(g, x0 + 3, y0 + 4, CW - 8, TEXT, "#3a4656", CYAN, "#04060a", CYAN, BINDER)
    g.put(x0 + 4, y0 + 18, "Every chapter has a tab down the right edge: ↑↓ moves", FAINT, BINDER)
    g.put(x0 + 4, y0 + 19, "between them from any page, space opens one.", FAINT, BINDER)
    hint(g, "↑↓ the tabs · space open · ← → turn the page · esc close")
    return g


def c_page():
    g = backdrop()
    x0, y0 = binder(g, 3)
    g.put(x0 + 4, y0 + 1, "4", TABS[3], BINDER, bold=True)
    g.put(x0 + 6, y0 + 1, "CONTROLS", CYAN, BINDER, bold=True)
    g.put(x0 + CW - 10, y0 + 1, "page 11", FAINT, BINDER)
    g.put(x0 + 4, y0 + 2, "─" * (CW - 8), RULE, BINDER)
    body(g, x0 + 4, y0 + 4, CW - 8, TEXT, FAINT, CYAN, BINDER, CYAN, BINDER, RULE)
    hint(g, "← → turn the page · ↑↓ the tabs · esc close")
    return g


LOOKS = [
    ("A", "The paper booklet",
     "One page at a time on cream paper in black and red ink, the way the booklet in the "
     "box looked. The cover is the box art; keys print as little caps.",
     [("Cover", a_cover), ("Contents", a_contents), ("A page", a_page)]),
    ("B", "The open spread",
     "Two pages side by side on the game's own dark, stapled at the spine; ← → turns a "
     "spread. The cover is the closed booklet, the right-hand page alone.",
     [("Cover", b_cover), ("Contents", b_contents), ("A spread", b_page)]),
    ("C", "The ring binder",
     "A binder with a coloured tab for every chapter down the right edge, so the "
     "contents are always in view and ↑↓ jumps tab to tab from any page.",
     [("Cover", c_cover), ("Contents", c_contents), ("A page", c_page)]),
]


def page():
    out = ["<!doctype html><meta charset=utf-8><title>Manual mock</title><style>",
           term.CSS, ".tm .r{height:12px}.tm .c{width:6px;height:12px}",
           "body{background:#0b0f16;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;"
           "margin:0;padding:24px 28px}h1{font-size:20px;margin:0 0 4px}"
           "h2{font-size:17px;margin:34px 0 4px;color:#00e5ff}p{margin:0 0 12px;max-width:900px;color:#a0aab9}"
           ".row{display:flex;gap:18px;flex-wrap:wrap}.shot h3{font-size:13px;margin:0 0 6px;"
           "color:#78828f;font-weight:600;text-transform:uppercase;letter-spacing:.06em}",
           "</style><h1>MANUAL · the booklet in the box</h1>"
           "<p>Three looks for the in-game manual at the smallest terminal (100×30), each as its "
           "cover, its contents and a page. Pages are a fixed size, so the page numbers in the "
           "contents stay true at any window size. The words are placeholders.</p>"]
    for key, name, about, shots in LOOKS:
        out.append("<h2>%s · %s</h2><p>%s</p><div class=row>" % (key, name, about))
        for label, fn in shots:
            out.append("<div class=shot><h3>%s</h3>%s</div>" % (label, term.render(fn(), 10)))
        out.append("</div>")
    return "".join(out)


if __name__ == "__main__":
    open(sys.argv[1], "w").write(page())
