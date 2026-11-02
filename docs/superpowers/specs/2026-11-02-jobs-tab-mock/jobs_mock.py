"""JOBS tab mockups, round 1, drawn over the game's own deck (seed 8, the story's drop
open, six jobs from three fixers, one in every state).

    python3 -I jobs_mock.py <shots dir> <out dir>

The shots dir holds jobs_now_*.svg and jobs_base_*.svg from the throwaway
`jobs_tab_now` shot test. The clock is moved on 3h 20m (22:00 to 01:20) so the clocks
have run; everything else is the shot's.

Now · the tab as it is.
A · Ledger: sections (STORY, IN HAND, ON OFFER, CLOSED folded), each row with its
    state, pips, clock and pay; the card with the due time, the clock bar, where each
    objective is and what blocks the hand-in; space opens the job's verbs.
B · Next steps: every row says what to do next under its title; the card draws the
    job as a route from you to the hand-in, with the window as a bar.
C · Fixers: the jobs under the fixer who gave them; a fixer's row opens their card,
    with where they are, the standing ladder and what it unlocks, and the record."""

import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else HERE

PANEL = "#04060a"
BAR_BG = "#003c46"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#788291"
FAINTER = "#5c6673"
TEXT = "#cdd4de"
RULE = "#283c4b"
DONE = "#5ac88c"
AMBER = "#ffc850"
GREEN = "#50e678"
WHITE = "#ffffff"


class Geo:
    """The deck's body at a terminal size, as `Deck::draw` lays it out."""

    def __init__(self, w, h):
        self.w, self.h = w, h
        if h >= 36:
            self.y0, self.y1 = 10, 37  # body rows y0..y1-1; the strip at y1, hints below
        else:
            self.y0, self.y1 = 5, 27
        inner = w - 2
        lw = max(inner * 2 // 5, 28)
        self.lx, self.lw = 1, lw
        self.sep = 1 + lw
        self.tx = self.sep + 2
        self.tw = w - 2 - lw - 1 - 2
        self.hint = h - 2
        self.strip = h - 3


def base(geo, name="jobs_base"):
    g = Grid.from_svg(os.path.join(S, "%s_%dx%d.svg" % (name, geo.w, geo.h)), geo.w, geo.h)
    # The empty tab's "nothing here yet": the ghost from the row below in its place.
    for x in range(geo.lx, geo.w - 1):
        g.c[geo.y0][x] = list(g.c[geo.y0 + 1][x])
    for x in range(1, geo.w - 1):
        g.c[geo.hint][x] = [" ", TEXT, PANEL, False]
    return g


def put(g, x, y, s, fg, bg=PANEL, bold=False, right=None):
    """Text on a solid backing, spaces too."""
    right = g.w - 1 if right is None else right
    for i, ch in enumerate(s):
        if x + i < right:
            g.c[y][x + i] = [ch, fg, bg, bold]
    return x + len(s)


def spans(g, x, y, parts, bg=PANEL, right=None):
    for text, fg, *bold in parts:
        x = put(g, x, y, text, fg, bg, bool(bold and bold[0]), right)
    return x


def width(parts):
    return sum(len(p[0]) for p in parts)


def cut(s, room):
    return s if len(s) <= room else s[:max(room - 1, 0)].rstrip() + "…"


def wrap(s, room):
    out, line = [], ""
    for word in s.split(" "):
        if line and len(line) + 1 + len(word) > room:
            out.append(line)
            line = word
        else:
            line = word if not line else line + " " + word
    out.append(line)
    return out


def hint(g, geo, text):
    put(g, 2, geo.hint, text, DIM)


# ---- the list

def head(g, geo, y, label, tail=None):
    """A section header: its label, a rule to the pane's edge, an optional tail."""
    x0, right = geo.lx + 3, geo.sep - 1
    put(g, geo.lx, y, "   ", DIM)
    x = put(g, x0, y, label + " ", DIM, bold=False)
    tail_parts = tail or []
    tw = width(tail_parts)
    for cx in range(x, right - (tw + 1 if tw else 0)):
        g.c[y][cx] = ["─", RULE, PANEL, False]
    if tw:
        put(g, right - tw - 1, y, " ", PANEL)
        spans(g, right - tw, y, tail_parts)


def row(g, geo, y, left, right_parts, tone=TEXT, selected=False, mark=" ", indent=0,
        mark_col=None):
    """One selectable row: the cursor, a mark, the title, and parts aligned right."""
    x0, right = geo.lx, geo.sep
    bg = BAR_BG if selected else PANEL
    for cx in range(x0, right):
        g.c[y][cx] = [" ", TEXT, bg, False]
    if selected:
        put(g, x0 + 1, y, "▸", CYAN, bg, True)
    put(g, x0 + 3 + indent, y, mark, mark_col or tone, bg, bool(mark_col))
    rw = width(right_parts)
    room = right - 1 - (x0 + 5 + indent) - (rw + 1 if rw else 0)
    fg = CYAN if selected else tone
    put(g, x0 + 5 + indent, y, cut(left, room), fg, bg, selected)
    if rw:
        parts = [(p[0], p[1], selected or (len(p) > 2 and p[2])) for p in right_parts]
        spans(g, right - 1 - rw, y, parts, bg)


def sub(g, geo, y, parts, indent=7):
    x0 = geo.lx + indent
    put(g, geo.lx, y, " " * (geo.lw - 1), TEXT)
    spans(g, x0, y, parts, right=geo.sep - 1)


def separator(g, geo):
    for y in range(geo.y0, geo.y1):
        g.c[y][geo.sep] = ["║", RULE, PANEL, False]


# ---- the card

class Card:
    def __init__(self, g, geo):
        self.g, self.geo, self.y = g, geo, geo.y0

    @property
    def right(self):
        return self.geo.tx + self.geo.tw

    def line(self, parts=(), right_parts=()):
        if self.y >= self.geo.y1:
            return
        x = self.geo.tx
        for cx in range(x, self.right):
            self.g.c[self.y][cx] = [" ", TEXT, PANEL, False]
        spans(self.g, x, self.y, list(parts), right=self.right)
        if right_parts:
            spans(self.g, self.right - width(right_parts), self.y, list(right_parts),
                  right=self.right)
        self.y += 1

    def para(self, text, fg=TEXT, lead="", indent=""):
        for i, l in enumerate(wrap(text, self.geo.tw - len(lead))):
            self.line([((lead if i == 0 else indent) + l, fg)])

    def rule(self, ch="─", fg=RULE):
        self.line([(ch * self.geo.tw, fg)])

    def gap(self):
        self.line()


def bar(n, frac, on=CYAN, off=FAINTER, ch_on="━", ch_off="━"):
    k = round(n * frac)
    return [(ch_on * k, on), (ch_off * (n - k), off)]


# ---- the data: the shot's, the clock moved on to 01:20

NOW = "01:20"
STORY = dict(title="The drop", fr="Mezz", dist=30,
             objective="Hand the package to Marlo · Old Kowloon Row Station · Kowloon Row")
FIXERS = {
    "Mezz": dict(district="Kowloon Row", standing=7, hours="18:00–04:00", dist=140),
    "Yara-11": dict(district="Siren Avenue", standing=6, hours="18:00–04:00", dist=135),
    "Pike-11": dict(district="Siren Avenue", standing=5, hours="18:00–04:00", dist=162),
}
SHARD = dict(
    title="The apartment shard", kind="fetch", fixer="Mezz", pay=200, hours=12,
    left="8h 40m", frac=520 / 720, due="10:00", taken="22:00", done=0, total=2,
    brief=["Flat 101, 1F, apartment. the server knows the code.",
           "Somebody left a shard behind. I'd like it back before they notice."],
    objectives=[("Take the shard from 101 on 1F of apartment", False),
                ("Bring it to Mezz · Kowloon Row corner · 18:00–04:00", False)],
    dist=51, next="building, then 1F", why="the shard is not yet yours")
PACKAGE = dict(
    title="No questions", kind="deliver", fixer="Yara-11", pay=131, hours=8,
    left="4h 40m", frac=280 / 480, due="06:00", taken="22:00", done=1, total=2,
    brief=["Idris-6, the line worker at Hexflux Storage. Works 22:00–06:00.",
           "Package into their hands. Not a counter, not a colleague. Hands."],
    objectives=[("Hand the package to Idris-6 · Hexflux Storage · 22:00–06:00", True),
                ("Bring it to Yara-11 · Siren Avenue corner · 18:00–04:00", False)],
    dist=135, next=None, why=None)
OFFER = dict(title="A quiet read", kind="copy", fixer="Pike-11", pay=450, hours=24,
             brief=["bar-oromed, security 150. A buyer wants what the server knows.",
                    "Quiet is worth more than fast. Bring me the copy."])
CLOSED = [("✓", "A door into bar-oromed", "done", DONE, 700, "Mezz"),
          ("✗", "A flat on 1F", "late", MAGENTA, None, "Yara-11"),
          ("–", "Across town", "dropped", FAINT, None, "Pike-11")]


def pips(job):
    return [("▰" * job["done"], CYAN), ("▱" * (job["total"] - job["done"]), FAINT)]


def standing_word(n):
    if n <= -3:
        return "refuses you"
    if n < 0:
        return "cold"
    if n < 2:
        return "new"
    if n < 4:
        return "known"
    if n < 6:
        return "trusted"
    return "inner circle"


# ---- Now

def now(geo, row):
    return Grid.from_svg(os.path.join(S, "jobs_now_%d_%dx%d.svg" % (row, geo.w, geo.h)),
                         geo.w, geo.h)


# ---- A · Ledger

def ledger_list(g, geo, sel="shard", closed_open=False):
    y = geo.y0
    narrow = geo.lw < 44
    head(g, geo, y, "STORY")
    y += 1
    row(g, geo, y, STORY["title"], [("%d m" % STORY["dist"], DIM)], selected=sel == "story")
    y += 1
    head(g, geo, y, "IN HAND · 2")
    y += 1
    for key, job in (("shard", SHARD), ("package", PACKAGE)):
        right = []
        if job["done"] == job["total"] - 1 and job["why"] is None:
            right += [("READY", CYAN, True), (" ", TEXT)]
        if not narrow:
            right += pips(job) + [(" ", TEXT)]
        right += [("%7s" % job["left"], TEXT), ("  ¤%d" % job["pay"], DIM)]
        row(g, geo, y, job["title"], right, selected=sel == key)
        y += 1
    head(g, geo, y, "ON OFFER · 1")
    y += 1
    row(g, geo, y, OFFER["title"], [("%dh" % OFFER["hours"], FAINT), ("  ¤%d" % OFFER["pay"], FAINT)],
        tone=DIM, selected=sel == "offer")
    y += 1
    if closed_open:
        head(g, geo, y, "CLOSED · 3", [("c folds", FAINT)])
        y += 1
        for mark, title, word, col, pay, _ in CLOSED:
            right = [(word, col)] + ([("  ¤%d" % pay, FAINT)] if pay else [])
            row(g, geo, y, title, right, tone=FAINT, mark=mark)
            g.c[y][geo.lx + 3][1] = col
            y += 1
    else:
        head(g, geo, y, "CLOSED · 3 · ¤700 paid", [("c ▸", FAINT)])
    return y


def ledger_card(g, geo, job):
    c = Card(g, geo)
    c.line([(job["title"], CYAN, True)], [("%s · ¤%d" % (job["kind"].upper(), job["pay"]), DIM)])
    f = FIXERS[job["fixer"]]
    c.line([("from %s · %s · %s" % (job["fixer"], f["district"], standing_word(f["standing"])), DIM)])
    c.rule()
    c.line([("due %s" % job["due"], TEXT), (" · %s left" % job["left"], DIM)],
           [("taken %s" % job["taken"], FAINT)])
    c.line(bar(geo.tw, job["frac"]))
    c.gap()
    for i, l in enumerate(job["brief"]):
        q = ("“" if i == 0 else " ") + l + ("”" if i == len(job["brief"]) - 1 else "")
        c.para(q, TEXT, indent=" ")
    c.gap()
    for i, (text, done) in enumerate(job["objectives"]):
        if done:
            c.para(text, FAINT, lead="☑ ", indent="  ")
            continue
        c.para(text.split(" · 18:00")[0], TEXT, lead="☐ ", indent="  ")
        if i == 0 and job["kind"] == "fetch":
            c.line([("  ⌖ %d m · %s" % (job["dist"], job["next"]), DIM),
                    (" · door code ", FAINT), ("not known", MAGENTA)])
        elif i == len(job["objectives"]) - 1:
            f = FIXERS[job["fixer"]]
            lead = "  ⌖ %d m · " % job["dist"] if job["dist"] and job["done"] else "  "
            c.line([(lead + "on the corner till 04:00", DIM)])
    c.gap()
    if job["why"]:
        c.line([("hand in · ", FAINT), (job["why"], DIM)])
    else:
        c.line([("✓ ready to hand in", CYAN, True), (" · %s is on the corner" % job["fixer"], DIM)])
    c.line([("⌖ tracking · t stops", CYAN)] if job is SHARD else [("t track", DIM)])
    return c


def concept_a(geo, popup=False, sel="shard", closed_open=False):
    g = base(geo)
    separator(g, geo)
    ledger_list(g, geo, sel, closed_open)
    ledger_card(g, geo, SHARD if sel == "shard" else PACKAGE)
    hint(g, geo, "Q/E tabs · ↑↓ select · space verbs · t track · m map · c closed · tab close")
    if popup:
        verbs_popup(g, geo, geo.lx + geo.lw - 10, geo.y0 + 4, SHARD["title"], [
            ("t", "Track it", "⌖ on"),
            ("m", "Show it on the MAP", ""),
            ("r", "Read Mezz's mail", ""),
            ("d", "Drop it", "standing −1"),
        ])
    return g


def verbs_popup(g, geo, x, y, title, choices):
    w = 34
    top = "┍━ " + title + " " + "━" * (w - 5 - len(title)) + "┑"
    put(g, x, y, top, CYAN)
    lines = [""] + ["%s %s  %s" % ("▸" if i == 0 else " ", k, label) for i, (k, label, _) in
                    enumerate(choices)] + [""]
    for j, l in enumerate(lines):
        yy = y + 1 + j
        put(g, x, yy, "│", CYAN)
        put(g, x + 1, yy, " " * (w - 2), TEXT)
        if l:
            i = j - 1
            k, label, tail = choices[i]
            put(g, x + 2, yy, "▸" if i == 0 else " ", CYAN, bold=True)
            put(g, x + 4, yy, k, CYAN, bold=True)
            put(g, x + 7, yy, label, WHITE if i == 0 else TEXT, bold=i == 0)
            if tail:
                put(g, x + w - 2 - len(tail), yy, tail, FAINT)
        put(g, x + w - 1, yy, "│", CYAN)
    put(g, x, y + 1 + len(lines), "┕" + "━" * (w - 2) + "┙", CYAN)


# ---- B · Next steps

def next_list(g, geo, sel="shard"):
    y = geo.y0
    head(g, geo, y, "STORY")
    y += 1
    row(g, geo, y, STORY["title"], [("from Mezz", FAINT)], selected=sel == "story")
    y += 1
    sub(g, geo, y, [("%d m · Marlo · Old Kowloon Row Station" % STORY["dist"], DIM)])
    y += 2
    head(g, geo, y, "IN HAND")
    y += 1
    row(g, geo, y, SHARD["title"], [(SHARD["left"], TEXT)], selected=sel == "shard")
    y += 1
    sub(g, geo, y, [("⌖ ", CYAN), ("%d m · %s" % (SHARD["dist"], SHARD["next"]), DIM)])
    y += 1
    row(g, geo, y, PACKAGE["title"], [("READY ", CYAN, True), (PACKAGE["left"], TEXT)],
        selected=sel == "package")
    y += 1
    sub(g, geo, y, [("%d m · hand in to Yara-11" % PACKAGE["dist"], DIM)])
    y += 2
    head(g, geo, y, "ON OFFER")
    y += 1
    row(g, geo, y, OFFER["title"], [("¤%d" % OFFER["pay"], FAINT)], tone=DIM,
        selected=sel == "offer")
    y += 1
    sub(g, geo, y, [("take it from Pike-11 · %d m" % FIXERS["Pike-11"]["dist"], FAINT)])
    y += 2
    head(g, geo, y, "CLOSED · 3", [("c ▸", FAINT)])


def route_card(g, geo, job):
    c = Card(g, geo)
    tag = job["kind"].upper()
    t = " %s " % job["title"].upper()
    fill = geo.tw - len(tag) - 2 - len(t) - len(" ¤%d" % job["pay"])
    c.line([(tag, MAGENTA, True), (" ━", RULE), (t, CYAN, True), ("━" * fill, RULE),
            (" ¤%d" % job["pay"], TEXT)])
    f = FIXERS[job["fixer"]]
    c.line([("for %s · %s · %s" % (job["fixer"], f["district"], standing_word(f["standing"])), DIM)])
    c.gap()
    if job is SHARD:
        stops = [
            ("@", WHITE, "you · Street · by Stack 101", "", None),
            ("┃", CYAN, "%d m · %s" % (job["dist"], job["next"]), "", CYAN),
            ("◆", MAGENTA, "flat 101 · 1F · apartment", "take the shard", None),
            ("┊", FAINT, "door code not known · the server knows it", "", MAGENTA),
            ("┊", FAINT, "", "", None),
            ("$", AMBER, "Mezz · Kowloon Row corner", "hand it in", None),
            (" ", FAINT, "on the corner till 04:00", "", None),
        ]
    else:
        stops = [
            ("✓", DONE, "Idris-6 · Hexflux Storage", "package handed", None),
            ("┊", FAINT, "", "", None),
            ("@", WHITE, "you · Street · by Stack 101", "", None),
            ("┃", CYAN, "%d m · Siren Avenue" % job["dist"], "", CYAN),
            ("$", AMBER, "Yara-11 · Siren Avenue corner", "hand it in", None),
            (" ", FAINT, "on the corner till 04:00", "READY", None),
        ]
    for glyph, col, text, verb, tcol in stops:
        right = [(verb, CYAN if verb == "READY" else TEXT, verb == "READY")] if verb else []
        body = DIM if glyph in "┃┊ " else TEXT
        c.line([("  ", TEXT), (glyph, col, glyph in "@$◆"), ("  ", TEXT), (text, tcol or body)],
               right)
    c.gap()
    n = geo.tw - 8
    c.line([("window  ", FAINT)] + [("━" * round(n * (1 - job["frac"])), FAINTER),
                                    ("╋", WHITE, True),
                                    ("━" * (n - 1 - round(n * (1 - job["frac"]))), CYAN)])
    c.line([("        taken %s" % job["taken"], FAINT)],
           [("now %s · " % NOW, DIM), ("%s left" % job["left"], TEXT), (" · due %s" % job["due"], DIM)])
    c.gap()
    for i, l in enumerate(job["brief"]):
        q = ("“" if i == 0 else " ") + l + ("”" if i == len(job["brief"]) - 1 else "")
        c.para(q, DIM, indent=" ")
    c.gap()
    c.line([("⌖ tracking · t stops", CYAN)] if job is SHARD else [("t track", DIM)])
    return c


def concept_b(geo, sel="shard"):
    g = base(geo)
    separator(g, geo)
    next_list(g, geo, sel)
    route_card(g, geo, SHARD if sel == "shard" else PACKAGE)
    hint(g, geo, "Q/E tabs · ↑↓ select · space verbs · t track · m map · c closed · tab close")
    return g


# ---- C · Fixers

def fixer_head(g, geo, y, name, selected=False):
    f = FIXERS[name]
    x0, right = geo.lx, geo.sep
    bg = BAR_BG if selected else PANEL
    for cx in range(x0, right):
        g.c[y][cx] = [" ", TEXT, bg, False]
    if selected:
        put(g, x0 + 1, y, "▸", CYAN, bg, True)
    x = put(g, x0 + 3, y, name.upper(), CYAN if selected else WHITE, bg, True)
    narrow = geo.lw < 44
    if not narrow:
        put(g, x, y, " · " + f["district"], DIM, bg)
    meter = [("●", GREEN), (" ", TEXT), ("▮" * f["standing"], CYAN), ("▯" * (8 - f["standing"]), FAINTER)]
    spans(g, right - 1 - width(meter), y, meter, bg)


def fixers_list(g, geo, sel="mezz"):
    y = geo.y0
    head(g, geo, y, "STORY")
    y += 1
    row(g, geo, y, STORY["title"], [("%d m" % STORY["dist"], DIM)], selected=sel == "story")
    y += 2
    head(g, geo, y, "FIXERS · 3 · 2 on the corner now")
    y += 1
    fixer_head(g, geo, y, "Mezz", sel == "mezz")
    y += 1
    row(g, geo, y, SHARD["title"], [(SHARD["left"], TEXT), ("  ¤%d" % SHARD["pay"], DIM)],
        selected=sel == "shard", indent=2)
    y += 1
    row(g, geo, y, "A door into bar-oromed", [("done", DONE)], tone=FAINT, mark="✓", indent=2)
    g.c[y][geo.lx + 5][1] = DONE
    y += 2
    fixer_head(g, geo, y, "Yara-11", sel == "yara")
    y += 1
    row(g, geo, y, PACKAGE["title"], [("READY ", CYAN, True), (PACKAGE["left"], TEXT),
                                      ("  ¤%d" % PACKAGE["pay"], DIM)],
        selected=sel == "package", indent=2)
    y += 1
    row(g, geo, y, "A flat on 1F", [("late", MAGENTA)], tone=FAINT, mark="✗", indent=2)
    g.c[y][geo.lx + 5][1] = MAGENTA
    y += 2
    fixer_head(g, geo, y, "Pike-11", sel == "pike")
    y += 1
    row(g, geo, y, OFFER["title"], [("offer", AMBER), ("  ¤%d" % OFFER["pay"], DIM)],
        tone=DIM, selected=sel == "offer", indent=2)
    y += 1
    row(g, geo, y, "Across town", [("dropped", FAINT)], tone=FAINT, mark="–", indent=2)


def fixer_card(g, geo, name):
    f = FIXERS[name]
    c = Card(g, geo)
    c.line([(name.upper(), CYAN, True)], [("fixer · %s" % f["district"], DIM)])
    c.line([("●", GREEN), (" on the corner now · till 04:00 · %d m" % f["dist"], DIM)])
    c.gap()
    c.line([("standing ", DIM), ("%d" % f["standing"], WHITE, True), (" · %s" % standing_word(f["standing"]), CYAN)])
    # The ladder: -3 .. 8, two cells a step.
    lo, hi, step = -3, 8, max((geo.tw - 4) // 11, 3)
    marks = {-3: "refuses", 0: "fetch", 2: "copy", 4: "plant", 6: "vault"}
    lad, lab, you = [], [" "] * (step * (hi - lo) + 2), [" "] * (step * (hi - lo) + 2)
    for v in range(lo, hi + 1):
        x = (v - lo) * step
        on = v <= f["standing"]
        ch = "┿" if v in marks else "━"
        lad.append((ch, CYAN if on else FAINTER))
        if v < hi:
            lad.append(("━" * (step - 1), CYAN if v < f["standing"] else FAINTER))
        if v in marks:
            for i, chh in enumerate(marks[v]):
                if x + i < len(lab):
                    lab[x + i] = chh
        if v == f["standing"]:
            you[x] = "▲"
    c.line([(" ", TEXT)] + lad)
    c.line([(" " + "".join(lab), FAINT)])
    c.line([(" " + "".join(you), WHITE)])
    c.line([("  done +1 · dropped −1 · late −2 · refuses you at −3", FAINTER)])
    c.gap()
    c.rule()
    c.line([("record  ", FAINT), ("1 done", DONE), (" · 0 late · 0 dropped · ", DIM), ("¤700", TEXT),
            (" paid", DIM)])
    c.line([("in hand ", FAINT), (SHARD["title"], TEXT), (" · %s left" % SHARD["left"], DIM)])
    c.line([("offers  ", FAINT), ("face to face · one at a time", DIM)])
    c.rule()
    c.gap()
    c.para("last said · “Details on your rig. Don't lose them.”", DIM)
    c.gap()
    c.line([("t track Mezz", DIM)])


def concept_c(geo, sel="mezz"):
    g = base(geo)
    separator(g, geo)
    fixers_list(g, geo, sel)
    if sel == "mezz":
        fixer_card(g, geo, "Mezz")
    else:
        ledger_card(g, geo, PACKAGE if sel == "package" else SHARD)
    hint(g, geo, "Q/E tabs · ↑↓ select · space verbs · t track · m map · tab close")
    return g


# ---- Round 2 · C's list, B's card

DIM_AMBER = "#be963c"
CLOSED_BY = {"Mezz": CLOSED[0], "Yara-11": CLOSED[1], "Pike-11": CLOSED[2]}
NEXT = {
    "shard": [("⌖ ", CYAN), ("%d m · %s" % (SHARD["dist"], SHARD["next"]), DIM)],
    "package": [("%d m · hand in to Yara-11" % PACKAGE["dist"], DIM)],
    "offer": [("take it from Pike-11 · %d m" % FIXERS["Pike-11"]["dist"], FAINT)],
}


def fixer_row(g, geo, y, name, selected=False):
    """A fixer's row: the name, the district when there is room, the standing meter."""
    f = FIXERS[name]
    x0, right = geo.lx, geo.sep
    bg = BAR_BG if selected else PANEL
    for cx in range(x0, right):
        g.c[y][cx] = [" ", TEXT, bg, False]
    if selected:
        put(g, x0 + 1, y, "▸", CYAN, bg, True)
    x = put(g, x0 + 3, y, name.upper(), CYAN if selected else WHITE, bg, True)
    meter = [("▮" * f["standing"], CYAN), ("▯" * (8 - f["standing"]), FAINTER)]
    if x + 3 + len(f["district"]) < right - 1 - width(meter) - 1:
        put(g, x, y, " · " + f["district"], DIM, bg)
    spans(g, right - 1 - width(meter), y, meter, bg)


def fixers2(g, geo, sel="shard", steps=False, fold=False):
    """C's list: the story, then each fixer with their jobs; `steps` puts the next step
    under each live job, `fold` folds each fixer's closed jobs into one row."""
    y = geo.y0
    head(g, geo, y, "STORY")
    y += 1
    row(g, geo, y, STORY["title"], [("%d m" % STORY["dist"], DIM)], selected=sel == "story")
    y += 1
    if steps:
        sub(g, geo, y, [("%d m · Marlo · Old Kowloon Row Station" % STORY["dist"], DIM)])
        y += 1
    y += 1
    head(g, geo, y, "FIXERS · on their corners till 04:00" if geo.lw > 44 else "FIXERS · till 04:00")
    y += 1
    narrow = geo.lw < 44
    live = {
        "Mezz": ("shard", SHARD["title"], [(SHARD["left"], TEXT)] + ([] if narrow else [("  ¤%d" % SHARD["pay"], DIM)]), TEXT),
        "Yara-11": ("package", PACKAGE["title"], [("READY ", CYAN, True), (PACKAGE["left"], TEXT)] + ([] if narrow else [("  ¤%d" % PACKAGE["pay"], DIM)]), TEXT),
        "Pike-11": ("offer", OFFER["title"], [("offer", AMBER), ("  ¤%d" % OFFER["pay"], DIM)], DIM),
    }
    for i, name in enumerate(FIXERS):
        fixer_row(g, geo, y, name, sel == name)
        y += 1
        key, title, right, tone = live[name]
        tracked = key == "shard"
        row(g, geo, y, title, right, tone=tone, selected=sel == key, indent=2,
            mark="⌖" if tracked else " ", mark_col=CYAN if tracked else None)
        y += 1
        if steps:
            sub(g, geo, y, NEXT[key], indent=9)
            y += 1
        mark, ctitle, word, col, pay, _ = CLOSED_BY[name]
        if fold:
            row(g, geo, y, "1 closed", [("→ unfold", FAINTER)], tone=FAINT, indent=2)
        else:
            picked = sel == "closed" and name == "Mezz"
            row(g, geo, y, ctitle, [(word, col)], tone=FAINT, mark=mark, indent=2,
                selected=picked)
            g.c[y][geo.lx + 5][1] = col
        y += 1
        if i < len(FIXERS) - 1 and not (steps and geo.h < 36):
            y += 1


def title_line(c, geo, tag, tag_col, title, right):
    t = " %s " % title.upper()
    fill = geo.tw - len(tag) - 2 - len(t) - len(right)
    c.line([(tag, tag_col, True), (" ━", RULE), (t, CYAN, True), ("━" * max(fill, 0), RULE),
            (right, TEXT)])


def stop_lines(c, stops):
    for glyph, col, text, verb, tcol in stops:
        vcol = CYAN if verb == "READY" else (FAINT if tcol == FAINT else TEXT)
        right = [(verb, vcol, verb == "READY")] if verb else []
        body = DIM if glyph in "┃┊ " else TEXT
        c.line([("  ", TEXT), (glyph, col, glyph in "@$◆◇"), ("  ", TEXT), (text, tcol or body)],
               right)


def track_line(c, where, dist, tracked=False):
    """The card's last line: `t` as a key, what it does and what tracking shows; while
    tracked, a lit band with the distance and how to stop."""
    g, geo, y = c.g, c.geo, c.y
    c.line()
    if y >= geo.y1:
        return
    if tracked:
        for cx in range(geo.tx, c.right):
            g.c[y][cx] = [" ", TEXT, BAR_BG, False]
        x = put(g, geo.tx + 1, y, "⌖ TRACKING", CYAN, BAR_BG, True)
        tail = "  %s · %d m · the floor lights the way" % (where, dist)
        room = c.right - 10 - x
        if len(tail) > room:
            tail = "  %s · %d m" % (where, dist)
        put(g, x, y, tail, TEXT, BAR_BG, right=c.right - 9)
        x = put(g, c.right - 8, y, " t ", PANEL, CYAN, True)
        put(g, x, y, " stop", DIM, BAR_BG, right=c.right)
        return
    x = put(g, geo.tx, y, " t ", PANEL, CYAN, True)
    x = put(g, x, y, "  TRACK", CYAN, PANEL, True)
    tail = " · the floor lights the way to %s" % where
    if x + len(tail) > c.right:
        tail = " · lights the way to %s" % where
    put(g, x, y, tail, DIM, right=c.right)


def window_bar(c, geo, job, fixer_hours=False):
    """The window from taken to due, now on it; with `fixer_hours`, the stretch after
    the fixer is off the corner drawn apart, and when they leave."""
    n = geo.tw - 8
    spent = round(n * (1 - job["frac"]))
    if not fixer_hours:
        c.line([("window  ", FAINT), ("━" * spent, FAINTER), ("╋", WHITE, True),
                ("━" * (n - 1 - spent), CYAN)])
        c.line([("        taken %s" % job["taken"], FAINT)],
               [("now %s · " % NOW, DIM), ("%s left" % job["left"], TEXT),
                (" · due %s" % job["due"], DIM)])
        return
    # Taken 22:00; the fixer off the corner at 04:00 (360 minutes in) until 18:00.
    total = job["hours"] * 60
    off = round(n * 360 / total)
    c.line([("window  ", FAINT), ("━" * spent, FAINTER), ("╋", WHITE, True),
            ("━" * (off - spent - 1), CYAN), ("┄" * (n - off), DIM_AMBER)])
    lab = [" "] * (n + 8)
    for x, s in ((8, "taken %s" % job["taken"]), (8 + off - 2, "04:00"),
                 (8 + n - len("due %s" % job["due"]), "due %s" % job["due"])):
        for i, ch in enumerate(s):
            if x + i < len(lab):
                lab[x + i] = ch
    c.line([("".join(lab), FAINT)])
    # Advice, not a deadline: a fixer off the corner still takes the hand-in.
    tail = " · %s is back at 18:00" % job["fixer"]
    if len("off the corner from 04:00" + tail) > geo.tw:
        tail = ""
    c.line([("off the corner from 04:00", AMBER), (tail, DIM)])


def job_card(g, geo, job, fixer_hours=False):
    c = Card(g, geo)
    f = FIXERS[job["fixer"]]
    title_line(c, geo, job["kind"].upper(), MAGENTA, job["title"], " ¤%d" % job["pay"])
    c.line([("for %s · %s · %s" % (job["fixer"], f["district"], standing_word(f["standing"])), DIM)])
    c.gap()
    if job is SHARD:
        stop_lines(c, [
            ("@", WHITE, "you · Street · by Stack 101", "", None),
            ("┃", CYAN, "%d m · %s" % (job["dist"], job["next"]), "", CYAN),
            ("◆", MAGENTA, "flat 101 · 1F · apartment", "take the shard", None),
            ("┊", FAINT, "door code not known · the server knows it", "", MAGENTA),
            ("┊", FAINT, "", "", None),
            ("$", AMBER, "Mezz · Kowloon Row corner", "hand it in", None),
            (" ", FAINT, "on the corner till 04:00", "", None),
        ])
    else:
        stop_lines(c, [
            ("✓", DONE, "Idris-6 · Hexflux Storage", "package delivered", None),
            ("┊", FAINT, "", "", None),
            ("@", WHITE, "you · Street · by Stack 101", "", None),
            ("┃", CYAN, "%d m · Siren Avenue" % job["dist"], "", CYAN),
            ("$", AMBER, "Yara-11 · Siren Avenue corner", "hand it in", None),
            (" ", FAINT, "on the corner till 04:00", "READY", None),
        ])
    c.gap()
    window_bar(c, geo, job, fixer_hours)
    c.gap()
    for i, l in enumerate(job["brief"]):
        q = ("“" if i == 0 else " ") + l + ("”" if i == len(job["brief"]) - 1 else "")
        c.para(q, DIM, indent=" ")
    c.gap()
    if job is SHARD:
        track_line(c, "flat 101", job["dist"], tracked=True)
    else:
        track_line(c, "Yara-11's corner", job["dist"])


def offer_card(g, geo):
    c = Card(g, geo)
    f = FIXERS["Pike-11"]
    title_line(c, geo, "COPY", MAGENTA, OFFER["title"], " ¤%d" % OFFER["pay"])
    c.line([("on offer from Pike-11 · %s · %s" % (f["district"], standing_word(f["standing"])), DIM)])
    c.gap()
    stop_lines(c, [
        ("@", WHITE, "you · Street · by Stack 101", "", None),
        ("┃", CYAN, "%d m · Siren Avenue" % f["dist"], "", CYAN),
        ("$", AMBER, "Pike-11 · Siren Avenue corner", "take the job", None),
        ("┊", FAINT, "on the corner till 04:00", "", None),
        ("┊", FAINT, "", "", None),
        ("◇", FAINT, "bar-oromed · security 150", "copy its server", FAINT),
        ("┊", FAINT, "in the Net", "", FAINT),
        ("$", FAINT, "Pike-11 · Siren Avenue corner", "hand it in", FAINT),
    ])
    c.gap()
    c.line([("window  ", FAINT), ("24 hours", TEXT), (", from when you take it", DIM)])
    c.gap()
    for i, l in enumerate(OFFER["brief"]):
        q = ("“" if i == 0 else " ") + l + ("”" if i == len(OFFER["brief"]) - 1 else "")
        c.para(q, DIM, indent=" ")
    c.gap()
    c.line([("copy work · Pike-11 offers it from standing 2 · you are at 5", FAINT)])
    track_line(c, "Pike-11's corner", f["dist"])


def fixer_card2(g, geo, name="Mezz"):
    f = FIXERS[name]
    c = Card(g, geo)
    title_line(c, geo, "FIXER", AMBER, name, " " + standing_word(f["standing"]))
    c.line([("%s corner · %s · on it now" % (f["district"], f["hours"]), DIM)])
    c.gap()
    stop_lines(c, [
        ("@", WHITE, "you · Street · by Stack 101", "", None),
        ("┃", CYAN, "%d m · %s" % (f["dist"], f["district"]), "", CYAN),
        ("$", AMBER, "%s · %s corner" % (name, f["district"]), "2h 40m left", None),
    ])
    c.gap()
    c.line([("standing ", DIM), ("%d" % f["standing"], WHITE, True), (" · %s" % standing_word(f["standing"]), CYAN)])
    lo, hi = -3, 8
    step = max((geo.tw - 4) // 11, 3)
    marks = {-3: "refuses", 0: "fetch", 2: "copy", 4: "plant", 6: "vault"}
    lad = []
    lab = [" "] * (step * (hi - lo) + 2)
    you = [" "] * (step * (hi - lo) + 2)
    for v in range(lo, hi + 1):
        lad.append(("┿" if v in marks else "━", CYAN if v <= f["standing"] else FAINTER))
        if v < hi:
            lad.append(("━" * (step - 1), CYAN if v < f["standing"] else FAINTER))
        x = (v - lo) * step
        for i, ch in enumerate(marks.get(v, "")):
            if x + i < len(lab):
                lab[x + i] = ch
        if v == f["standing"]:
            you[x] = "▲"
    c.line([(" ", TEXT)] + lad)
    c.line([(" " + "".join(lab), FAINT)])
    c.line([(" " + "".join(you), WHITE)])
    c.line([("  done +1 · dropped −1 · late −2", FAINTER)])
    c.gap()
    c.line([("record  ", FAINT), ("1 done", DONE), (" · 0 late · 0 dropped · ", DIM), ("¤700", TEXT),
            (" paid", DIM)])
    c.line([("in hand ", FAINT), (SHARD["title"], TEXT), (" · %s left" % SHARD["left"], DIM)])
    c.line([("said    ", FAINT), ("“Details on your rig. Don't lose them.”", DIM)])
    c.gap()
    track_line(c, "%s's corner" % name, f["dist"])


def closed_card(g, geo):
    c = Card(g, geo)
    title_line(c, geo, "PLANT", MAGENTA, "A door into bar-oromed", " ¤700")
    c.line([("for Mezz · Kowloon Row · done", DIM)])
    c.gap()
    stop_lines(c, [
        ("✓", DONE, "bar-oromed · its host", "backdoor planted", FAINT),
        ("┊", FAINT, "", "", None),
        ("✓", DONE, "Mezz · Kowloon Row corner", "handed in", FAINT),
    ])
    c.gap()
    c.line([("PAID ", DONE, True), ("¤700", WHITE, True), (" · standing +1 → 7", DIM)])
    c.line([("yours   ", FAINT), ("the backdoor on bar-oromed · health ", DIM),
            ("STABLE", TEXT)])
    c.gap()
    c.para("“bar-oromed. Security 150. I want a key under the mat.", DIM)
    c.para(" Plant it, then find me before it burns.”", DIM)


def story_card(g, geo):
    c = Card(g, geo)
    title_line(c, geo, "STORY", MAGENTA, STORY["title"], "")
    c.line([("from Mezz · open", DIM)])
    c.gap()
    stop_lines(c, [
        ("@", WHITE, "you · Street · by Stack 101", "", None),
        ("┃", CYAN, "%d m · Kowloon Row" % STORY["dist"], "", CYAN),
        ("◆", MAGENTA, "Marlo · Old Kowloon Row Station", "deliver the package", None),
    ])
    c.gap()
    track_line(c, "Marlo", STORY["dist"])


def together(geo, sel="shard", card=None, steps=False, fold=False, popup=None):
    g = base(geo)
    separator(g, geo)
    fixers2(g, geo, sel, steps, fold)
    (card or (lambda g, geo: job_card(g, geo, SHARD)))(g, geo)
    hint(g, geo, "Q/E tabs · ↑↓ select · ← → fold · space verbs · t track · m map · tab close"
         if fold else "Q/E tabs · ↑↓ select · space verbs · t track · m map · tab close")
    if popup:
        x, y, title, choices = popup(geo)
        verbs_popup(g, geo, x, y, title, choices)
    return g


ROUND2 = [
    ("t", "Together · a job in hand",
     lambda geo: together(geo),
     "C's list on the left: the story, then each fixer with their standing, their live job "
     "and what they closed. B's card on the right: the job as a route from you to the "
     "hand-in, and its window."),
    ("t_ready", "Together · ready to hand in",
     lambda geo: together(geo, sel="package", card=lambda g, geo: job_card(g, geo, PACKAGE)),
     "The delivered job: the first stop ticked, the walk to Yara-11, READY."),
    ("v_hours", "Variant · the fixer's hours on the window",
     lambda geo: together(geo, card=lambda g, geo: job_card(g, geo, SHARD, fixer_hours=True)),
     "Fixers hold their corners 18:00–04:00; after that you have to find them (they still "
     "take the hand-in). The window shows the stretch they are off the corner (amber "
     "dashes) and when they leave."),
    ("v_steps", "Variant · next steps in the list, history folded",
     lambda geo: together(geo, steps=True, fold=True),
     "B's line under every live job, so the list alone says what to do next; each fixer's "
     "closed jobs fold into one row (→ unfolds), so the list stays short however long you play."),
    ("x_offer", "Addition · an offer",
     lambda geo: together(geo, sel="offer", card=offer_card),
     "The route starts at the fixer: take the job there, then the rest of it greyed. The "
     "window says when the clock starts; the last line says which standing opened this work."),
    ("x_fixer", "Addition · a fixer",
     lambda geo: together(geo, sel="Mezz", card=fixer_card2),
     "The fixer's own card in B's language: the walk to their corner and how long they hold "
     "it, the standing ladder and what each step unlocks, the record, the last thing they said."),
    ("x_closed", "Addition · a closed job",
     lambda geo: together(geo, sel="closed", card=closed_card),
     "Every stop ticked, what it paid, what it did to the standing, and what is still "
     "yours (the backdoor, with its health). A late or dropped job says what it cost instead."),
    ("x_story", "Addition · the story",
     lambda geo: together(geo, sel="story", card=story_card),
     "The story's step as a route too, with no window."),
    ("x_verbs", "Addition · space on a fixer",
     lambda geo: together(geo, sel="Mezz", card=fixer_card2, popup=lambda geo: (
         geo.lx + geo.lw - 12, geo.y0 + 5, "Mezz", [
             ("t", "Track Mezz", "⌖"),
             ("m", "Show on the MAP", ""),
             ("r", "Read Mezz's mail", "2"),
         ])),
     "A fixer's verbs; a job's are track, map, read the mail and drop."),
]


# ---- the page

BIG, SMALL = Geo(120, 40), Geo(100, 30)
CONCEPTS = [
    ("now", "Now · the tab as it is", lambda geo: now(geo, 1),
     "The list is titles only. Offered and dropped jobs are DIM, barely apart from the "
     "jobs in hand; finished jobs stay forever. The card has no due time, no distance, no "
     "door code and no reason the hand-in is refused; the map shows between its words. "
     "Space does nothing although the hints offer it."),
    ("a", "A · Ledger", lambda geo: concept_a(geo),
     "Sections with counts; CLOSED folds away (c). Each row says READY, its pips, its "
     "clock and its pay. The card: due 10:00 and the clock bar, the fixer's standing as a "
     "word, the distance and next leg under the objective, the door code you lack, and why "
     "the hand-in waits."),
    ("a_verbs", "A · space on a job", lambda geo: concept_a(geo, popup=True),
     "The verbs every job has, without walking to the fixer: track, show on the MAP, read "
     "the mail, drop."),
    ("a_ready", "A · a job ready to hand in", lambda geo: concept_a(geo, sel="package", closed_open=True),
     "The delivered job, ready; CLOSED unfolded with what each one paid or cost."),
    ("b", "B · Next steps", lambda geo: concept_b(geo),
     "Every row carries its next step under the title, so the list alone says what to do. "
     "The card draws the job as a route: you, the walk, the flat, the fixer's corner; the "
     "window as a bar with now on it."),
    ("b_ready", "B · the delivered job", lambda geo: concept_b(geo, sel="package"),
     "The route with the first stop done and the hand-in ready."),
    ("c", "C · Fixers", lambda geo: concept_c(geo),
     "The jobs under the fixer who gave them. A fixer's row opens their card: on the corner "
     "or not, the standing ladder and what each step unlocks, the record, the job in hand."),
    ("c_job", "C · a job under its fixer", lambda geo: concept_c(geo, sel="package"),
     "A job's row opens the same card as A's."),
]


ROUNDS = {
    1: (CONCEPTS, "JOBS tab · round 1", ""),
    2: (ROUND2, "JOBS tab · round 2 · C's list, B's card",
        " Round 2: the fixers' list with the route card, then variants and additions."),
}


def page(n, px_big=13.0, px_small=11.0):
    concepts, head_text, more = ROUNDS[n]
    css = term.CSS + """
body{background:#10131a;color:#e6e6f0;font-family:system-ui;margin:0;padding:24px}
h2{font-size:20px;margin:28px 0 6px} p{max-width:1000px;color:#b8c0cc;margin:0 0 10px;
font-size:14px;line-height:1.45} .pair{display:flex;gap:18px;align-items:flex-start;flex-wrap:wrap}
.lab{font-size:12px;color:#8a93a3;margin:4px 0}"""
    out = ["<!doctype html><meta charset=utf-8><title>JOBS tab mockups</title><style>%s</style>" % css,
           "<h1 style='font-size:24px'>%s</h1>" % html.escape(head_text),
           "<p>Drawn over the game's own deck: seed 8, the story's drop open, six jobs from "
           "three fixers. The clock is moved on to 01:20 so the clocks have run.%s</p>"
           % html.escape(more)]
    for key, title, draw, note in concepts:
        out.append("<h2>%s</h2><p>%s</p><div class='pair'>" % (html.escape(title), html.escape(note)))
        out.append("<div><div class='lab'>120 × 40</div>%s</div>" % term.render(draw(BIG), px_big))
        out.append("<div><div class='lab'>100 × 30</div>%s</div>" % term.render(draw(SMALL), px_small))
        out.append("</div>")
    return "".join(out)


def single(concepts, key, geo, px):
    for k, title, draw, note in concepts:
        if k == key:
            css = term.CSS + ("body{background:#10131a;color:#e6e6f0;font-family:system-ui;"
                              "margin:0;padding:14px} h2{font-size:22px;margin:0 0 6px} "
                              "p{color:#b8c0cc;font-size:15px;line-height:1.4;margin:0 0 10px;"
                              "max-width:%dpx}" % int(geo.w * px * 0.6))
            return ("<!doctype html><meta charset=utf-8><style>%s</style><h2>%s</h2><p>%s</p>%s"
                    % (css, html.escape(title), html.escape(note), term.render(draw(geo), px)))


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for n, (concepts, _, _) in ROUNDS.items():
        open(os.path.join(OUT, "jobs_round%d.html" % n), "w").write(page(n))
        for k, *_ in concepts:
            name = "r%d_%s" % (n, k)
            open(os.path.join(OUT, "one_%s.html" % name), "w").write(single(concepts, k, BIG, 14))
            open(os.path.join(OUT, "one_%s_small.html" % name), "w").write(
                single(concepts, k, SMALL, 14))
    print("ok", OUT)
